//! Lazy panel contract fetch for preview SSR (P3b).

use std::path::Path;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use mei_lang_kernel::resolve_app_root;
use serde_json::{json, Value};

use crate::graph::content_store::{self, PANEL_CONTRACT};
use crate::graph::feature::graph_registry_enabled;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mcg::panel_contract::PanelContractRecord;
use crate::AppState;

use super::panel_lookup::find_panel_contract_node;

#[derive(Debug, serde::Deserialize)]
pub struct PanelContractQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(rename = "sceneId", default)]
    pub scene_id: Option<String>,
    #[serde(rename = "panelId", default)]
    pub panel_id: Option<String>,
    /// Content ref (`content/inspection-stats`) or basename (`inspection-stats`).
    #[serde(rename = "panelKey", default)]
    pub panel_key: Option<String>,
}

pub async fn api_build_panel_contract(
    State(state): State<AppState>,
    Query(query): Query<PanelContractQuery>,
) -> impl IntoResponse {
    if !graph_registry_enabled() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "MEI_GRAPH_REGISTRY disabled"})),
        )
            .into_response();
    }
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "appId is required"})),
        )
            .into_response();
    }

    let scene_id = query
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home");

    let lookup_key = resolve_lookup_key(&query, scene_id);
    let Some(lookup_key) = lookup_key else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "panelKey or sceneId+panelId is required"})),
        )
            .into_response();
    };

    let source_root = state.source_root.as_path();
    let mcg = McgRegistryWriter::load(source_root, app_id);
    let node = find_panel_contract_node(&mcg, lookup_key.as_str(), scene_id);
    let Some(node) = node else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "panel contract not found",
                "panelKey": lookup_key,
                "sceneId": scene_id,
            })),
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
            Json(json!({"error": "panel contract has no payload ref"})),
        )
            .into_response();
    };
    let app_root = resolve_app_root(source_root, app_id);
    let path = content_store::get(app_root.as_path(), PANEL_CONTRACT, hash);
    let Some(path) = path else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "panel contract blob missing"})),
        )
            .into_response();
    };
    match load_panel_payload(path.as_path()) {
        Ok(payload) => (
            StatusCode::OK,
            Json(json!({
                "panelKey": lookup_key,
                "sceneId": scene_id,
                "mcgNode": node,
                "contentHash": hash,
                "panel": payload,
            })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

fn resolve_lookup_key(query: &PanelContractQuery, scene_id: &str) -> Option<String> {
    if let Some(panel_key) = query
        .panel_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(panel_key.to_string());
    }
    let panel_id = query.panel_id.as_deref().map(str::trim).filter(|v| !v.is_empty());
    panel_id.map(|panel_id| format!("{scene_id}:{panel_id}"))
}

fn load_panel_payload(path: &Path) -> anyhow::Result<Value> {
    let raw = std::fs::read_to_string(path)?;
    if let Ok(record) = serde_json::from_str::<PanelContractRecord>(&raw) {
        if let Some(panel) = record.panel {
            return Ok(panel);
        }
    }
    let value: Value = serde_json::from_str(&raw)?;
    if let Some(payload) = value.get("payload") {
        return Ok(payload.clone());
    }
    Ok(value)
}
