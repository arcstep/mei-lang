use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use mei_host_core::HostContext;
use mei_host_graph::{record_access, record_slots_from_descriptors, MrgAccessKind};
use mei_lang_datasets::{
    map_dataset_query_filters, normalize_query_filters, normalize_query_search, query_dataset_rows,
    query_metric_dataframe, query_state_from_request, DatasetQueryOptions,
};
use mei_lang_kernel::{locate_dataset_resource, FilterIntent, MetricContract, QueryState};

use crate::eval_pipeline::{eval_metrics_with_slots, EvalPipelineRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
struct DatasetQueryRequest {
    #[serde(default)]
    scene_id: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    preview_scope: Option<String>,
    dataset_id: String,
    #[serde(default)]
    page: Option<usize>,
    #[serde(default)]
    page_size: Option<usize>,
    #[serde(default)]
    search: Option<String>,
    #[serde(default)]
    filters: BTreeMap<String, String>,
    #[serde(default)]
    query_state: Option<QueryState>,
    #[serde(default)]
    filter_intents: Vec<FilterIntent>,
    #[serde(default)]
    metric_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MetricQueryRequest {
    #[serde(default)]
    scene_id: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    preview_scope: Option<String>,
    #[serde(default)]
    dataset_id: String,
    #[serde(default)]
    metric_ids: Vec<String>,
    #[serde(default)]
    metric_groups: Vec<MetricQueryGroupRequest>,
    #[serde(default)]
    search: Option<String>,
    #[serde(
        default,
        deserialize_with = "mei_lang_datasets::serde_lenient::string_map"
    )]
    filters: BTreeMap<String, String>,
    #[serde(default)]
    query_state: Option<QueryState>,
    #[serde(default)]
    filter_intents: Vec<FilterIntent>,
}

#[derive(Debug, Clone, Deserialize)]
struct MetricQueryGroupRequest {
    dataset_id: String,
    #[serde(default)]
    metric_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MetricQueryGroupResponse {
    dataset_id: String,
    total_rows: usize,
    metrics: Vec<MetricContract>,
    perf: BTreeMap<String, u64>,
}

#[derive(Debug, Serialize)]
struct MetricQueryResponse {
    scene_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    scene_path: Option<String>,
    dataset_id: String,
    total_rows: usize,
    metrics: Vec<MetricContract>,
    perf: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    groups: Vec<MetricQueryGroupResponse>,
}

pub fn query_dataset(ctx: &HostContext, body: &Value) -> Result<Value> {
    let started = Instant::now();
    let request: DatasetQueryRequest =
        serde_json::from_value(body.clone()).context("parse dataset query request")?;
    enforce_dev_eval_scope(
        ctx,
        request.preview_scope.as_deref(),
        "dataset query",
        request
            .metric_id
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .as_slice(),
    )?;
    let scene_id = scene_id_from_request(request.scene_id.as_deref());
    let target = request.target.as_deref().filter(|value| !value.is_empty());
    info!(
        app_id = %ctx.app_id,
        scene_id = %scene_id,
        target = target.unwrap_or("-"),
        dataset_id = %request.dataset_id,
        metric_id = request.metric_id.as_deref().unwrap_or("-"),
        "dataset query started"
    );
    let outcome = mei_host_graph::assemble_scope_from_registry(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
        scene_id,
    )?
    .ok_or_else(|| anyhow!("scene `{scene_id}` not assembled"))?;
    let compiled = &outcome.compiled;
    let normalized_search = normalize_query_search(request.search.as_deref());
    let normalized_filters = normalize_query_filters(&request.filters);
    let effective_query_state = query_state_from_request(
        &normalized_filters,
        normalized_search.as_deref(),
        request.query_state.as_ref(),
    );
    let options = dataset_options_from_request(&request, &effective_query_state);
    let app_root = ctx.app_root();
    let result = if let Some(metric_id) = request
        .metric_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_metric_dataframe(
            compiled,
            app_root.as_path(),
            request.dataset_id.trim(),
            metric_id,
            Some(scene_id),
            target,
            outcome.compile_revision.as_str(),
            options,
            Some(effective_query_state.clone()),
            request.filter_intents.clone(),
        )?
    } else {
        let resource = locate_dataset_resource(compiled, request.dataset_id.trim())
            .map_err(|error| anyhow!("{error}"))?;
        let dataset = resource
            .dataset
            .as_ref()
            .ok_or_else(|| anyhow!("resource `{}` is not a dataset", resource.id))?;
        let mut row_options = options;
        row_options.filters = map_dataset_query_filters(&effective_query_state, dataset);
        query_dataset_rows(app_root.as_path(), dataset, row_options)?
    };

