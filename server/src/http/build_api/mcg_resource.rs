//! Direct MCG node and content-store artifact access for build-view debugging.

use std::path::Path;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use mei_lang_kernel::resolve_app_root;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::block::parse_block_id;
use crate::graph::content_store;
use crate::graph::feature::graph_registry_enabled;
use crate::graph::mcg::registry::{McgNodeRecord, McgRegistry, McgRegistryWriter};
use crate::graph::types::{GraphNodeKind, PayloadRef};
use crate::AppState;

use super::panel_lookup::find_content_panel_node;

#[derive(Debug, Deserialize)]
pub struct McgNodeQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
    /// Full stable id, e.g. `content_panel:inspection-stats` or block id `content_panel:content/inspection-stats`.
    #[serde(rename = "nodeId")]
    pub node_id: Option<String>,
    pub kind: Option<String>,
    pub key: Option<String>,
    #[serde(rename = "sceneId", default)]
    pub scene_id: Option<String>,
    #[serde(rename = "includeArtifact", default)]
    pub include_artifact: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct McgArtifactQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub kind: String,
    #[serde(rename = "contentHash")]
    pub content_hash: String,
}

pub async fn api_build_graph_mcg_node(
    State(state): State<AppState>,
    Query(query): Query<McgNodeQuery>,
) -> impl IntoResponse {
    if !graph_registry_enabled() {
        return not_found_json("MEI_GRAPH_REGISTRY disabled");
    }
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return bad_request_json("appId is required");
    }

    let mcg = McgRegistryWriter::load(state.source_root.as_path(), app_id);
    let scene_id = query
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home");

    let node = if let Some(node_id) = query
        .node_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        resolve_mcg_node(&mcg, node_id, scene_id)
    } else {
        let kind = query
            .kind
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let key = query
            .key
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        match (kind, key) {
            (Some(kind), Some(key)) => find_node_by_kind_key(&mcg, kind, key, scene_id),
            _ => None,
        }
    };

    let Some(node) = node else {
        return not_found_json("mcg node not found");
    };

    let include_artifact = query.include_artifact.unwrap_or(false);
    let app_root = resolve_app_root(state.source_root.as_path(), app_id);
    let artifact = if include_artifact {
        load_payload_json(app_root.as_path(), node.payload_ref.as_ref())
    } else {
        None
    };

    (
        StatusCode::OK,
        Json(json!({
            "node": node,
            "stableId": node.id.stable_key(),
            "artifact": artifact,
        })),
    )
        .into_response()
}

pub async fn api_build_graph_mcg_artifact(
    State(state): State<AppState>,
    Query(query): Query<McgArtifactQuery>,
) -> impl IntoResponse {
    if !graph_registry_enabled() {
        return not_found_json("MEI_GRAPH_REGISTRY disabled");
    }
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return bad_request_json("appId is required");
    }
    let kind = query.kind.trim();
    let hash = query.content_hash.trim();
    if kind.is_empty() || hash.is_empty() {
        return bad_request_json("kind and contentHash are required");
    }

    let app_root = resolve_app_root(state.source_root.as_path(), app_id);
    let pref = PayloadRef::new(kind, hash, "");
    let artifact = load_payload_json(app_root.as_path(), Some(&pref));
    match artifact {
        Some(value) => (StatusCode::OK, Json(value)).into_response(),
        None => not_found_json("artifact blob missing in content store"),
    }
}

fn resolve_mcg_node<'a>(
    mcg: &'a McgRegistry,
    node_id: &str,
    scene_id: &str,
) -> Option<&'a McgNodeRecord> {
    if let Ok(block_id) = parse_block_id(node_id) {
        if block_id.kind == GraphNodeKind::ContentPanel {
            if let Some(node) = find_content_panel_node(mcg, &block_id.key, scene_id) {
                return Some(node);
            }
        }
        if let Some(node) = mcg
            .nodes
            .iter()
            .find(|node| node.id.kind == block_id.kind && node.id.key == block_id.key)
        {
            return Some(node);
        }
    }

    if let Some((kind_slug, key)) = node_id.split_once(':') {
        if let Some(node) = find_node_by_kind_key(mcg, kind_slug, key, scene_id) {
            return Some(node);
        }
    }

    if node_id.contains('/') && !node_id.contains(':') {
        return find_content_panel_node(mcg, node_id, scene_id);
    }

    None
}

fn find_node_by_kind_key<'a>(
    mcg: &'a McgRegistry,
    kind_slug: &str,
    key: &str,
    scene_id: &str,
) -> Option<&'a McgNodeRecord> {
    let kind = graph_node_kind_from_slug(kind_slug)?;
    let candidates = if kind == GraphNodeKind::ContentPanel {
        super::panel_lookup::content_panel_lookup_keys(key, scene_id)
    } else {
        vec![key.to_string(), format!("{kind_slug}:{key}")]
    };
    for candidate in candidates {
        if let Some(node) = mcg
            .nodes
            .iter()
            .find(|node| node.id.kind == kind && node.id.key == candidate)
        {
            return Some(node);
        }
    }
    None
}

fn graph_node_kind_from_slug(slug: &str) -> Option<GraphNodeKind> {
    match slug.trim() {
        "app_skeleton" => Some(GraphNodeKind::AppSkeleton),
        "scene_payload" => Some(GraphNodeKind::ScenePayload),
        "content_panel" => Some(GraphNodeKind::ContentPanel),
        "catalog_resource" => Some(GraphNodeKind::CatalogResource),
        "metric_def_bundle" => Some(GraphNodeKind::MetricDefBundle),
        "semantic_graph" => Some(GraphNodeKind::SemanticGraph),
        "page_instance" => Some(GraphNodeKind::PageInstance),
        "data_source" => Some(GraphNodeKind::DataSource),
        "eval_plan" => Some(GraphNodeKind::EvalPlan),
        "workset" => Some(GraphNodeKind::Workset),
        "material_slot" => Some(GraphNodeKind::MaterialSlot),
        "navigation" => Some(GraphNodeKind::Navigation),
        "object_catalog" => Some(GraphNodeKind::ObjectCatalog),
        _ => None,
    }
}

fn load_payload_json(app_root: &Path, pref: Option<&PayloadRef>) -> Option<Value> {
    let pref = pref?;
    let hash = pref.content_hash.trim();
    if hash.is_empty() {
        return None;
    }
    let path = content_store::get(app_root, pref.kind.as_str(), hash)?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn bad_request_json(message: &str) -> axum::response::Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))).into_response()
}

fn not_found_json(message: &str) -> axum::response::Response {
    (StatusCode::NOT_FOUND, Json(json!({ "error": message }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_node_kind_from_slug_content_panel() {
        assert_eq!(
            graph_node_kind_from_slug("content_panel"),
            Some(GraphNodeKind::ContentPanel)
        );
    }
}
