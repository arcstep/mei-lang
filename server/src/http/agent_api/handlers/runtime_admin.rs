use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    agent_runtime::{
        runtime::{
            managed_agent_config_summary, managed_agent_runtime_status, managed_agent_skill_status,
            start_managed_agent, stop_managed_agent, sync_managed_agent_skill,
        },
        StartManagedOpencodeRequest,
    },
    AppState,
};

use crate::http::error_response;

pub async fn api_agent_config(State(state): State<AppState>) -> Response {
    Json(managed_agent_config_summary(&state)).into_response()
}

pub async fn api_agent_runtime(State(state): State<AppState>) -> Response {
    match managed_agent_runtime_status(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_skill(State(state): State<AppState>) -> Response {
    match managed_agent_skill_status(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_sync_skill(State(state): State<AppState>) -> Response {
    match sync_managed_agent_skill(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_start(
    State(state): State<AppState>,
    Json(request): Json<StartManagedOpencodeRequest>,
) -> Response {
    match start_managed_agent(&state, request).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_stop(State(state): State<AppState>) -> Response {
    match stop_managed_agent(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}