    let latency_ms = started.elapsed().as_millis() as u64;
    info!(
        app_id = %ctx.app_id,
        scene_id = %scene_id,
        dataset_id = %request.dataset_id,
        total = result.total,
        latency_ms,
        "dataset query finished"
    );
    let mut perf = result.perf.clone();
    perf.insert("total_ms".to_string(), latency_ms);
    Ok(json!({
        "scene_id": scene_id,
        "scene_path": target,
        "dataset_id": request.dataset_id.trim(),
        "metric_id": request.metric_id,
        "page": result.page,
        "page_size": result.page_size,
        "total": result.total,
        "has_more": result.has_more,
        "columns": result.columns,
        "rows": result.rows,
        "lazy": result.lazy,
        "perf": perf,
        "column_meta": result.column_meta,
        "summary": result.summary,
    }))
}

pub fn query_metrics(ctx: &HostContext, body: &Value) -> Result<Value> {
    let started = Instant::now();
    let request: MetricQueryRequest =
        serde_json::from_value(body.clone()).context("parse metric query request")?;
    let groups = normalize_metric_query_groups(&request)?;
    let requested_metric_ids = groups
        .iter()
        .flat_map(|group| group.metric_ids.iter().cloned())
        .collect::<Vec<_>>();
    enforce_dev_eval_scope(
        ctx,
        request.preview_scope.as_deref(),
        "metric query",
        requested_metric_ids.as_slice(),
    )?;
    let scene_id = scene_id_from_request(request.scene_id.as_deref());
    let target = request
        .target
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let metric_count: usize = groups.iter().map(|group| group.metric_ids.len()).sum();
    info!(
        app_id = %ctx.app_id,
        scene_id = %scene_id,
        target = target.as_deref().unwrap_or("-"),
        metric_group_count = groups.len(),
        metric_count,
        "metric query started"
    );
    let outcome = mei_host_graph::assemble_scope_from_registry(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
        scene_id,
    )?
    .ok_or_else(|| anyhow!("scene `{scene_id}` not assembled"))?;
    let normalized_search = normalize_query_search(request.search.as_deref());
    let normalized_filters = normalize_query_filters(&request.filters);
    let effective_query_state = query_state_from_request(
        &normalized_filters,
        normalized_search.as_deref(),
        request.query_state.as_ref(),
    );

    if groups.len() == 1 {
        let group = execute_metric_group(
            ctx,
            &outcome.compiled,
            outcome.compile_revision.as_str(),
            &groups[0],
            scene_id,
            target.as_deref(),
            &effective_query_state,
            &request.filter_intents,
        )?;
        let latency_ms = started.elapsed().as_millis() as u64;
        info!(
            app_id = %ctx.app_id,
            scene_id = %scene_id,
            dataset_id = %group.dataset_id,
            metric_count = group.metrics.len(),
            latency_ms,
            "metric query finished"
        );
        return Ok(serde_json::to_value(MetricQueryResponse {
            scene_id: scene_id.to_string(),
            scene_path: target,
            dataset_id: group.dataset_id,
            total_rows: group.total_rows,
            metrics: group.metrics,
            perf: perf_with_total(group.perf, latency_ms),
            groups: Vec::new(),
        })?);
    }

    let mut batch_groups = Vec::new();
    for group in &groups {
        match execute_metric_group(
            ctx,
            &outcome.compiled,
            outcome.compile_revision.as_str(),
            group,
            scene_id,
            target.as_deref(),
            &effective_query_state,
            &request.filter_intents,
        ) {
            Ok(response) => batch_groups.push(response),
            Err(error) => {
                warn!(
                    app_id = %ctx.app_id,
                    scene_id = %scene_id,
                    dataset_id = %group.dataset_id,
                    error = %error,
                    "metric batch group failed"
                );
                return Err(error);
            }
        }
    }
    let latency_ms = started.elapsed().as_millis() as u64;
    info!(
        app_id = %ctx.app_id,
        scene_id = %scene_id,
        metric_group_count = batch_groups.len(),
        latency_ms,
        "metric batch query finished"
    );
    Ok(serde_json::to_value(MetricQueryResponse {
        scene_id: scene_id.to_string(),
        scene_path: target,
        dataset_id: "__scene_batch__".to_string(),
        total_rows: batch_groups
            .iter()
            .map(|group| group.total_rows)
            .max()
            .unwrap_or(0),
        metrics: Vec::new(),
        perf: BTreeMap::from([("total_ms".to_string(), latency_ms)]),
        groups: batch_groups,
    })?)
}

