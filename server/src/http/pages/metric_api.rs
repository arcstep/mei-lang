use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use mei_lang_kernel::{
    evaluate_runtime_metric_defs_with_scope_and_dag, resolve_runtime_metric_def_key,
    runtime_eval_node_cache_enabled, EvalPlanNodeKind, FilterIntent, MetricContract, QueryState,
};
use serde::{Deserialize, Serialize};

use crate::http::observation::{CompileObservation, EvalObservation};
use crate::{AppError, AppState};

use super::super::compile_cache::compile_app_with_cache;
use super::super::datasets::{
    eval_node_cache_key, hydrate_file_backed_datasets_for_metric_defs,
    metric_request_revision_fingerprint, metric_scope_cache_key, normalize_query_filters,
    normalize_query_search, plan_access_metric_eval_for_ids, query_dataset_rows,
    query_state_from_request, runtime_metric_eval_scope, runtime_metric_workset,
    serialize_cache_value, DatasetQueryOptions,
};
use super::components::resolve_components_root;
use super::scene_qualified::{
    compile_options_from_coords, locate_dataset_resource, resolved_scene_context,
    strict_runtime_query_contract, strict_scene_query_coords,
};
use super::util::elapsed_ms;

const METRIC_RESPONSE_CACHE_TTL_MS: u64 = 30_000;

#[derive(Debug, Clone)]
struct CachedMetricResponse {
    expires_at: Instant,
    total_rows: usize,
    metrics_map: BTreeMap<String, MetricContract>,
}

fn metric_response_cache() -> &'static Mutex<BTreeMap<String, CachedMetricResponse>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, CachedMetricResponse>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn metric_response_cache_ttl() -> Duration {
    Duration::from_millis(METRIC_RESPONSE_CACHE_TTL_MS)
}

fn hash_fingerprint(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn metric_eval_diagnostic_code(message: &str) -> &'static str {
    if message.contains("cyclic_eval_dependency")
        || message.contains("metric_eval_recursion_guard_tripped")
    {
        "metric_eval_recursion_guard_tripped"
    } else {
        "metric_eval_failed"
    }
}

fn runtime_metric_scope_requested(
    query_state: &QueryState,
    filter_intents: &[FilterIntent],
) -> bool {
    !query_state.filters.is_empty()
        || query_state
            .search
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        || !query_state.group.is_empty()
        || query_state.time_range.is_some()
        || !filter_intents.is_empty()
}

fn metric_response_cache_key(
    app_id: &str,
    scene_id: &str,
    scene_path: Option<&str>,
    dataset_id: &str,
    metric_scope_key: &str,
    query: &DatasetQueryOptions,
    compile_revision: &str,
    dependency_revision_key: &str,
) -> String {
    let group = serialize_cache_value(&query.group);
    let time_range = serialize_cache_value(&query.time_range);
    format!(
        "{app_id}|compile={compile_revision}|{dependency_revision_key}|scene={scene_id}|target={}|dataset={dataset_id}|metric_ids={metric_scope_key}|search={}|filters={}|group={}|time_range={}",
        scene_path.unwrap_or(""),
        query.search.as_deref().unwrap_or(""),
        serialize_cache_value(&query.filters),
        group,
        time_range
    )
}

fn take_cached_metric_response(key: &str) -> Option<CachedMetricResponse> {
    let Ok(mut cache) = metric_response_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    cache.get(key).cloned()
}

fn store_cached_metric_response(
    key: String,
    total_rows: usize,
    metrics_map: &BTreeMap<String, MetricContract>,
) {
    let Ok(mut cache) = metric_response_cache().lock() else {
        return;
    };
    cache.retain(|_, entry| entry.expires_at > Instant::now());
    cache.insert(
        key,
        CachedMetricResponse {
            expires_at: Instant::now() + metric_response_cache_ttl(),
            total_rows,
            metrics_map: metrics_map.clone(),
        },
    );
}

