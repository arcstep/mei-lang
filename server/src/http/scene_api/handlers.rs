use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    Json,
};
use mei_lang_kernel::{
    compile_app, initial_runtime_state, project_runtime_view, render_runtime_html, runtime_step,
};

use crate::{AppError, AppState};

use super::types::{
    SimStepRequest, SimStepResponse, WorldAssetGetQuery, WorldAssetGetResponse,
    WorldAssetListQuery, WorldAssetListResponse, WorldContextSnapshot, WorldRuntimePeekQuery,
    WorldRuntimePeekResponse, WorldScopeQuery,
};
use super::world::{
    build_world_context_snapshot, query_resource_get, query_resource_list,
    query_resource_runtime_peek,
};

pub async fn world_context_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Query(scope_query): Query<WorldScopeQuery>,
) -> Result<Json<WorldContextSnapshot>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let scope = scope_query.to_scope();
    let snapshot = build_world_context_snapshot(&state.source_root, app_id, Some(&scope))
        .map_err(AppError::from)?;
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
    .map_err(AppError::from)?;
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
            .map_err(AppError::from)?;
    Ok(Json(response))
}

pub async fn sim_step_api(
    State(state): State<AppState>,
    AxumPath(app_id_raw): AxumPath<String>,
    Json(request): Json<SimStepRequest>,
) -> Result<Json<SimStepResponse>, AppError> {
    let app_id = app_id_raw.trim_start_matches('/');
    let compiled = compile_app(&state.source_root, app_id).map_err(AppError::from)?;
    let contract = compiled.scene_contract.ok_or_else(|| {
        AppError::msg(format!(
            "app `{}` does not provide a scene contract",
            app_id
        ))
    })?;
    let current_state = request
        .state
        .clone()
        .unwrap_or_else(|| initial_runtime_state(&contract, 1));
    let next_state = runtime_step(&contract, request.state, &request.intent);
    let trace_delta = if next_state.trace_events.len() > current_state.trace_events.len() {
        next_state.trace_events[current_state.trace_events.len()..].to_vec()
    } else if request.intent.kind == "sync" {
        next_state.trace_events.clone()
    } else {
        Vec::new()
    };
    let scene_view = project_runtime_view(&contract, &next_state);
    let html = render_runtime_html(&scene_view, &next_state);
    Ok(Json(SimStepResponse {
        state: next_state,
        scene_view,
        trace_delta,
        html,
    }))
}