fn execute_metric_group(
    ctx: &HostContext,
    compiled: &mei_lang_kernel::CompiledApp,
    compile_revision: &str,
    group: &MetricQueryGroupRequest,
    scene_id: &str,
    target: Option<&str>,
    query_state: &QueryState,
    filter_intents: &[FilterIntent],
) -> Result<MetricQueryGroupResponse> {
    let started = Instant::now();
    let bundle_key = bundle_key_for_dataset(group.dataset_id.as_str());
    let workset_id = format!("jit:{scene_id}:{}", group.dataset_id.trim());
    let pipeline = eval_metrics_with_slots(
        ctx,
        compiled,
        compile_revision,
        &EvalPipelineRequest {
            scope_key: scene_id.to_string(),
            target: target.map(str::to_string),
            owner_resource_id: group.dataset_id.trim().to_string(),
            metric_ids: group.metric_ids.clone(),
            workset_id: workset_id.clone(),
            bundle_key,
            query_state: query_state.clone(),
            filter_intents: filter_intents.to_vec(),
        },
    )?;
    record_access(MrgAccessKind::MetricsApi, pipeline.artifact_hit);
    if !pipeline.artifact_hit {
        mei_lang_datasets::record_scope_cache_miss(scene_id);
        crate::smart_warmup::maybe_trigger_smart_warmup(ctx, scene_id);
    }
    if let Err(error) = record_slots_from_descriptors(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
        &pipeline.descriptors,
    ) {
        warn!(
            app_id = %ctx.app_id,
            scene_id = %scene_id,
            cache_key = %pipeline.cache_key,
            error = %error,
            "failed to record MRG slots after metric query"
        );
    }
    crate::client_bootstrap_refresh::maybe_refresh_client_bootstrap_after_eval(
        ctx,
        scene_id,
        workset_id.as_str(),
        &pipeline,
        query_state,
        filter_intents,
    );
    let mut perf = pipeline.query_perf.clone();
    if pipeline.artifact_hit {
        perf.insert("cache_layer".to_string(), 1);
    } else {
        perf.insert("metric_eval_ms".to_string(), pipeline.wall_ms);
    }
    if pipeline.result_artifact_hit {
        perf.insert("result_artifact_hit".to_string(), 1);
    }
    perf.insert("total_ms".to_string(), started.elapsed().as_millis() as u64);
    Ok(MetricQueryGroupResponse {
        dataset_id: group.dataset_id.clone(),
        total_rows: pipeline.total_rows,
        metrics: pipeline.metrics,
        perf,
    })
}

fn bundle_key_for_dataset(dataset_id: &str) -> String {
    dataset_id
        .strip_prefix("__world_metrics__::")
        .unwrap_or("")
        .to_string()
}

fn normalize_metric_query_groups(
    request: &MetricQueryRequest,
) -> Result<Vec<MetricQueryGroupRequest>> {
    let mut groups = if request.metric_groups.is_empty() {
        vec![MetricQueryGroupRequest {
            dataset_id: request.dataset_id.trim().to_string(),
            metric_ids: request.metric_ids.clone(),
        }]
    } else {
        request.metric_groups.clone()
    };
    if groups.is_empty() {
        anyhow::bail!("metric query requires at least one dataset binding");
    }
    for group in &mut groups {
        group.dataset_id = group.dataset_id.trim().to_string();
        if group.dataset_id.is_empty() {
            anyhow::bail!("metric query requires non-empty dataset_id");
        }
        group.metric_ids = group
            .metric_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        group.metric_ids.sort();
        group.metric_ids.dedup();
    }
    Ok(groups)
}

fn dataset_options_from_request(
    request: &DatasetQueryRequest,
    query_state: &QueryState,
) -> DatasetQueryOptions {
    DatasetQueryOptions {
        page: request.page.unwrap_or(1),
        page_size: request.page_size.unwrap_or(0),
        search: query_state.search.clone(),
        filters: query_state.filters.clone(),
        group: query_state.group.clone(),
        time_range: query_state.time_range.clone(),
        collect_all: false,
        sort: Vec::new(),
        column_state: None,
        summary: false,
    }
}

fn scene_id_from_request(scene_id: Option<&str>) -> &str {
    scene_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home")
}

fn perf_with_total(mut perf: BTreeMap<String, u64>, total_ms: u64) -> BTreeMap<String, u64> {
    perf.insert("total_ms".to_string(), total_ms);
    perf
}

pub(crate) fn enforce_dev_eval_scope(
    ctx: &HostContext,
    preview_scope: Option<&str>,
    request_kind: &str,
    metric_ids: &[String],
) -> Result<()> {
    let gate = mei_lang_kernel::RuntimeDevEvalGate::resolve_for_app(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
    );
    let decisions = if metric_ids.is_empty() {
        vec![gate.decide_scope(preview_scope)]
    } else {
        metric_ids
            .iter()
            .map(|metric_id| gate.decide_metric(Some(metric_id.as_str()), preview_scope))
            .collect::<Vec<_>>()
    };
    if decisions.iter().all(|decision| decision.accepted) {
        return Ok(());
    }
    let decision = decisions
        .iter()
        .find(|decision| !decision.accepted)
        .copied()
        .expect("rejected dev eval decision");
    warn!(
        app_id = %ctx.app_id,
        profile = gate.profile.slug(),
        preview_scope = preview_scope.unwrap_or("-"),
        metric_ids = %metric_ids.join(","),
        reason = decision.reason,
        "{request_kind} rejected by dev eval gate"
    );
    anyhow::bail!(
        "{request_kind} rejected by dev eval gate: reason={} preview_scope={}",
        decision.reason,
        preview_scope.unwrap_or("-")
    )
}