fn project_requested_metrics(
    resource_id: &str,
    request_metric_ids: &[String],
    runtime_metric_defs: &BTreeMap<String, serde_json::Value>,
    metrics_map: &BTreeMap<String, MetricContract>,
) -> Vec<MetricContract> {
    if request_metric_ids.is_empty() {
        return metrics_map.values().cloned().collect();
    }
    request_metric_ids
        .iter()
        .filter_map(|metric_id| {
            let resolved =
                resolve_runtime_metric_def_key(resource_id, metric_id, runtime_metric_defs)?;
            let mut metric = metrics_map.get(&resolved)?.clone();
            metric.id = metric_id.clone();
            Some(metric)
        })
        .collect()
}

pub(crate) fn clear_metric_response_cache() -> usize {
    let Ok(mut cache) = metric_response_cache().lock() else {
        return 0;
    };
    let removed = cache.len();
    cache.clear();
    removed
}

#[derive(Debug, Deserialize)]
pub struct MetricQueryRequest {
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    pub dataset_id: String,
    #[serde(default)]
    pub metric_ids: Vec<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub filters: BTreeMap<String, String>,
    #[serde(default)]
    pub query_state: Option<QueryState>,
    #[serde(default)]
    pub filter_intents: Vec<FilterIntent>,
}

#[derive(Debug, Serialize)]
pub struct MetricQueryResponse {
    pub scene_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_path: Option<String>,
    pub dataset_id: String,
    pub total_rows: usize,
    pub metrics: Vec<MetricContract>,
    pub perf: BTreeMap<String, u64>,
}

