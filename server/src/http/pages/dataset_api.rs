use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use mei_lang_kernel::{deserialize_string_map, resolve_app_root, FilterIntent, QueryState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{AppError, AppState};
use crate::http::observation::CompileObservation;

use super::super::compile_cache::{
    load_compile_artifact_only_shared,
};
use super::super::datasets::{
    map_dataset_query_filters, query_dataset_rows, query_metric_dataframe,
    query_state_from_request,
    serde_lenient,
    table_contract::{
        apply_table_request_fields, enrich_table_result, TableColumnState, TableSortSpec,
    },
    DatasetQueryOptions,
};
use super::super::runtime_cache::{invalidate_app_runtime_caches, invalidate_report_perf};
use super::components::resolve_components_root;
use super::scene_qualified::{
    compile_options_from_coords, locate_dataset_resource, resolved_scene_context,
    strict_dataset_query_mode_contract, strict_runtime_query_contract, strict_scene_query_coords,
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
    #[serde(default, deserialize_with = "serde_lenient::opt_usize")]
    pub page: Option<usize>,
    #[serde(default, deserialize_with = "serde_lenient::opt_usize")]
    pub page_size: Option<usize>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_map")]
    pub filters: BTreeMap<String, String>,
    #[serde(default)]
    pub query_state: Option<QueryState>,
    #[serde(default)]
    pub filter_intents: Vec<FilterIntent>,
    #[serde(default, deserialize_with = "serde_lenient::bool_default_false")]
    pub full: bool,
    /// 非空时对 runtime metric（dataframe）求值后分页，与 dataset 行集共用过滤/分页语义。
    #[serde(default)]
    pub metric_id: Option<String>,
    #[serde(default)]
    pub sort: Vec<TableSortSpec>,
    #[serde(default)]
    pub column_state: Option<TableColumnState>,
    #[serde(default, deserialize_with = "serde_lenient::bool_default_false")]
    pub summary: bool,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_meta: Vec<super::super::datasets::TableColumnMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<super::super::datasets::TableSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_state_echo: Option<super::super::datasets::table_contract::QueryStateEcho>,
}

#[derive(Debug, Deserialize)]
pub struct DatasetRecomputeRequest {
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    pub dataset_id: String,
    #[serde(default)]
    pub metric_id: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DatasetRecomputeResponse {
    pub scene_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_path: Option<String>,
    pub dataset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_id: Option<String>,
    pub mode: String,
    pub compile_cache_cleared: usize,
    pub compiled_app_artifacts_cleared: usize,
    pub file_cache_cleared: usize,
    pub import_artifacts_cleared: usize,
    pub dataset_rows_cache_cleared: usize,
    pub eval_artifacts_cleared: usize,
    pub kernel_caches_cleared: bool,
    pub warmed: bool,
    pub perf: BTreeMap<String, u64>,
}

fn access_artifact_unavailable_error(
    request_kind: &str,
    app_id: &str,
    scene_id: &str,
    target: &str,
) -> AppError {
    let scene_label = if scene_id.trim().is_empty() || scene_id == "-" {
        "scene=<unspecified>"
    } else {
        scene_id
    };
    let target_label = if target.trim().is_empty() || target == "-" {
        "target=<unspecified>"
    } else {
        target
    };
    AppError::status(
        StatusCode::SERVICE_UNAVAILABLE,
        format!(
            "{request_kind} requires prebuilt access artifacts on access-only host: app={app_id} {scene_label} {target_label}; wait for startup warmup or prebuild artifacts before serving access traffic"
        ),
    )
}

pub async fn dataset_query_api(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
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
    strict_runtime_query_contract(
        &request.filters,
        request.search.as_deref(),
        request.query_state.as_ref(),
        &request.filter_intents,
        "dataset query",
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
    let requested_metric_id = request
        .metric_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let request_span = tracing::info_span!(
        "dataset_query_api",
        app_id = %app_id,
        scene_id = %requested_scene_id,
        target = %requested_target,
        dataset_id = %requested_dataset_id,
        metric_id = %requested_metric_id
    );
    let _request_span_guard = request_span.enter();
    tracing::info!("dataset query started");
    let coords = strict_scene_query_coords(
        request.scene_id.clone(),
        request.target.clone(),
        "dataset query",
    )?;
    let compile_options = compile_options_from_coords(&coords);
    let components_root = resolve_components_root(&state.source_root);
    let build_view_dev = crate::http::compile_cache::is_build_view_request(&headers);
    let access_artifact_only = !build_view_dev;
    let compile_outcome = crate::http::compile_cache::resolve_runtime_compile_shared(
        &state,
        &app_id,
        &compile_options,
        components_root.as_path(),
        build_view_dev,
    )
    .ok_or_else(|| {
        let scene_label = if requested_scene_id.trim().is_empty() || requested_scene_id == "-" {
            "scene=<unspecified>".to_string()
        } else {
            requested_scene_id.to_string()
        };
        let target_label = if requested_target.trim().is_empty() || requested_target == "-" {
            "target=<unspecified>".to_string()
        } else {
            requested_target.to_string()
        };
        let error_message = format!(
            "dataset query requires prebuilt access artifacts on access-only host: app={app_id} {scene_label} {target_label}; wait for startup warmup or prebuild artifacts before serving access traffic"
        );
        let error = access_artifact_unavailable_error(
            "dataset query",
            &app_id,
            requested_scene_id,
            requested_target,
        );
        crate::http::host_api::mark_access_artifact_degraded(
            &app_id,
            Some(requested_scene_id),
            Some(requested_target),
            &error_message,
        );
        tracing::warn!(
            app_id = %app_id,
            scene_id = %requested_scene_id,
            target = %requested_target,
            dataset_id = %requested_dataset_id,
            metric_id = %requested_metric_id,
            phase = "artifact_only_miss",
            "dataset query rejected because host requires prebuilt artifacts"
        );
        error
    })?;
    let compile_observation = CompileObservation::from_compile_outcome_shared(
        &app_id,
        "-",
        None,
        &compile_outcome,
    );
    let compiled = compile_outcome.compiled;
    let scene_ctx = resolved_scene_context(&compiled);
    let requested_dataset_id = request.dataset_id.trim();
    let resource = locate_dataset_resource(&compiled, requested_dataset_id, Some(&coords))
        .map_err(|error| {
            tracing::warn!(
                app_id = %app_id,
                scene_id = %requested_scene_id,
                target = %requested_target,
                dataset_id = %requested_dataset_id,
                metric_id = %requested_metric_id,
                phase = "locate_dataset",
                error = ?error,
                "dataset query locate failed"
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
    let canonical_dataset_id = resource.id.clone();
    let app_root = resolve_app_root(state.source_root.as_path(), &app_id);
    let effective_query_state = query_state_from_request(
        &request.filters,
        request.search.as_deref(),
        request.query_state.as_ref(),
    );
    let mapped_filters = map_dataset_query_filters(&effective_query_state, dataset);
    let mut query = DatasetQueryOptions {
        page: request.page.unwrap_or(1),
        page_size: request.page_size.unwrap_or(0),
        search: effective_query_state.search.clone(),
        filters: mapped_filters,
        group: effective_query_state.group.clone(),
        time_range: effective_query_state.time_range.clone(),
        collect_all: request.full,
        ..DatasetQueryOptions::default()
    };
    apply_table_request_fields(
        &mut query,
        request.sort.clone(),
        request.column_state.clone(),
        request.summary,
    );
    let metric_id = request
        .metric_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    strict_dataset_query_mode_contract(metric_id.as_deref(), &request.filter_intents)?;
    let query_started = Instant::now();
    let mut result = if let Some(metric_id) = metric_id.as_deref() {
        query_metric_dataframe(
            &compiled,
            &app_root,
            canonical_dataset_id.as_str(),
            metric_id,
            Some(&scene_ctx.scene_id),
            scene_ctx.scene_path.as_deref(),
            &compile_outcome.compile_revision,
            query.clone(),
            Some(effective_query_state.clone()),
            request.filter_intents.clone(),
        )
        .map_err(|error| {
            tracing::warn!(
                app_id = %app_id,
                scene_id = %scene_ctx.scene_id,
                target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
                dataset_id = %resource.id,
                metric_id = %metric_id,
                phase = "query_metric_dataframe",
                error = %error,
                "dataset query metric dataframe failed"
            );
            AppError::from(error)
        })?
    } else {
        query_dataset_rows(&app_root, dataset, query.clone()).map_err(|error| {
            tracing::warn!(
                app_id = %app_id,
                scene_id = %scene_ctx.scene_id,
                target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
                dataset_id = %resource.id,
                phase = "query_dataset_rows",
                error = %error,
                "dataset query rows failed"
            );
            AppError::from(error)
        })?
    };
    let query_ms = elapsed_ms(query_started);
    result = enrich_table_result(dataset, &query, result);
    let mut perf = result.perf.clone();
    compile_observation.write_perf(&mut perf);
    perf.insert(
        "access_artifact_only_mode".to_string(),
        u64::from(access_artifact_only),
    );
    perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
    perf.insert("query_api_ms".to_string(), query_ms);
    let total_ms = elapsed_ms(request_started);
    perf.insert("total_ms".to_string(), total_ms);
    tracing::info!(
        app_id = %app_id,
        scene_id = %scene_ctx.scene_id,
        target = %scene_ctx.scene_path.as_deref().unwrap_or("-"),
        dataset_id = %resource.id,
        metric_id = %metric_id.as_deref().unwrap_or("-"),
        page = result.page,
        page_size = result.page_size,
        total_rows = result.total,
        compile_cache_hit = compile_outcome.cache_hit,
        compile_ms = compile_observation.compile_ms,
        compile_cache_lock_wait_ms = compile_outcome.compile_cache_lock_wait_ms,
        query_api_ms = query_ms,
        total_ms,
        "dataset query finished"
    );
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
        column_meta: result.column_meta,
        summary: result.summary,
        query_state_echo: result.query_state_echo,
    }))
}

pub async fn dataset_recompute_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Json(request): Json<DatasetRecomputeRequest>,
) -> Result<Json<DatasetRecomputeResponse>, AppError> {
    let request_started = Instant::now();
    let app_id = app_id_raw.trim_start_matches('/').to_string();
    if app_id.is_empty() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "missing app id in route",
        ));
    }
    let mode = request
        .mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("clear_and_warm")
        .to_ascii_lowercase();
    if !matches!(mode.as_str(), "clear_only" | "clear_and_warm") {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            format!("unsupported recompute mode `{mode}`"),
        ));
    }
    let app_root = resolve_app_root(state.source_root.as_path(), &app_id);
    let warmed = mode == "clear_and_warm";
    let invalidate_report = invalidate_app_runtime_caches(&state, &app_id);
    let mut perf = invalidate_report_perf(&invalidate_report);
    let metric_id = request
        .metric_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let requested_scene = request
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
        .map(str::to_string);
    let mut response_scene_id = requested_scene.to_string();
    let mut response_scene_path = requested_target;
    if warmed {
        let coords = strict_scene_query_coords(
            request.scene_id.clone(),
            request.target.clone(),
            "dataset recompute",
        )?;
        let compile_options = compile_options_from_coords(&coords);
        let components_root = resolve_components_root(&state.source_root);
        let compile_started = Instant::now();
        let compile_outcome = load_compile_artifact_only_shared(
            &state,
            &app_id,
            &compile_options,
            components_root.as_path(),
        )
        .ok_or_else(|| {
            access_artifact_unavailable_error(
                "dataset recompute warmup",
                &app_id,
                requested_scene,
                response_scene_path.as_deref().unwrap_or("-"),
            )
        })?;
        let compile_ms = elapsed_ms(compile_started);
        perf.insert("compile_ms".to_string(), compile_ms);
        perf.insert(
            "compile_cache_hit".to_string(),
            u64::from(compile_outcome.cache_hit),
        );
        perf.insert(
            "compile_cache_lookup_ms".to_string(),
            compile_outcome.cache_lookup_ms,
        );
        let compiled = compile_outcome.compiled;
        let scene_ctx = resolved_scene_context(&compiled);
        response_scene_id = scene_ctx.scene_id.clone();
        response_scene_path = scene_ctx.scene_path.clone();
        let locate_started = Instant::now();
        let resource = locate_dataset_resource(&compiled, request.dataset_id.trim(), Some(&coords))
            .map_err(AppError::from)?;
        perf.insert("locate_dataset_ms".to_string(), elapsed_ms(locate_started));
        let warm_started = Instant::now();
        let warm_query = DatasetQueryOptions {
            page: 1,
            page_size: 20,
            search: None,
            filters: BTreeMap::new(),
            collect_all: false,
            ..DatasetQueryOptions::default()
        };
        if let Some(metric_id) = metric_id.as_deref() {
            let result = query_metric_dataframe(
                &compiled,
                &app_root,
                resource.id.as_str(),
                metric_id,
                Some(&scene_ctx.scene_id),
                scene_ctx.scene_path.as_deref(),
                &compile_outcome.compile_revision,
                warm_query,
                None,
                Vec::new(),
            )
            .map_err(AppError::from)?;
            for (key, value) in result.perf {
                perf.insert(format!("warm_{key}"), value);
            }
            perf.insert("warm_total_rows".to_string(), result.total as u64);
        } else {
            let dataset = resource.dataset.as_ref().ok_or_else(|| {
                AppError::status(
                    StatusCode::BAD_REQUEST,
                    format!("resource `{}` is not a dataset", resource.id),
                )
            })?;
            let result =
                query_dataset_rows(&app_root, dataset, warm_query).map_err(AppError::from)?;
            for (key, value) in result.perf {
                perf.insert(format!("warm_{key}"), value);
            }
            perf.insert("warm_total_rows".to_string(), result.total as u64);
        }
        perf.insert("warm_ms".to_string(), elapsed_ms(warm_started));
    }
    perf.insert("total_ms".to_string(), elapsed_ms(request_started));
    Ok(Json(DatasetRecomputeResponse {
        scene_id: response_scene_id,
        scene_path: response_scene_path,
        dataset_id: request.dataset_id.trim().to_string(),
        metric_id,
        mode,
        compile_cache_cleared: invalidate_report.compile_cache_cleared,
        compiled_app_artifacts_cleared: invalidate_report.compiled_app_artifacts_cleared,
        file_cache_cleared: invalidate_report.file_cache_cleared,
        import_artifacts_cleared: invalidate_report.import_artifacts_cleared,
        dataset_rows_cache_cleared: invalidate_report.dataset_rows_cache_cleared,
        eval_artifacts_cleared: invalidate_report.eval_artifacts_cleared,
        kernel_caches_cleared: true,
        warmed,
        perf,
    }))
}
