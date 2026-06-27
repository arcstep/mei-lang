//! Runtime observability snapshot API.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::http::runtime_snapshot::build_runtime_observability_snapshot;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct RuntimeSnapshotQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
}

pub async fn api_runtime_snapshot(
    State(state): State<AppState>,
    Query(params): Query<RuntimeSnapshotQuery>,
) -> impl IntoResponse {
    let app_id = params.app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "appId is required"})),
        )
            .into_response();
    }
    let snapshot = build_runtime_observability_snapshot(state.source_root.as_path(), app_id);
    (StatusCode::OK, Json(snapshot)).into_response()
}
