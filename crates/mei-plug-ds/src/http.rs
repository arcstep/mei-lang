use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use mei_host_core::HostContext;
use serde_json::json;

#[derive(Clone)]
pub struct PlugState {
    pub ctx: HostContext,
}

pub fn router(ctx: HostContext) -> Router {
    let state = Arc::new(PlugState { ctx });
    Router::new()
        .route("/api/plug-ds/health", get(api_health))
        .route(
            "/api/datasets/query/:app_id",
            post(api_datasets_query_with_app),
        )
        .route("/api/datasets/query", post(api_datasets_query))
        .route(
            "/api/datasets/metrics/:app_id",
            post(api_datasets_metrics),
        )
        .with_state(state)
}

async fn api_health() -> impl IntoResponse {
    Json(json!({ "ok": true, "plug": "mei-plug-ds" }))
}

async fn api_datasets_query(
    State(state): State<Arc<PlugState>>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    match crate::plugin::query_dataset(&state.ctx, &body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

async fn api_datasets_query_with_app(
    State(state): State<Arc<PlugState>>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    if state.ctx.app_id != app_id {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app mismatch"})),
        )
            .into_response();
    }
    api_datasets_query(State(state), axum::Json(body)).await
}

async fn api_datasets_metrics(
    State(state): State<Arc<PlugState>>,
    Path(app_id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    if state.ctx.app_id != app_id {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app mismatch"})),
        )
            .into_response();
    }
    match crate::plugin::query_metrics(&state.ctx, &body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}
