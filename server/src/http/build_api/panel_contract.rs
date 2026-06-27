//! Lazy panel contract fetch for preview SSR (P3b).

use std::path::Path;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use mei_lang_kernel::resolve_app_root;

use crate::graph::content_store::{self, PANEL_CONTRACT};
use crate::graph::feature::graph_registry_enabled;
use crate::graph::mcg::panel_contract::PanelContractRecord;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::types::GraphNodeKind;
use crate::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct PanelContractQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(rename = "sceneId")]
    pub scene_id: String,
    #[serde(rename = "panelId")]
    pub panel_id: String,
}

pub async fn api_build_panel_contract(
    State(state): State<AppState>,
    Query(query): Query<PanelContractQuery>,
) -> impl IntoResponse {
    if !graph_registry_enabled() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "MEI_GRAPH_REGISTRY disabled"})),
        )
            .into_response();
    }
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "appId is required"})),
        )
            .into_response();
    }
    let scene_id = query.scene_id.trim();
    let panel_id = query.panel_id.trim();
    if scene_id.is_empty() || panel_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "sceneId and panelId are required"})),
        )
            .into_response();
    }
    let panel_key = format!("{scene_id}:{panel_id}");
    let source_root = state.source_root.as_path();
    let mcg = McgRegistryWriter::load(source_root, app_id);
    let node = mcg
        .nodes
        .iter()
        .find(|node| node.id.kind == GraphNodeKind::PanelContract && node.id.key == panel_key);
    let Some(node) = node else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "panel contract not found", "panelKey": panel_key})),
        )
            .into_response();
    };
    let hash = node
        .payload_ref
        .as_ref()
        .map(|payload| payload.content_hash.as_str())
        .filter(|value| !value.is_empty());
    let Some(hash) = hash else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "panel contract has no payload ref"})),
        )
            .into_response();
    };
    let app_root = resolve_app_root(source_root, app_id);
    let path = content_store::get(app_root.as_path(), PANEL_CONTRACT, hash);
    let Some(path) = path else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "panel contract blob missing"})),
        )
            .into_response();
    };
    match load_panel_record(path.as_path()) {
        Ok(record) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "panelKey": record.panel_key,
                "sceneId": record.scene_id,
                "panelId": record.panel_id,
                "revision": record.revision,
                "panel": record.panel,
            })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

fn load_panel_record(path: &Path) -> anyhow::Result<PanelContractRecord> {
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}
