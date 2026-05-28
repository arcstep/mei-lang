use std::collections::BTreeMap;
use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use mei_lang_kernel::{evaluate_runtime_metric_defs, MetricContract};
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState};

use super::super::compile_cache::compile_app_with_cache;
use super::super::datasets::{query_dataset_rows, DatasetQueryOptions};
use super::components::resolve_components_root;
use super::scene_qualified::{
    compile_options_from_coords, locate_dataset_resource, resolved_scene_context, SceneQueryCoords,
};
use super::util::elapsed_ms;

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
    let query = DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: request.search.clone(),
        filters: request.filters.clone(),
        collect_all: true,
        ..DatasetQueryOptions::default()
    };
    let query_started = Instant::now();
    let filtered_rows = query_dataset_rows(&app_root, dataset, query).map_err(|error| {
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
    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    datasets.insert(resource.id.clone(), runtime_dataset.clone());
    let metric_ids = if request.metric_ids.is_empty() {
        None
    } else {
        Some(request.metric_ids.as_slice())
    };
    let metric_started = Instant::now();
    let metrics_map = evaluate_runtime_metric_defs(
        &dataset.runtime_metric_defs,
        &runtime_dataset.rows,
        &datasets,
        metric_ids,
    )
    .map_err(|error| {
        tracing::warn!(
            app_id = %app_id,
            scene_id = %scene_ctx.scene_id,
            target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
            dataset_id = %resource.id,
            metric_ids = %requested_metric_ids,
            phase = "metric_eval",
            error = %error,
            "metric query evaluate runtime metric defs failed"
        );
        AppError::from(error)
    })?;
    let metric_eval_ms = elapsed_ms(metric_started);
    let metrics = if request.metric_ids.is_empty() {
        metrics_map.into_values().collect::<Vec<_>>()
    } else {
        request
            .metric_ids
            .iter()
            .filter_map(|metric_id| metrics_map.get(metric_id).cloned())
            .collect::<Vec<_>>()
    };
    let mut perf = filtered_rows.perf.clone();
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
    perf.insert("query_api_ms".to_string(), query_ms);
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
    Ok(Json(MetricQueryResponse {
        scene_id: scene_ctx.scene_id,
        scene_path: scene_ctx.scene_path,
        dataset_id: resource.id.clone(),
        total_rows: runtime_dataset.rows.len(),
        metrics,
        perf,
    }))
}
