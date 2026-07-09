//! Read-only MCG graph HTTP handlers for host-shell.

use std::path::Path;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use mei_host_graph::{
    read_payload_json, GraphNodeKind, McgNodeRecord, McgRegistry, McgRegistryWriter, PayloadRef,
};
use mei_lang_kernel::resolve_app_root;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::SharedState;

#[derive(Debug, Deserialize)]
pub struct GraphAppQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
}

#[derive(Debug, Deserialize)]
pub struct McgNodeQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
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

pub async fn api_build_graph_mcg(
    State(state): State<SharedState>,
    Query(params): Query<GraphAppQuery>,
) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    graph_read_response(
        guard.ctx.workspace_root.as_path(),
        params.app_id.as_str(),
        |root, app_id| McgRegistryWriter::load(root, app_id),
    )
}

pub async fn api_build_graph_mcg_node(
    State(state): State<SharedState>,
    Query(query): Query<McgNodeQuery>,
) -> impl IntoResponse {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return bad_request_json("appId is required");
    }
    let guard = state.read().expect("state lock");
    let workspace = guard.ctx.workspace_root.as_path();
    let mcg = McgRegistryWriter::load(workspace, app_id);
    let scene_id = query
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home");

    let node = if let Some(node_id) = query.node_id.as_deref().map(str::trim).filter(|v| !v.is_empty())
    {
        resolve_mcg_node(&mcg, node_id, scene_id)
    } else {
        let kind = query.kind.as_deref().map(str::trim).filter(|v| !v.is_empty());
        let key = query.key.as_deref().map(str::trim).filter(|v| !v.is_empty());
        match (kind, key) {
            (Some(kind), Some(key)) => find_node_by_kind_key(&mcg, kind, key, scene_id),
            _ => None,
        }
    };

    let Some(node) = node else {
        return not_found_json("mcg node not found");
    };

    let include_artifact = query.include_artifact.unwrap_or(false);
    let app_root = resolve_app_root(workspace, app_id);
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
    State(state): State<SharedState>,
    Query(query): Query<McgArtifactQuery>,
) -> impl IntoResponse {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return bad_request_json("appId is required");
    }
    let kind = query.kind.trim();
    let hash = query.content_hash.trim();
    if kind.is_empty() || hash.is_empty() {
        return bad_request_json("kind and contentHash are required");
    }
    let guard = state.read().expect("state lock");
    let app_root = resolve_app_root(guard.ctx.workspace_root.as_path(), app_id);
    let pref = PayloadRef::new(kind, hash, "");
    let artifact = load_payload_json(app_root.as_path(), Some(&pref));
    match artifact {
        Some(value) => (StatusCode::OK, Json(value)).into_response(),
        None => not_found_json("artifact blob missing in content store"),
    }
}

fn graph_read_response<T: serde::Serialize>(
    source_root: &Path,
    app_id: &str,
    load: impl FnOnce(&Path, &str) -> T,
) -> axum::response::Response {
    if app_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "appId is required"})),
        )
            .into_response();
    }
    let registry = load(source_root, app_id.trim());
    (StatusCode::OK, Json(registry)).into_response()
}

fn resolve_mcg_node<'a>(
    mcg: &'a McgRegistry,
    node_id: &str,
    scene_id: &str,
) -> Option<&'a McgNodeRecord> {
    if let Some((kind_slug, key)) = node_id.split_once(':') {
        if let Some(node) = find_node_by_kind_key(mcg, kind_slug, key, scene_id) {
            return Some(node);
        }
    }

    if node_id.contains('/') && !node_id.contains(':') {
        return find_content_panel_node_local(mcg, node_id, scene_id);
    }

    mcg.nodes.iter().find(|node| node.id.stable_key() == node_id)
}

fn find_node_by_kind_key<'a>(
    mcg: &'a McgRegistry,
    kind_slug: &str,
    key: &str,
    scene_id: &str,
) -> Option<&'a McgNodeRecord> {
    let kind = graph_node_kind_from_slug(kind_slug)?;
    if kind == GraphNodeKind::ContentPanel {
        return find_content_panel_node_local(mcg, key, scene_id);
    }
    mcg.nodes
        .iter()
        .find(|node| node.id.kind == kind && node.id.key == key)
}

fn find_content_panel_node_local<'a>(
    mcg: &'a McgRegistry,
    panel_key: &str,
    scene_id: &str,
) -> Option<&'a McgNodeRecord> {
    let mut keys = vec![
        format!("content_panel:{panel_key}"),
        panel_key.to_string(),
        format!("content_panel:{scene_id}:{panel_key}"),
        format!("{scene_id}:{panel_key}"),
    ];
    if let Some(basename) = panel_key.rsplit('/').next() {
        if basename != panel_key {
            keys.push(format!("content_panel:{basename}"));
            keys.push(basename.to_string());
        }
    }
    for key in keys {
        if let Some(node) = mcg.nodes.iter().find(|node| {
            node.id.kind == GraphNodeKind::ContentPanel && node.id.key == key
        }) {
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
        "warmup_policy" => Some(GraphNodeKind::WarmupPolicy),
        "world_model" => Some(GraphNodeKind::WorldModel),
        _ => None,
    }
}

fn load_payload_json(app_root: &Path, pref: Option<&PayloadRef>) -> Option<Value> {
    let pref = pref?;
    read_payload_json(app_root, pref).ok().flatten()
}

fn bad_request_json(message: &str) -> axum::response::Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": message })),
    )
        .into_response()
}

fn not_found_json(message: &str) -> axum::response::Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": message })),
    )
        .into_response()
}
