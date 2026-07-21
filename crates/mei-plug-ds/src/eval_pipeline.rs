use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use mei_host_core::{CacheLayersReady, EvalSlotDescriptor, HostContext};
use mei_host_graph::{default_metric_response_descriptor, record_slot_failed};
use mei_lang_datasets::{
    collect_all_query_options, default_result_artifact_scope, evaluate_runtime_metrics,
    metric_id_is_scalar_rowset, metric_request_revision_fingerprint_for_compiled,
    metric_response_cache_scope_key, populate_l1_from_loaded_metric_artifact,
    project_metrics_map_for_l1, project_requested_metrics, request_needs_bulk_l1_metrics,
    store_cached_metric_response, store_cached_metric_response_aliases,
    store_demand_metric_response, store_metric_response_lite_only,
    store_metric_response_result_artifact, take_cached_metric_response,
    take_demand_metric_response, try_load_disk_metric_response, L1PinPolicy,
    LoadedMetricResponseArtifact, RuntimeMetricEvalMode,
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
    let mut lookup_cache_keys = vec![cache_key.clone()];
    if request.target.is_some() {
        let warmup_cache_key = metric_response_cache_scope_key(
            ctx.app_id.as_str(),
            request.scope_key.as_str(),
            None,
            request.owner_resource_id.as_str(),
            &query_options,
            compile_revision,
            dependency_revision_key.as_str(),
            &request.filter_intents,
            None,
        );
        if warmup_cache_key != cache_key {
            lookup_cache_keys.push(warmup_cache_key);
        }
    }
    let requested: BTreeSet<String> = request.metric_ids.iter().cloned().collect();
    let request_all_metrics = request.metric_ids.is_empty();
    let result_artifact_candidate =
        default_result_artifact_scope(&request.query_state, &request.filter_intents);

    let needs_bulk = request_needs_bulk_l1_metrics(&requested, request_all_metrics);

    for lookup_cache_key in &lookup_cache_keys {
        if let Some(cached) =
            take_cached_metric_response(lookup_cache_key.as_str(), &requested, request_all_metrics)
        {
            let _ = store_cached_metric_response_aliases(
                &lookup_cache_keys,
                cached.total_rows,
                &cached.metrics_map,
                &cached.covered_metric_ids,
                cached.complete,
            );
            return Ok(build_outcome_from_cached(
                request,
                dependency_revision_key.as_str(),
                lookup_cache_key.as_str(),
                started,
                cached.total_rows,
                &cached.metrics_map,
                "memory",
                true,
                false,
            ));
        }
    }

    if needs_bulk {
        for lookup_cache_key in &lookup_cache_keys {
            if let Some(cached) = take_demand_metric_response(
                lookup_cache_key.as_str(),
                &requested,
                request_all_metrics,
            ) {
                let _ = populate_l1_from_loaded_metric_artifact(
                    &lookup_cache_keys,
                    &LoadedMetricResponseArtifact {
                        total_rows: cached.total_rows,
                        metrics_map: (*cached.metrics_map).clone(),
                        covered_metric_ids: cached.covered_metric_ids.clone(),
                        complete: cached.complete,
                    },
                );
                return Ok(build_outcome_from_cached(
                    request,
                    dependency_revision_key.as_str(),
                    lookup_cache_key.as_str(),
                    started,
                    cached.total_rows,
                    &cached.metrics_map,
                    "demand",
                    true,
                    false,
                ));
            }
        }
    }

    if result_artifact_candidate {
        // Shared Pack-First resolver: lite (non-bulk) then full — never hydrate on hit.
        if let Some(hit) = try_load_disk_metric_response(
            app_root.as_path(),
            &lookup_cache_keys,
            &requested,
            request_all_metrics,
            !needs_bulk,
        )? {
            let _ = populate_l1_from_loaded_metric_artifact(&lookup_cache_keys, &hit.artifact);
            if needs_bulk {
                let demand_map: BTreeMap<String, MetricContract> = hit
                    .artifact
                    .metrics_map
                    .iter()
                    .filter(|(metric_id, _)| request_all_metrics || requested.contains(*metric_id))
                    .map(|(metric_id, contract)| (metric_id.clone(), contract.clone()))
                    .collect();
                store_demand_metric_response(
                    &lookup_cache_keys,
                    hit.artifact.total_rows,
                    &demand_map,
                    &requested,
                    hit.artifact.complete,
                );
            }
            let perf_key = if hit.source == "lite" {
                "result_artifact_lite_load_ms"
            } else {
                "result_artifact_load_ms"
            };
            let query_perf = BTreeMap::from([(perf_key.to_string(), hit.load_ms)]);
            let metrics = project_requested_metrics(
                request.owner_resource_id.as_str(),
                &request.metric_ids,
                &BTreeMap::new(),
                &hit.artifact.metrics_map,
            );
            let memory_ready = !needs_bulk;
            let descriptors = build_descriptors_for_metrics(
                request,
                dependency_revision_key.as_str(),
                hit.cache_key.as_str(),
                hit.load_ms,
                true,
                "disk",
                CacheLayersReady {
                    disk: true,
                    memory: memory_ready,
                    client: false,
                },
            );
            return Ok(EvalPipelineOutcome {
                descriptors,
                cache_key: hit.cache_key,
                artifact_hit: true,
                cache_layer: "disk".to_string(),
                result_artifact_hit: true,
                wall_ms: started.elapsed().as_millis() as u64,
                metrics,
                total_rows: hit.artifact.total_rows,
                query_perf,
            });
        }
    }

    let mut eval = match evaluate_runtime_metrics(
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
    // Pack-First: non-bulk KPI paths write lite only. Full packs (with optional
    // rowsets) require bulk request or MEI_DUAL_WRITE_FULL_METRIC_RESPONSE=1.
    let map_has_rowsets = eval
        .metrics_map
        .keys()
        .any(|metric_id| metric_id_is_scalar_rowset(metric_id));
    if needs_bulk || map_has_rowsets {
        store_metric_response_result_artifact(
            app_root.as_path(),
            cache_key.as_str(),
            eval.total_rows,
            &eval.metrics_map,
            &requested,
            requested.len() == request.metric_ids.len(),
        )?;
    } else {
        store_metric_response_lite_only(
            app_root.as_path(),
            cache_key.as_str(),
            eval.total_rows,
            &eval.metrics_map,
            &requested,
            requested.len() == request.metric_ids.len(),
        )?;
    }
    if needs_bulk {
        // Demand cache may hold rowsets, but only the requested subset — never the
        // full frontier-expanded metrics_map working set.
        let demand_map: BTreeMap<String, MetricContract> = eval
            .metrics_map
            .iter()
            .filter(|(metric_id, _)| requested.contains(*metric_id))
            .map(|(metric_id, contract)| (metric_id.clone(), contract.clone()))
            .collect();
        store_demand_metric_response(
            &lookup_cache_keys,
            eval.total_rows,
            &demand_map,
            &requested,
            requested.len() == request.metric_ids.len(),
        );
    }
    // After Disk dual-write, collapse the in-process map to L1 shape so KPI
    // request paths do not keep frontier rowsets as residents.
    if !needs_bulk {
        let covered: BTreeSet<String> = eval.metrics_map.keys().cloned().collect();
        let (projected, _, _) =
            project_metrics_map_for_l1(&eval.metrics_map, &covered, &L1PinPolicy::default());
        eval.metrics_map = projected;
    }
    let _ = store_cached_metric_response(
        cache_key.clone(),
        eval.total_rows,
        &eval.metrics_map,
        &requested,
        requested.len() == request.metric_ids.len(),
    );
    let wall_ms = started.elapsed().as_millis() as u64;
    let memory_ready = request
        .metric_ids
        .iter()
        .all(|metric_id| !metric_id_is_scalar_rowset(metric_id))
        && !request_all_metrics;
    let descriptors = build_descriptors_for_metrics(
        request,
        dependency_revision_key.as_str(),
        cache_key.as_str(),
        wall_ms,
        false,
        "compute",
        CacheLayersReady {
            disk: true,
            memory: memory_ready,
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
