use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    opencode::{
        bridge::{health as bridge_health, BridgeHealthResponse},
        runtime::{
            managed_opencode_config_summary, managed_opencode_runtime_status,
            managed_opencode_server_url, start_managed_opencode, stop_managed_opencode,
        },
        StartManagedOpencodeRequest,
    },
    AppState,
};

use super::error_response;

pub async fn api_opencode_config(State(state): State<AppState>) -> Response {
    Json(managed_opencode_config_summary(&state)).into_response()
}

pub async fn api_opencode_runtime(State(state): State<AppState>) -> Response {
    match managed_opencode_runtime_status(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_health(State(state): State<AppState>) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(_) => {
            return Json(BridgeHealthResponse {
                server_url: String::new(),
                healthy: false,
                version: String::new(),
            })
            .into_response()
        }
    };
    match bridge_health(&state.opencode_http, &server_url).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_start(
    State(state): State<AppState>,
    Json(request): Json<StartManagedOpencodeRequest>,
) -> Response {
    match start_managed_opencode(&state, request).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_stop(State(state): State<AppState>) -> Response {
    match stop_managed_opencode(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}
