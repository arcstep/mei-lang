mod opencode_api;
pub mod pages;
pub mod projection_api;
pub mod scene_api;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(pages::index))
        .route("/apps/:mode/:app_id", get(pages::app_page))
        .route("/api/projection/:app_id", get(projection_api::projection_api))
        .route("/api/sim/step/:app_id", post(scene_api::sim_step_api))
        .route("/api/opencode/config", get(opencode_api::api_opencode_config))
        .route("/api/opencode/runtime", get(opencode_api::api_opencode_runtime))
        .route("/api/opencode/health", get(opencode_api::api_opencode_health))
        .route("/api/opencode/start", post(opencode_api::api_opencode_start))
        .route("/api/opencode/stop", post(opencode_api::api_opencode_stop))
        .route("/workspace-components/*path", get(pages::component_asset))
}

pub(crate) fn error_response(error: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
}
