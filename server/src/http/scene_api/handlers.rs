use anyhow::Error as AnyhowError;
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use mei_lang_toolchain as toolchain;

use crate::{AppError, AppState};

use super::types::{
    SimStepRequest, SimStepResponse, WorldAssetGetQuery, WorldAssetGetResponse,
    WorldAssetListQuery, WorldAssetListResponse, WorldContextSnapshot, WorldRuntimePeekQuery,
    WorldRuntimePeekResponse, WorldScopeQuery,
};
use super::{query_resource_get, query_resource_list, query_resource_runtime_peek};

fn map_world_bundle_error(err: AnyhowError) -> AppError {
    let msg = err.to_string();
    if (msg.contains("scene `") && msg.contains("not found"))
        || msg.contains("does not provide a scene contract")
    {
        AppError::status(StatusCode::NOT_FOUND, msg)
    } else if msg.contains("not bound to target") {
        AppError::status(StatusCode::BAD_REQUEST, msg)
    } else {
        AppError::from(err)
    }
}

pub async fn world_context_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Query(scope_query): Query<WorldScopeQuery>,
) -> Result<Json<WorldContextSnapshot>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let scope = scope_query.to_scope();
    let snapshot = toolchain::build_world_context_snapshot(&state.source_root, app_id, Some(&scope))
        .map_err(map_world_bundle_error)?;
    Ok(Json(snapshot))
}

pub async fn world_assets_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Query(query): Query<WorldAssetListQuery>,
) -> Result<Json<WorldAssetListResponse>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let scope = query.scope.to_scope();
    let response = query_resource_list(
        &state.source_root,
        app_id,
        Some(&scope),
        query.kind.as_deref(),
        query.limit,
    )
    .map_err(map_world_bundle_error)?;
    Ok(Json(response))
}

pub async fn world_asset_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Query(query): Query<WorldAssetGetQuery>,
) -> Result<Json<WorldAssetGetResponse>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let scope = query.scope.to_scope();
    let target_id = query.id.trim().to_string();
    if target_id.is_empty() {
        return Err(AppError::status(
            StatusCode::BAD_REQUEST,
            "query parameter `id` is required",
        ));
    }
    let response = query_resource_get(&state.source_root, app_id, Some(&scope), &target_id)
        .map_err(|error| {
            let msg = error.to_string();
            if msg.contains("not found") {
                AppError::status(StatusCode::NOT_FOUND, msg)
            } else if msg.contains("required") {
                AppError::status(StatusCode::BAD_REQUEST, msg)
            } else {
                AppError::from(error)
            }
        })?;
    Ok(Json(response))
}

pub async fn world_runtime_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Query(query): Query<WorldRuntimePeekQuery>,
) -> Result<Json<WorldRuntimePeekResponse>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let scope = query.scope.to_scope();
    let response =
        query_resource_runtime_peek(&state.source_root, app_id, Some(&scope), query.trace_limit)
            .map_err(map_world_bundle_error)?;
    Ok(Json(response))
}

pub async fn sim_step_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Json(request): Json<SimStepRequest>,
) -> Result<Json<SimStepResponse>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let result = toolchain::runtime_sim_step(
        &state.source_root,
        app_id,
        request.state,
        request.intent,
    )
    .map_err(AppError::from)?;
    Ok(Json(SimStepResponse {
        state: result.state,
        scene_view: result.scene_view,
        trace_delta: result.trace_delta,
        html: result.html,
    }))
}