pub async fn dataset_metric_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Json(request): Json<MetricQueryRequest>,
) -> Result<Json<MetricQueryResponse>, AppError> {
    let request_started = Instant::now();
    let app_id = app_id_raw.trim_start_matches('/').to_string();
    if app_id.is_empty() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "missing app id in route",
        ));
    }
    strict_runtime_query_contract(
        &request.filters,
        request.search.as_deref(),
        request.query_state.as_ref(),
        &request.filter_intents,
        "metric query",
    )?;
    let requested_scene_id = request
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let requested_target = request
        .target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let requested_dataset_id = request.dataset_id.trim().to_string();
    let requested_metric_ids = if request.metric_ids.is_empty() {
        "-".to_string()
    } else {
        request
            .metric_ids
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(",")
    };
    let request_span = tracing::info_span!(
        "dataset_metric_api",
        app_id = %app_id,
        scene_id = %requested_scene_id,
        target = %requested_target,
        dataset_id = %requested_dataset_id,
        metric_ids = %requested_metric_ids
    );
    let _request_span_guard = request_span.enter();
    tracing::info!("metric query started");
    let coords = strict_scene_query_coords(
        request.scene_id.clone(),
        request.target.clone(),
        "metric query",
    )?;
    let compile_options = compile_options_from_coords(&coords);
    let components_root = resolve_components_root(&state.source_root);
    let compile_outcome =
        compile_app_with_cache(&state, &app_id, compile_options, components_root.as_path())
            .map_err(|failure| {
                tracing::warn!(
                    app_id = %app_id,
                    scene_id = %requested_scene_id,
                    target = %requested_target,
                    dataset_id = %requested_dataset_id,
                    metric_ids = %requested_metric_ids,
                    phase = "compile",
                    error = %failure.error,
                    cache_lookup_ms = failure.cache_lookup_ms,
                    compile_cache_lock_wait_ms = failure.compile_cache_lock_wait_ms,
                    compile_ms = failure.compile_ms,
                    "metric query compile failed"
                );
                AppError::from(failure.error)
            })?;
    let compile_ms = compile_outcome.compile_ms;
    let scene_ctx = resolved_scene_context(&compile_outcome.compiled);
    let compile_observation = CompileObservation::from_compile_outcome(
        &app_id,
        &scene_ctx.scene_id,
        scene_ctx.scene_path.as_deref(),
        &compile_outcome,
    );
    let compiled = compile_outcome.compiled;
    let normalized_dataset_id = request.dataset_id.trim();
    let resource = locate_dataset_resource(
        &compiled,
        normalized_dataset_id,
        coords
            .scene_id
            .as_deref()
            .or(Some(scene_ctx.scene_id.as_str())),
    )
    .map_err(|error| {
        tracing::warn!(
            app_id = %app_id,
            scene_id = %requested_scene_id,
            target = %requested_target,
            dataset_id = %requested_dataset_id,
            metric_ids = %requested_metric_ids,
            phase = "locate_dataset",
            error = ?error,
            "metric query locate failed"
        );
        AppError::from(error)
    })?;
    let locate_started = Instant::now();
    let locate_dataset_ms = elapsed_ms(locate_started);
    let dataset = resource.dataset.as_ref().ok_or_else(|| {
        AppError::status(
            StatusCode::BAD_REQUEST,
            format!("resource `{}` is not a dataset", resource.id),
        )
    })?;
    let normalized_search = normalize_query_search(request.search.as_deref());
    let normalized_filters = normalize_query_filters(&request.filters);
    let effective_query_state = query_state_from_request(
        &normalized_filters,
        normalized_search.as_deref(),
        request.query_state.as_ref(),
    );
    if !dataset.has_runtime_metric_defs() && dataset.uses_compiled_metric_snapshot_only() {
        if runtime_metric_scope_requested(&effective_query_state, &request.filter_intents) {
            tracing::warn!(
                app_id = %app_id,
                scene_id = %scene_ctx.scene_id,
                target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
                dataset_id = %resource.id,
                metric_ids = %requested_metric_ids,
                phase = "metric_defs",
                "metric query refused compile-time snapshot fallback for scoped request"
            );
            return Err(AppError::status(
                StatusCode::BAD_REQUEST,
                format!(
                    "dataset `{}` only exposes compile-time metric snapshots; scoped runtime metric queries require runtime_metric_defs",
                    resource.id
                ),
            ));
        }
        // Static fallback only applies when the dataset exposes compile-time
        // metric snapshots but no runtime-authoritative defs to re-evaluate.
        let metrics = if request.metric_ids.is_empty() {
            dataset.metrics.values().cloned().collect::<Vec<_>>()
        } else {
            request
                .metric_ids
                .iter()
                .filter_map(|metric_id| dataset.metrics.get(metric_id).cloned())
                .collect::<Vec<_>>()
        };
        let mut perf = BTreeMap::new();
        compile_observation.write_perf(&mut perf);
        perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
        perf.insert("compat_compiled_metric_snapshot_fallback".to_string(), 1);
        let total_ms = elapsed_ms(request_started);
        perf.insert("total_ms".to_string(), total_ms);
        tracing::info!(
            app_id = %app_id,
            scene_id = %scene_ctx.scene_id,
            target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
            dataset_id = %resource.id,
            metric_ids = %requested_metric_ids,
            compile_cache_hit = compile_outcome.cache_hit,
            compile_ms,
            total_rows = 0,
            total_ms,
            "metric query finished (dataset static metrics fallback)"
        );
        return Ok(Json(MetricQueryResponse {
            scene_id: scene_ctx.scene_id,
            scene_path: scene_ctx.scene_path,
            dataset_id: resource.id.clone(),
            total_rows: 0,
            metrics,
            perf,
        }));
    }
    let access_plan =
        plan_access_metric_eval_for_ids(&compiled, normalized_dataset_id, &request.metric_ids)
            .map_err(|error| {
                tracing::warn!(
                    app_id = %app_id,
                    scene_id = %scene_ctx.scene_id,
                    target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
                    dataset_id = %requested_dataset_id,
                    metric_ids = %requested_metric_ids,
                    phase = "metric_defs",
                    error = %error,
                    "metric query runtime metric plan failed"
                );
                AppError::status(StatusCode::BAD_REQUEST, error.to_string())
            })?;
    let primary_dataset = access_plan.primary_dataset;
    let owner_dataset = access_plan.owner_dataset;
    let app_root = state.source_root.join(&app_id);
    let query = DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: effective_query_state.search.clone(),
        filters: effective_query_state.filters.clone(),
        group: effective_query_state.group.clone(),
        time_range: effective_query_state.time_range.clone(),
        collect_all: true,
        ..DatasetQueryOptions::default()
    };
    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .fold(BTreeMap::new(), |mut acc, (resource_id, dataset)| {
            acc.insert(resource_id, dataset.clone());
            acc.entry(dataset.id.clone()).or_insert(dataset);
            acc
        });
    let workset = runtime_metric_workset(
        &access_plan.owner.id,
        &access_plan.request_metric_ids,
        owner_dataset,
    );
    let metric_ids = workset.eval_metric_ids.as_deref();
    let defs_for_hydrate = workset.defs_for_hydrate.clone();
    let dependency_revision_key = metric_request_revision_fingerprint(
        &app_root,
        &datasets,
        &access_plan.owner.id,
        &defs_for_hydrate,
    );
    let response_cache_key = metric_response_cache_key(
        &app_id,
        &scene_ctx.scene_id,
        scene_ctx.scene_path.as_deref(),
        &access_plan.owner.id,
        &metric_scope_cache_key(if request.metric_ids.is_empty() {
            &workset.resolved_metric_ids
        } else {
            workset
                .eval_metric_ids
                .as_deref()
                .unwrap_or(workset.closure_metric_ids.as_slice())
        }),
        &query,
        &compile_outcome.compile_revision,
        &dependency_revision_key,
    );
    let response_cache_lookup_started = Instant::now();
    if let Some(cached) = take_cached_metric_response(&response_cache_key) {
        let mut perf = BTreeMap::new();
        compile_observation.write_perf(&mut perf);
        perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
        let mut eval_observation = EvalObservation::new(true)
            .with_response_cache_key_hash(hash_fingerprint(&response_cache_key));
        eval_observation.insert_counter("request_dag_observed", 0);
        eval_observation.insert_counter("eval_memo_hits", 0);
        eval_observation.insert_counter("eval_memo_eval_node_cache_hits", 0);
        eval_observation.insert_counter("eval_memo_eval_node_cache_misses", 0);
        eval_observation.write_perf(&mut perf);
        perf.insert(
            "response_cache_lookup_ms".to_string(),
            elapsed_ms(response_cache_lookup_started),
        );
        let total_ms = elapsed_ms(request_started);
        perf.insert("total_ms".to_string(), total_ms);
        return Ok(Json(MetricQueryResponse {
            scene_id: scene_ctx.scene_id,
            scene_path: scene_ctx.scene_path,
            dataset_id: resource.id.clone(),
            total_rows: cached.total_rows,
            metrics: project_requested_metrics(
                &access_plan.owner.id,
                &access_plan.request_metric_ids,
                &owner_dataset.runtime_metric_defs,
                &cached.metrics_map,
            ),
            perf,
        }));
    }
    let query_started = Instant::now();
    let filtered_rows =
        query_dataset_rows(&app_root, primary_dataset, query.clone()).map_err(|error| {
            tracing::warn!(
                app_id = %app_id,
                scene_id = %scene_ctx.scene_id,
                target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
                dataset_id = %resource.id,
                metric_ids = %requested_metric_ids,
                phase = "query_dataset_rows",
                error = %error,
                "metric query rows failed"
            );
            AppError::from(error)
        })?;
    let query_ms = elapsed_ms(query_started);
    let mut runtime_dataset = primary_dataset.clone();
    runtime_dataset.rows = filtered_rows.rows.clone();
    if !filtered_rows.columns.is_empty() {
        runtime_dataset.columns = filtered_rows.columns.clone();
    }
    datasets.insert(access_plan.primary.id.clone(), runtime_dataset.clone());
    let hydrate_started = Instant::now();
    let hydrate_perf = hydrate_file_backed_datasets_for_metric_defs(
        &app_root,
        &mut datasets,
        &defs_for_hydrate,
        &query,
    )
    .map_err(|error| {
        tracing::warn!(
            app_id = %app_id,
            scene_id = %scene_ctx.scene_id,
            target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
            dataset_id = %resource.id,
            metric_ids = %requested_metric_ids,
            phase = "hydrate_bindings",
            error = %error,
            "metric query hydrate binding validation failed"
        );
        AppError::status(StatusCode::BAD_REQUEST, error.to_string())
    })?;
    let hydrate_ms = elapsed_ms(hydrate_started);
    let metric_started = Instant::now();
    let eval_scope = runtime_metric_eval_scope(
        Some(primary_dataset),
        &access_plan.primary.id,
        &scene_ctx.scene_id,
        scene_ctx.scene_path.as_deref(),
        effective_query_state.search.as_deref(),
        &effective_query_state.filters,
        Some(&effective_query_state),
        &request.filter_intents,
        &dependency_revision_key,
    )
    .map_err(|error| {
        tracing::warn!(
            app_id = %app_id,
            scene_id = %scene_ctx.scene_id,
            target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
            dataset_id = %resource.id,
            metric_ids = %requested_metric_ids,
            phase = "scope_binding",
            error = %error,
            "metric query scope binding validation failed"
        );
        AppError::status(StatusCode::BAD_REQUEST, error.to_string())
    })?;
    let (metrics_map, eval_report) = evaluate_runtime_metric_defs_with_scope_and_dag(
        &owner_dataset.runtime_metric_defs,
        &runtime_dataset.rows,
        &datasets,
        metric_ids,
        &eval_scope,
    )
    .map_err(|error| {
        let error_text = error.to_string();
        let diagnostic_code = metric_eval_diagnostic_code(&error_text);
        tracing::warn!(
            app_id = %app_id,
            scene_id = %scene_ctx.scene_id,
            target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
            dataset_id = %resource.id,
            metric_ids = %requested_metric_ids,
            diagnostic_code,
            phase = "metric_eval",
            error = %error,
            "metric query evaluate runtime metric defs failed"
        );
        AppError::from(error)
    })?;
    let metric_eval_ms = elapsed_ms(metric_started);
    let dag_metrics = &eval_report.request_dag_metrics;
    let eval_plan = &eval_report.eval_plan;
    let metrics = if request.metric_ids.is_empty() {
        metrics_map.values().cloned().collect::<Vec<_>>()
    } else {
        project_requested_metrics(
            &access_plan.owner.id,
            &access_plan.request_metric_ids,
            &owner_dataset.runtime_metric_defs,
            &metrics_map,
        )
    };
    let mut perf = filtered_rows.perf.clone();
    let eval_scope_key = eval_node_cache_key("metric_scope", &eval_scope);
    compile_observation.write_perf(&mut perf);
    perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
    let mut eval_observation = EvalObservation::new(false)
        .with_response_cache_key_hash(hash_fingerprint(&response_cache_key));
    eval_observation.insert_counter("request_dag_observed", 1);
    eval_observation.insert_counter("eval_memo_hits", dag_metrics.request_cache_hits);
    eval_observation.insert_counter(
        "eval_memo_eval_node_cache_hits",
        dag_metrics.eval_node_cache_hits,
    );
    eval_observation.insert_counter(
        "eval_memo_eval_node_cache_misses",
        dag_metrics.eval_node_cache_misses,
    );
    eval_observation.write_perf(&mut perf);
    perf.insert(
        "response_cache_lookup_ms".to_string(),
        elapsed_ms(response_cache_lookup_started),
    );
    perf.insert("query_api_ms".to_string(), query_ms);
    perf.insert("hydrate_datasets_ms".to_string(), hydrate_ms);
    perf.insert("compat_compiled_metric_snapshot_fallback".to_string(), 0);
    for (key, value) in hydrate_perf {
        perf.insert(key, value);
    }
    perf.insert(
        "eval_plan_targets".to_string(),
        eval_plan.targets.len() as u64,
    );
    perf.insert("eval_plan_nodes".to_string(), eval_plan.nodes.len() as u64);
    perf.insert("eval_plan_edges".to_string(), eval_plan.edges.len() as u64);
    perf.insert(
        "eval_plan_metric_nodes".to_string(),
        eval_plan.node_count_by_kind(EvalPlanNodeKind::MetricEval) as u64,
    );
    perf.insert(
        "eval_plan_rowset_nodes".to_string(),
        eval_plan.node_count_by_kind(EvalPlanNodeKind::Rowset) as u64,
    );
    perf.insert(
        "eval_plan_scalar_nodes".to_string(),
        eval_plan.node_count_by_kind(EvalPlanNodeKind::ScalarExpr) as u64,
    );
    perf.insert(
        "eval_plan_hydrate_nodes".to_string(),
        eval_plan.node_count_by_kind(EvalPlanNodeKind::Hydrate) as u64,
    );
    perf.insert(
        "eval_scope_key_hash".to_string(),
        hash_fingerprint(&eval_scope_key),
    );
    perf.insert(
        "eval_scope_group_key_hash".to_string(),
        hash_fingerprint(&eval_scope.query_state.group_identity_key()),
    );
    perf.insert(
        "eval_scope_time_range_key_hash".to_string(),
        hash_fingerprint(&eval_scope.query_state.time_range_identity_key()),
    );
    perf.insert(
        "eval_scope_group_dimensions".to_string(),
        eval_scope.query_state.group.len() as u64,
    );
    perf.insert("request_dag_nodes".to_string(), dag_metrics.nodes as u64);
    perf.insert("request_dag_edges".to_string(), dag_metrics.edges as u64);
    perf.insert("request_dag_hits".to_string(), dag_metrics.hits);
    perf.insert("request_dag_misses".to_string(), dag_metrics.misses);
    perf.insert(
        "request_dag_request_cache_hits".to_string(),
        dag_metrics.request_cache_hits,
    );
    perf.insert(
        "request_dag_eval_node_cache_hits".to_string(),
        dag_metrics.eval_node_cache_hits,
    );
    perf.insert(
        "request_dag_eval_node_cache_misses".to_string(),
        dag_metrics.eval_node_cache_misses,
    );
    if !workset.closure_metric_ids.is_empty() {
        perf.insert(
            "analysis_closure_nodes".to_string(),
            workset.closure_metric_ids.len() as u64,
        );
        let closure_set = workset
            .closure_metric_ids
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let closure_edges = dataset
            .runtime_analysis_graph
            .edges
            .iter()
            .filter(|edge| closure_set.contains(&edge.from) && closure_set.contains(&edge.to))
            .count() as u64;
        perf.insert("analysis_closure_edges".to_string(), closure_edges);
    }
    perf.insert(
        "eval_node_cache_enabled".to_string(),
        u64::from(runtime_eval_node_cache_enabled()),
    );
    perf.insert("metric_eval_ms".to_string(), metric_eval_ms);
    let total_ms = elapsed_ms(request_started);
    perf.insert("total_ms".to_string(), total_ms);
    tracing::info!(
        app_id = %app_id,
        scene_id = %scene_ctx.scene_id,
        target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
        dataset_id = %resource.id,
        metric_ids = %requested_metric_ids,
        compile_cache_hit = compile_outcome.cache_hit,
        compile_ms,
        query_api_ms = query_ms,
        metric_eval_ms,
        total_rows = runtime_dataset.rows.len(),
        total_ms,
        "metric query finished"
    );
    store_cached_metric_response(response_cache_key, runtime_dataset.rows.len(), &metrics_map);
    Ok(Json(MetricQueryResponse {
        scene_id: scene_ctx.scene_id,
        scene_path: scene_ctx.scene_path,
        dataset_id: resource.id.clone(),
        total_rows: runtime_dataset.rows.len(),
        metrics,
        perf,
    }))
}

