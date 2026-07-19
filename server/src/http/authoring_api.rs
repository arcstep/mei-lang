//! Restricted authoring API for scene scaffolding (0332: no grid/metric JSON plans).

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::AppState;

fn resolve_app_root(state: &AppState, app_id: &str) -> Option<PathBuf> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return None;
    }
    let root = mei_lang_kernel::resolve_app_root(state.source_root.as_path(), app_id);
    if root.is_dir() {
        Some(root)
    } else {
        None
    }
}

fn normalize_rel(rel: &str) -> Option<PathBuf> {
    let rel = rel.trim().trim_start_matches('/');
    if rel.is_empty() || rel.contains("..") {
        return None;
    }
    let path = PathBuf::from(rel);
    if path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::RootDir))
    {
        return None;
    }
    Some(path)
}

pub async fn authoring_structure_get(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
) -> impl IntoResponse {
    let Some(app_root) = resolve_app_root(&state, &app_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    };
    let scene_root = app_root.join("src/scene");
    let mut nodes = Vec::new();
    if scene_root.is_dir() {
        for entry in walkdir::WalkDir::new(&scene_root).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let rel = path
                .strip_prefix(&app_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let kind =
                if rel.ends_with("plane.mei") || rel.contains("/plane-") && rel.ends_with(".mei") {
                    "plane"
                } else if rel.ends_with("region.mei")
                    || (Path::new(&rel)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|n| n.starts_with("r-") && n.ends_with(".mei")))
                {
                    "region"
                } else if rel.ends_with("section.mei")
                    || (Path::new(&rel)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|n| n.starts_with("s-") && n.ends_with(".mei")))
                {
                    "section"
                } else if rel.ends_with("assembly.mei") {
                    "assembly"
                } else if rel.ends_with("layout.mei") {
                    "layout"
                } else if rel.ends_with("content.mei") {
                    "content"
                } else if rel.ends_with(".mei") {
                    "mei"
                } else {
                    continue;
                };
            nodes.push(json!({
                "path": rel,
                "kind": kind,
                "name": entry.file_name().to_string_lossy(),
            }));
        }
    }
    nodes.sort_by(|a, b| {
        a.get("path")
            .and_then(Value::as_str)
            .cmp(&b.get("path").and_then(Value::as_str))
    });
    (
        StatusCode::OK,
        Json(json!({
            "app_id": app_id,
            "nodes": nodes,
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct ScaffoldBody {
    /// Relative under src/scene, e.g. `home/t1/r-new/s-panel`
    pub node_path: String,
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    "section".to_string()
}

pub async fn authoring_scaffold(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    Json(body): Json<ScaffoldBody>,
) -> impl IntoResponse {
    let Some(app_root) = resolve_app_root(&state, &app_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    };
    let Some(rel) = normalize_rel(&format!(
        "src/scene/{}",
        body.node_path.trim_start_matches('/')
    )) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid node_path"})),
        )
            .into_response();
    };
    let dir = app_root.join(&rel);
    let folder_name = dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("node")
        .to_string();
    let id = folder_name
        .trim_start_matches("section-")
        .trim_start_matches("region-")
        .trim_start_matches("plane-")
        .trim_start_matches("s-")
        .trim_start_matches("r-")
        .trim_start_matches("p-")
        .to_string();

    // T2 plane: default flat `plane-{id}.mei` (025004). Only region/section use role-named roots in a folder.
    if body.kind.as_str() == "plane" {
        let parent = dir.parent().unwrap_or(app_root.as_path());
        if let Err(err) = fs::create_dir_all(parent) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("mkdir failed: {err}")})),
            )
                .into_response();
        }
        let plane_file_name = if folder_name.starts_with("plane-") {
            format!("{folder_name}.mei")
        } else {
            format!("plane-{id}.mei")
        };
        let decl_path = parent.join(&plane_file_name);
        let stub = format!(
            "plane_layout(\n    id = \"{id}\",\n    tier = \"t2\",\n    layout = grid(rows = [\"1fr\"], columns = [\"1fr\"], areas = [[\"main\"]]),\n    regions = [],\n)\n"
        );
        let _ = fs::write(&decl_path, stub);
        let rel_file = decl_path
            .strip_prefix(&app_root)
            .unwrap_or(&decl_path)
            .to_string_lossy()
            .replace('\\', "/");
        return (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "path": rel_file,
                "files": [rel_file],
            })),
        )
            .into_response();
    }

    if let Err(err) = fs::create_dir_all(&dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("mkdir failed: {err}")})),
        )
            .into_response();
    }
    let (decl_name, stub) = match body.kind.as_str() {
        "region" => (
            "region.mei",
            format!(
                "region_layout(\n    id = \"{id}\",\n    area = \"{id}\",\n    layout = grid(rows = [\"1fr\"], columns = [\"1fr\"], areas = [[\"body\"]]),\n    sections = [],\n)\n"
            ),
        ),
        _ => (
            "section.mei",
            format!(
                "section_layout(\n    id = \"{id}\",\n    area = \"{id}\",\n    title = \"{id}\",\n    layout = grid(\n        rows = [\"auto\", \"1fr\"],\n        columns = [\"1fr\"],\n        areas = [[\"title\"], [\"body\"]],\n    ),\n    shell = section_shell(\n        title = \"{id}\",\n        body = content_panel(\n            id = \"{id}\",\n            layout = grid(rows = [\"1fr\"], columns = [\"1fr\"], areas = [[\"body\"]]),\n            blocks = [],\n        ),\n    ),\n)\n"
            ),
        ),
    };
    let decl_path = dir.join(decl_name);
    let _ = fs::write(&decl_path, stub);
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "path": rel.to_string_lossy(),
            "files": [
                decl_path.strip_prefix(&app_root).unwrap_or(&decl_path).to_string_lossy(),
            ]
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct RemoveNodeBody {
    pub node_path: String,
}

pub async fn authoring_remove_node(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    Json(body): Json<RemoveNodeBody>,
) -> impl IntoResponse {
    let Some(app_root) = resolve_app_root(&state, &app_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    };
    let Some(rel) = normalize_rel(&format!(
        "src/scene/{}",
        body.node_path.trim_start_matches('/')
    )) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid node_path"})),
        )
            .into_response();
    };
    let dir = app_root.join(&rel);
    if !dir.starts_with(app_root.join("src/scene")) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "can only remove under src/scene"})),
        )
            .into_response();
    }
    match fs::remove_dir_all(&dir) {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({"ok": true, "path": rel.to_string_lossy()})),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("remove failed: {err}")})),
        )
            .into_response(),
    }
}
