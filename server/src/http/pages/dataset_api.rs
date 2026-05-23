use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{AppError, AppState};

use super::super::compile_cache::compile_app_with_cache;
use super::super::datasets::{query_dataset_rows, query_metric_dataframe, DatasetQueryOptions};
use super::components::resolve_components_root;
use super::scene_qualified::{
    compile_options_from_coords, locate_dataset_resource, resolved_scene_context, SceneQueryCoords,
};
use super::util::elapsed_ms;

#[derive(Debug, Deserialize)]
pub struct DatasetQueryRequest {
    /// Scene anchor (preferred). `dataset_id` is local to this scene.
    #[serde(default)]
    pub scene_id: Option<String>,
    /// Legacy source locator; used when `scene_id` is absent.
    #[serde(default)]
    pub target: Option<String>,
    pub dataset_id: String,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub page_size: Option<usize>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub filters: BTreeMap<String, String>,
    #[serde(default)]
    pub full: bool,
    /// 非空时对 runtime metric（dataframe）求值后分页，与 dataset 行集共用过滤/分页语义。
    #[serde(default)]
    pub metric_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DatasetQueryResponse {
    pub scene_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_path: Option<String>,
    pub dataset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_id: Option<String>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub has_more: bool,
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    pub lazy: bool,
    pub perf: BTreeMap<String, u64>,
}

pub async fn dataset_query_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Json(request): Json<DatasetQueryRequest>,
) -> Result<Json<DatasetQueryResponse>, AppError> {
    let request_started = Instant::now();
    let app_id = app_id_raw.trim_start_matches('/').to_string();
    if app_id.is_empty() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "missing app id in route",
        ));
    }
    let coords = SceneQueryCoords::from_parts(request.scene_id.clone(), request.target.clone());
    let compile_options = compile_options_from_coords(&coords);
    let components_root = resolve_components_root(&state.source_root);
    let compile_outcome =
        compile_app_with_cache(&state, &app_id, compile_options, components_root.as_path())
            .map_err(|failure| {
                tracing::warn!(
                    app_id = %app_id,
                    error = %failure.error,
                    cache_lookup_ms = failure.cache_lookup_ms,
                    compile_cache_lock_wait_ms = failure.compile_cache_lock_wait_ms,
                    compile_ms = failure.compile_ms,
                    "dataset query compile failed"
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
    )?;
    let locate_started = Instant::now();
    let locate_dataset_ms = elapsed_ms(locate_started);
    let dataset = resource.dataset.as_ref().ok_or_else(|| {
        AppError::status(
            StatusCode::BAD_REQUEST,
            format!("resource `{}` is not a dataset", resource.id),
        )
    })?;
    let app_root = state.source_root.join(&app_id);
    let query = DatasetQueryOptions {
        page: request.page.unwrap_or(1),
        page_size: request.page_size.unwrap_or(0),
        search: request.search.clone(),
        filters: request.filters.clone(),
        collect_all: request.full,
    };
    let metric_id = request
        .metric_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let query_started = Instant::now();
    let result = if let Some(metric_id) = metric_id.as_deref() {
        query_metric_dataframe(
            &compiled,
            &app_root,
            normalized_dataset_id,
            metric_id,
            query,
        )
        .map_err(AppError::from)?
    } else {
        query_dataset_rows(&app_root, dataset, query).map_err(AppError::from)?
    };
    let query_ms = elapsed_ms(query_started);
    let mut perf = result.perf.clone();
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
    perf.insert("total_ms".to_string(), elapsed_ms(request_started));
    Ok(Json(DatasetQueryResponse {
        scene_id: scene_ctx.scene_id,
        scene_path: scene_ctx.scene_path,
        dataset_id: resource.id.clone(),
        metric_id,
        page: result.page,
        page_size: result.page_size,
        total: result.total,
        has_more: result.has_more,
        columns: result.columns,
        rows: result.rows,
        lazy: result.lazy,
        perf,
    }))
}
