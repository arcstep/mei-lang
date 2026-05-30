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
    runtime_eval_node_cache_enabled, MetricContract,
};
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState};

use super::super::compile_cache::compile_app_with_cache;
use super::super::datasets::{
    eval_node_cache_key, hydrate_file_backed_datasets_for_metric_defs, metric_request_revision_fingerprint,
    metric_scope_cache_key, normalize_query_filters, normalize_query_search, query_dataset_rows,
    resolve_runtime_metric_ids, runtime_metric_eval_scope, select_metric_defs, serialize_cache_value,
    DatasetQueryOptions,
};
use super::components::resolve_components_root;
use super::scene_qualified::{
    compile_options_from_coords, locate_dataset_resource, resolved_scene_context, SceneQueryCoords,
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
    if message.contains("cyclic_eval_dependency") || message.contains("metric_eval_recursion_guard_tripped")
    {
        "metric_eval_recursion_guard_tripped"
    } else {
        "metric_eval_failed"
    }
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
    format!(
        "{app_id}|compile={compile_revision}|{dependency_revision_key}|scene={scene_id}|target={}|dataset={dataset_id}|metric_ids={metric_scope_key}|search={}|filters={}",
        scene_path.unwrap_or(""),
        query.search.as_deref().unwrap_or(""),
        serialize_cache_value(&query.filters)
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
    let coords = SceneQueryCoords::from_parts(request.scene_id.clone(), request.target.clone());
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
    let compiled = compile_outcome.compiled;
    let compile_ms = compile_outcome.compile_ms;
    let scene_ctx = resolved_scene_context(&compiled);
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
    if dataset.runtime_metric_defs.is_empty() {
        if dataset.metrics.is_empty() {
            tracing::warn!(
                app_id = %app_id,
                scene_id = %scene_ctx.scene_id,
                target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
                dataset_id = %resource.id,
                metric_ids = %requested_metric_ids,
                phase = "metric_defs",
                "metric query dataset has no runtime metric defs"
            );
            return Err(AppError::status(
                StatusCode::BAD_REQUEST,
                format!("dataset `{}` has no runtime metric defs", resource.id),
            ));
        }
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
        perf.insert("compile_ms".to_string(), compile_ms);
        perf.insert(
            "compile_cache_hit".to_string(),
            u64::from(compile_outcome.cache_hit),
        );
        perf.insert(
            "compile_cache_lookup_ms".to_string(),
            compile_outcome.cache_lookup_ms,
        );
        perf.insert(
            "compile_cache_lock_wait_ms".to_string(),
            compile_outcome.compile_cache_lock_wait_ms,
        );
        perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
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
    let app_root = state.source_root.join(&app_id);
    let normalized_search = normalize_query_search(request.search.as_deref());
    let normalized_filters = normalize_query_filters(&request.filters);
    let query = DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: normalized_search,
        filters: normalized_filters,
        collect_all: true,
        ..DatasetQueryOptions::default()
    };
    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| resource.dataset.clone().map(|dataset| (resource.id.clone(), dataset)))
        .fold(BTreeMap::new(), |mut acc, (resource_id, dataset)| {
            acc.insert(resource_id, dataset.clone());
            acc.entry(dataset.id.clone()).or_insert(dataset);
            acc
        });
    let resolved_metric_ids = resolve_runtime_metric_ids(
        &resource.id,
        &request.metric_ids,
        &dataset.runtime_metric_defs,
    );
    let metric_ids = if request.metric_ids.is_empty() {
        None
    } else {
        Some(resolved_metric_ids.as_slice())
    };
    let defs_for_hydrate = if request.metric_ids.is_empty() {
        dataset.runtime_metric_defs.clone()
    } else {
        select_metric_defs(&dataset.runtime_metric_defs, &resolved_metric_ids)
    };
    let dependency_revision_key =
        metric_request_revision_fingerprint(&app_root, &datasets, &resource.id, &defs_for_hydrate);
    let response_cache_key = metric_response_cache_key(
        &app_id,
        &scene_ctx.scene_id,
        scene_ctx.scene_path.as_deref(),
        &resource.id,
        &metric_scope_cache_key(&resolved_metric_ids),
        &query,
        &compile_outcome.compile_revision,
        &dependency_revision_key,
    );
    let response_cache_lookup_started = Instant::now();
    if let Some(cached) = take_cached_metric_response(&response_cache_key) {
        let mut perf = BTreeMap::new();
        perf.insert("compile_ms".to_string(), compile_ms);
        perf.insert(
            "compile_cache_hit".to_string(),
            u64::from(compile_outcome.cache_hit),
        );
        perf.insert(
            "compile_cache_lookup_ms".to_string(),
            compile_outcome.cache_lookup_ms,
        );
        perf.insert(
            "compile_cache_lock_wait_ms".to_string(),
            compile_outcome.compile_cache_lock_wait_ms,
        );
        perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
        perf.insert("response_cache_hit".to_string(), 1);
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
                &resource.id,
                &request.metric_ids,
                &dataset.runtime_metric_defs,
                &cached.metrics_map,
            ),
            perf,
        }));
    }
    let query_started = Instant::now();
    let filtered_rows = query_dataset_rows(&app_root, dataset, query.clone()).map_err(|error| {
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
    let mut runtime_dataset = dataset.clone();
    runtime_dataset.rows = filtered_rows.rows.clone();
    if !filtered_rows.columns.is_empty() {
        runtime_dataset.columns = filtered_rows.columns.clone();
    }
    datasets.insert(resource.id.clone(), runtime_dataset.clone());
    let hydrate_started = Instant::now();
    let hydrate_perf =
        hydrate_file_backed_datasets_for_metric_defs(&app_root, &mut datasets, &defs_for_hydrate, &query)
            .unwrap_or_default();
    let hydrate_ms = elapsed_ms(hydrate_started);
    let metric_started = Instant::now();
    let eval_scope = runtime_metric_eval_scope(
        &resource.id,
        &scene_ctx.scene_id,
        scene_ctx.scene_path.as_deref(),
        query.search.as_deref(),
        &query.filters,
        &dependency_revision_key,
    );
    let (metrics_map, dag_metrics) = evaluate_runtime_metric_defs_with_scope_and_dag(
        &dataset.runtime_metric_defs,
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
    let metrics = project_requested_metrics(
        &resource.id,
        &request.metric_ids,
        &dataset.runtime_metric_defs,
        &metrics_map,
    );
    let mut perf = filtered_rows.perf.clone();
    let eval_scope_key = eval_node_cache_key("metric_scope", &eval_scope);
    perf.insert("compile_ms".to_string(), compile_ms);
    perf.insert(
        "compile_cache_hit".to_string(),
        u64::from(compile_outcome.cache_hit),
    );
    perf.insert(
        "compile_cache_lookup_ms".to_string(),
        compile_outcome.cache_lookup_ms,
    );
    perf.insert(
        "compile_cache_lock_wait_ms".to_string(),
        compile_outcome.compile_cache_lock_wait_ms,
    );
    perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
    perf.insert("response_cache_hit".to_string(), 0);
    perf.insert(
        "response_cache_lookup_ms".to_string(),
        elapsed_ms(response_cache_lookup_started),
    );
    perf.insert("query_api_ms".to_string(), query_ms);
    perf.insert("hydrate_datasets_ms".to_string(), hydrate_ms);
    for (key, value) in hydrate_perf {
        perf.insert(key, value);
    }
    perf.insert(
        "eval_scope_key_hash".to_string(),
        hash_fingerprint(&eval_scope_key),
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