#[cfg(test)]
mod tests {
    use super::runtime_metric_scope_requested;
    use mei_lang_kernel::{
        FilterIntent, FilterIntentSource, FilterOperator, QueryState, QueryTimeRange,
    };
    use std::collections::BTreeMap;

    #[test]
    fn runtime_metric_scope_requested_is_false_for_context_free_request() {
        assert!(!runtime_metric_scope_requested(&QueryState::default(), &[]));
    }

    #[test]
    fn runtime_metric_scope_requested_is_true_for_query_state_context() {
        assert!(runtime_metric_scope_requested(
            &QueryState {
                filters: BTreeMap::from([("status".to_string(), "待办".to_string())]),
                search: None,
                group: vec!["park".to_string()],
                time_range: Some(QueryTimeRange {
                    dimension: Some("created_at".to_string()),
                    start: Some("2024-01-01".to_string()),
                    end: Some("2024-12-31".to_string()),
                    preset: None,
                }),
            },
            &[],
        ));
    }

    #[test]
    fn runtime_metric_scope_requested_is_true_for_filter_intents() {
        assert!(runtime_metric_scope_requested(
            &QueryState::default(),
            &[FilterIntent {
                dimension: "status".to_string(),
                operator: FilterOperator::Eq,
                value: "待办".to_string(),
                source: FilterIntentSource::FilterBar,
            }],
        ));
    }
}
