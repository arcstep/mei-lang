use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use mei_host_core::{CacheLayersReady, EvalSlotDescriptor, HostContext};
use mei_host_graph::{default_metric_response_descriptor, record_slot_failed};
use mei_lang_datasets::{
    collect_all_query_options, default_result_artifact_scope, evaluate_runtime_metrics,
    load_metric_response_result_artifact, metric_request_revision_fingerprint_for_compiled,
    metric_response_cache_scope_key, populate_l1_from_loaded_metric_artifact,
    project_requested_metrics, store_cached_metric_response, store_metric_response_result_artifact,
    take_cached_metric_response, RuntimeMetricEvalMode,
};
use mei_lang_kernel::{CompiledApp, FilterIntent, MetricContract, QueryState};

#[derive(Debug, Clone)]
pub struct EvalPipelineRequest {
    pub scope_key: String,
    pub target: Option<String>,
    pub owner_resource_id: String,
    pub metric_ids: Vec<String>,
    pub workset_id: String,
    pub bundle_key: String,
    pub query_state: QueryState,
    pub filter_intents: Vec<FilterIntent>,
}

#[derive(Debug, Clone)]
pub struct EvalPipelineOutcome {
    pub descriptors: Vec<EvalSlotDescriptor>,
    pub cache_key: String,
    pub artifact_hit: bool,
    pub cache_layer: String,
    pub result_artifact_hit: bool,
    pub wall_ms: u64,
    pub metrics: Vec<MetricContract>,
    pub total_rows: usize,
    pub query_perf: BTreeMap<String, u64>,
}

pub fn eval_metrics_with_slots(
    ctx: &HostContext,
    compiled: &CompiledApp,
    compile_revision: &str,
    request: &EvalPipelineRequest,
) -> anyhow::Result<EvalPipelineOutcome> {
    let started = Instant::now();
    let app_root = ctx.app_root();
    let query_options = collect_all_query_options(&request.query_state);
    let owner_dataset = compiled
        .resources
        .iter()
        .find(|resource| resource.id == request.owner_resource_id)
        .and_then(|resource| resource.dataset.as_ref())
        .ok_or_else(|| {
            anyhow::anyhow!("owner resource `{}` not found", request.owner_resource_id)
        })?;
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root.as_path(),
        compiled,
        request.owner_resource_id.as_str(),
        &owner_dataset.runtime_metric_defs,
    );
    let cache_key = metric_response_cache_scope_key(
        ctx.app_id.as_str(),
        request.scope_key.as_str(),
        request.target.as_deref(),
        request.owner_resource_id.as_str(),
        &query_options,
        compile_revision,
        dependency_revision_key.as_str(),
        &request.filter_intents,
        None,
    );
    let requested: BTreeSet<String> = request.metric_ids.iter().cloned().collect();
    let request_all_metrics = request.metric_ids.is_empty();
    let result_artifact_candidate =
        default_result_artifact_scope(&request.query_state, &request.filter_intents);

    if let Some(cached) =
        take_cached_metric_response(cache_key.as_str(), &requested, request_all_metrics)
    {
        return Ok(build_outcome_from_cached(
            request,
            dependency_revision_key.as_str(),
            cache_key.as_str(),
            started,
            cached.total_rows,
            &cached.metrics_map,
            "memory",
            true,
            false,
        ));
    }

    if result_artifact_candidate {
        if let Some((artifact, artifact_load_ms)) =
            load_metric_response_result_artifact(app_root.as_path(), cache_key.as_str())?
        {
            let artifact_covers_request = if request_all_metrics {
                artifact.complete
            } else {
                requested
                    .iter()
                    .all(|metric_id| artifact.covered_metric_ids.contains(metric_id))
            };
            if artifact_covers_request {
                populate_l1_from_loaded_metric_artifact(
                    std::slice::from_ref(&cache_key),
                    &artifact,
                );
                let query_perf =
                    BTreeMap::from([("result_artifact_load_ms".to_string(), artifact_load_ms)]);
                let metrics = project_requested_metrics(
                    request.owner_resource_id.as_str(),
                    &request.metric_ids,
                    &BTreeMap::new(),
                    &artifact.metrics_map,
                );
                let descriptors = build_descriptors_for_metrics(
                    request,
                    dependency_revision_key.as_str(),
                    cache_key.as_str(),
                    artifact_load_ms,
                    true,
                    "disk",
                    CacheLayersReady {
                        disk: true,
                        memory: true,
                        client: false,
                    },
                );
                return Ok(EvalPipelineOutcome {
                    descriptors,
                    cache_key: cache_key.clone(),
                    artifact_hit: true,
                    cache_layer: "disk".to_string(),
                    result_artifact_hit: true,
                    wall_ms: started.elapsed().as_millis() as u64,
                    metrics,
                    total_rows: artifact.total_rows,
                    query_perf,
                });
            }
        }
    }

    let eval = match evaluate_runtime_metrics(
        compiled,
        app_root.as_path(),
        request.owner_resource_id.as_str(),
        &request.metric_ids,
        request.scope_key.as_str(),
        request.target.as_deref(),
        &request.query_state,
        &request.filter_intents,
        RuntimeMetricEvalMode::WithDag,
    ) {
        Ok(eval) => eval,
        Err(error) => {
            record_eval_failures(
                ctx,
                request,
                dependency_revision_key.as_str(),
                error.to_string().as_str(),
            );
            return Err(error);
        }
    };
    store_metric_response_result_artifact(
        app_root.as_path(),
        cache_key.as_str(),
        eval.total_rows,
        &eval.metrics_map,
        &requested,
        requested.len() == request.metric_ids.len(),
    )?;
    store_cached_metric_response(
        cache_key.clone(),
        eval.total_rows,
        &eval.metrics_map,
        &requested,
        requested.len() == request.metric_ids.len(),
    );
    let wall_ms = started.elapsed().as_millis() as u64;
    let descriptors = build_descriptors_for_metrics(
        request,
        dependency_revision_key.as_str(),
        cache_key.as_str(),
        wall_ms,
        false,
        "compute",
        CacheLayersReady {
            disk: true,
            memory: true,
            client: false,
        },
    );
    Ok(EvalPipelineOutcome {
        descriptors,
        cache_key,
        artifact_hit: false,
        cache_layer: "compute".to_string(),
        result_artifact_hit: false,
        wall_ms,
        metrics: eval.metrics,
        total_rows: eval.total_rows,
        query_perf: eval.query_perf,
    })
}

