use std::time::Instant;

use axum::{
    body::Bytes,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use mei_lang_kernel::resolve_app_root;

use crate::http::observation::CompileObservation;
use crate::{AppError, AppState};

use super::support::*;
use super::types::*;
use crate::http::compile_cache::{
    access_import_required, RuntimeAccessPolicies, RuntimeArtifactPolicy,
};
use crate::http::datasets::{
    map_dataset_query_filters, query_dataset_rows, query_metric_dataframe,
    query_state_from_request,
    table_contract::{apply_table_request_fields, enrich_table_result},
    DatasetQueryOptions,
};
use crate::http::pages::components::resolve_components_root;
use crate::http::pages::scene_qualified::{
    compile_options_from_coords, locate_dataset_resource, resolved_scene_context,
    strict_dataset_query_mode_contract, strict_runtime_query_contract, strict_scene_query_coords,
};
use crate::http::pages::util::elapsed_ms;

pub async fn dataset_query_api(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    AxumPath(app_id_raw): AxumPath<String>,
    body: Bytes,
) -> Result<Json<DatasetQueryResponse>, AppError> {
    let request_started = Instant::now();
    let app_id = app_id_raw.trim_start_matches('/').to_string();
    if app_id.is_empty() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "missing app id in route",
        ));
    }
    let request = parse_dataset_query_request(&app_id, &body)?;
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
    tracing::debug!("dataset query started");
    let coords = strict_scene_query_coords(
        request.scene_id.clone(),
        request.target.clone(),
        "dataset query",
    )?;
    let compile_options = compile_options_from_coords(&coords);
    let components_root = resolve_components_root(&state.source_root);
    let access_policies = RuntimeAccessPolicies::from_headers(&headers);
    let access_artifact_only = true;
    let compile_resolution = crate::http::compile_cache::resolve_runtime_compile_shared(
        &state,
        &app_id,
        &compile_options,
        components_root.as_path(),
        access_policies,
        mei_lang_app::UiRouteMode::App,
    )
    .map_err(|failure| AppError::from(failure.error))?
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
    let compile_outcome = compile_resolution.outcome;
    let compile_observation =
        CompileObservation::from_compile_outcome_shared(&app_id, "-", None, &compile_outcome);
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
    query.facet_columns = request
        .facet_columns
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
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
    perf.insert(
        "runtime_artifact_policy_sealed_strict".to_string(),
        u64::from(compile_resolution.policy.is_sealed_strict()),
    );
    perf.insert(
        "runtime_artifact_policy_artifact_first_fallback".to_string(),
        u64::from(matches!(
            compile_resolution.policy,
            RuntimeArtifactPolicy::ArtifactFirstFallback
        )),
    );
    perf.insert(
        "correctness_fallback".to_string(),
        u64::from(compile_resolution.correctness_fallback),
    );
    perf.insert(
        "artifact_backfilled".to_string(),
        u64::from(compile_resolution.artifact_backfilled),
    );
    perf.insert(
        "access_parquet_import_required".to_string(),
        u64::from(access_import_required()),
    );
    perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
    perf.insert("query_api_ms".to_string(), query_ms);
    let total_ms = elapsed_ms(request_started);
    perf.insert("total_ms".to_string(), total_ms);
    tracing::debug!(
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
        column_facets: result.column_facets,
    }))
}
