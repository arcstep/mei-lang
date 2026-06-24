//! Read-only graph registry HTTP handlers (Phase 3).

use std::path::Path;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::graph::bridge::BridgeWriter;
use crate::graph::feature::graph_registry_enabled;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mrg::registry::MrgRegistryWriter;
use crate::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct GraphAppQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
}

pub async fn api_build_graph_mcg(
    State(state): State<AppState>,
    Query(params): Query<GraphAppQuery>,
) -> impl IntoResponse {
    graph_read_response(state.source_root.as_path(), params.app_id.as_str(), |root, app_id| {
        McgRegistryWriter::load(root, app_id)
    })
}

pub async fn api_build_graph_mrg(
    State(state): State<AppState>,
    Query(params): Query<GraphAppQuery>,
) -> impl IntoResponse {
    graph_read_response(state.source_root.as_path(), params.app_id.as_str(), |root, app_id| {
        MrgRegistryWriter::load(root, app_id)
    })
}

pub async fn api_build_graph_bridge(
    State(state): State<AppState>,
    Query(params): Query<GraphAppQuery>,
) -> impl IntoResponse {
    if !graph_registry_enabled() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "MEI_GRAPH_REGISTRY disabled"})),
        )
            .into_response();
    }
    match BridgeWriter::load(state.source_root.as_path(), params.app_id.as_str()) {
        Some(bridge) => (StatusCode::OK, Json(bridge)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "bridge not found"})),
        )
            .into_response(),
    }
}

fn graph_read_response<T: serde::Serialize>(
    source_root: &Path,
    app_id: &str,
    load: impl FnOnce(&Path, &str) -> T,
) -> axum::response::Response {
    if !graph_registry_enabled() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "MEI_GRAPH_REGISTRY disabled"})),
        )
            .into_response();
    }
    if app_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "appId is required"})),
        )
            .into_response();
    }
    let registry = load(source_root, app_id.trim());
    (StatusCode::OK, Json(registry)).into_response()
}