fn build_outcome_from_cached(
    request: &EvalPipelineRequest,
    data_source_revision: &str,
    cache_key: &str,
    started: Instant,
    total_rows: usize,
    metrics_map: &BTreeMap<String, MetricContract>,
    cache_layer: &str,
    artifact_hit: bool,
    result_artifact_hit: bool,
) -> EvalPipelineOutcome {
    let wall_ms = started.elapsed().as_millis() as u64;
    let metrics = project_requested_metrics(
        request.owner_resource_id.as_str(),
        &request.metric_ids,
        &BTreeMap::new(),
        metrics_map,
    );
    let descriptors = build_descriptors_for_metrics(
        request,
        data_source_revision,
        cache_key,
        wall_ms,
        artifact_hit,
        cache_layer,
        CacheLayersReady {
            disk: true,
            memory: cache_layer == "memory",
            client: false,
        },
    );
    EvalPipelineOutcome {
        descriptors,
        cache_key: cache_key.to_string(),
        artifact_hit,
        cache_layer: cache_layer.to_string(),
        result_artifact_hit,
        wall_ms,
        metrics,
        total_rows,
        query_perf: BTreeMap::from([("cache_layer".to_string(), 1)]),
    }
}

fn record_eval_failures(
    ctx: &HostContext,
    request: &EvalPipelineRequest,
    data_source_revision: &str,
    error_message: &str,
) {
    let bundle_revision = if request.bundle_key.is_empty() {
        request.owner_resource_id.as_str()
    } else {
        request.bundle_key.as_str()
    };
    for metric_id in &request.metric_ids {
        let slot_key = format!("{}::{}", request.workset_id, metric_id);
        if let Err(record_error) = record_slot_failed(
            ctx.workspace_root.as_path(),
            ctx.app_id.as_str(),
            slot_key.as_str(),
            request.scope_key.as_str(),
            request.owner_resource_id.as_str(),
            bundle_revision,
            data_source_revision,
            error_message,
        ) {
            tracing::warn!(
                app_id = %ctx.app_id,
                metric_id = %metric_id,
                error = %record_error,
                "failed to record MRG slot failure"
            );
        }
    }
}

fn build_descriptors_for_metrics(
    request: &EvalPipelineRequest,
    data_source_revision: &str,
    cache_key: &str,
    wall_ms: u64,
    artifact_hit: bool,
    cache_layer: &str,
    layers: CacheLayersReady,
) -> Vec<EvalSlotDescriptor> {
    request
        .metric_ids
        .iter()
        .map(|metric_id| {
            let mut descriptor = default_metric_response_descriptor(
                &format!("{}::{}", request.workset_id, metric_id),
                request.scope_key.as_str(),
                request.owner_resource_id.as_str(),
                request.bundle_key.as_str(),
                data_source_revision,
                cache_key,
                wall_ms,
                artifact_hit,
            );
            descriptor.workset_id = request.workset_id.clone();
            descriptor.cache_layer = cache_layer.to_string();
            descriptor.cache_layers_ready = layers.clone();
            descriptor
        })
        .collect()
}
