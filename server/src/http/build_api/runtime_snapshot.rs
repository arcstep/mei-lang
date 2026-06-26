//! Runtime observability snapshot API.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;

use crate::http::runtime_snapshot::build_runtime_observability_roots;
use crate::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct RuntimeSnapshotQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
}

#[derive(Debug, Serialize)]
struct RuntimeSnapshotResponse {
    #[serde(rename = "appId")]
    app_id: String,
    roots: Vec<mei_lang_kernel::ReachabilityTreeRoot>,
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
    let roots = build_runtime_observability_roots(state.source_root.as_path(), app_id);
    (
        StatusCode::OK,
        Json(RuntimeSnapshotResponse {
            app_id: app_id.to_string(),
            roots,
        }),
    )
        .into_response()
}
