use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use mei_lang_kernel::resolve_app_root;
use std::collections::BTreeMap;

use crate::{AppError, AppState};

use super::support::*;
use super::types::*;
use crate::http::compile_cache::RuntimeAccessPolicies;
use crate::http::datasets::{query_dataset_rows, query_metric_dataframe, DatasetQueryOptions};
use crate::http::pages::components::resolve_components_root;
use crate::http::pages::scene_qualified::{
    compile_options_from_coords, locate_dataset_resource, resolved_scene_context,
    strict_scene_query_coords,
};
use crate::http::pages::util::elapsed_ms;
use crate::http::runtime_cache::{
    invalidate_after_data_reload, invalidate_app_runtime_caches, invalidate_report_perf,
};

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
    let dataset_id = request.dataset_id.trim();
    let source_ids = if dataset_id.is_empty() {
        None
    } else {
        Some(vec![dataset_id.to_string()])
    };
    let source_ids_slice = source_ids.as_ref().map(|ids| ids.as_slice());
    let invalidate_report = invalidate_after_data_reload(&state, &app_id, source_ids_slice)
        .map(|(report, _)| report)
        .unwrap_or_else(|_| invalidate_app_runtime_caches(&state, &app_id));
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
        let access_policies = RuntimeAccessPolicies::default_for_access_host();
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
            access_artifact_unavailable_error(
                "dataset recompute warmup",
                &app_id,
                requested_scene,
                response_scene_path.as_deref().unwrap_or("-"),
            )
        })?;
        let compile_outcome = compile_resolution.outcome;
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
        perf.insert(
            "correctness_fallback".to_string(),
            u64::from(compile_resolution.correctness_fallback),
        );
        perf.insert(
            "artifact_backfilled".to_string(),
            u64::from(compile_resolution.artifact_backfilled),
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
