//! Workspace scene theme library catalog + studio read/write.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use mei_lang_kernel::{
    load_workspace_config, merge_scene_theme_studio_patch, resolve_workspace_scene_theme_value,
    scene_theme_studio_editable_keys, workspace_config_path, write_workspace_config,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::SharedState;

/// `GET /api/ops/scene-themes` → `{ themes: [{ id, label, value, swatches }], default, editable }`
pub async fn api_ops_scene_themes_get(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let workspace = load_workspace_config(workspace_root.as_path());
    let themes = mei_lang_kernel::list_workspace_scene_theme_catalog(&workspace);
    let default_id = workspace
        .ops
        .scene_theme_default
        .clone()
        .unwrap_or_else(|| "cockpit".to_string());
    (
        StatusCode::OK,
        Json(json!({
            "themes": themes,
            "default": default_id,
            "editable": scene_theme_studio_editable_keys(),
        })),
    )
        .into_response()
}

/// `GET /api/ops/scene-themes/:theme_id`
pub async fn api_ops_scene_theme_get(
    State(state): State<SharedState>,
    Path(theme_id): Path<String>,
) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let workspace = load_workspace_config(workspace_root.as_path());
    let Some(theme) = resolve_workspace_scene_theme_value(&workspace, &theme_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("scene theme not found: {theme_id}") })),
        )
            .into_response();
    };
    (
        StatusCode::OK,
        Json(json!({
            "id": theme_id,
            "theme": theme,
            "editable": scene_theme_studio_editable_keys(),
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct SceneThemePutBody {
    #[serde(default)]
    theme: Option<Value>,
    #[serde(default)]
    patch: Option<Value>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    font: Option<Value>,
    #[serde(default)]
    tokens: Option<Value>,
}

/// `PUT /api/ops/scene-themes/:theme_id` — merge studio patch into workspace sceneThemes.
pub async fn api_ops_scene_theme_put(
    State(state): State<SharedState>,
    Path(theme_id): Path<String>,
    Json(body): Json<SceneThemePutBody>,
) -> impl IntoResponse {
    let id = theme_id.trim();
    if id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "theme_id required" })),
        )
            .into_response();
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);

    let config_path = workspace_config_path(workspace_root.as_path());
    let mut workspace = load_workspace_config(workspace_root.as_path());
    let Some(existing) = workspace.ops.scene_themes.get(id).cloned() else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("scene theme not found: {id}") })),
        )
            .into_response();
    };

    let patch = if let Some(theme) = body.theme {
        theme
    } else if let Some(patch) = body.patch {
        patch
    } else {
        let mut map = serde_json::Map::new();
        if let Some(label) = body.label {
            map.insert("label".to_string(), Value::String(label));
        }
        if let Some(font) = body.font {
            map.insert("font".to_string(), font);
        }
        if let Some(tokens) = body.tokens {
            map.insert("tokens".to_string(), tokens);
        }
        Value::Object(map)
    };

    let merged = merge_scene_theme_studio_patch(&existing, &patch);
    workspace
        .ops
        .scene_themes
        .insert(id.to_string(), merged.clone());
    if let Err(error) = write_workspace_config(&config_path, &workspace) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("failed to write workspace.json: {error}") })),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "id": id,
            "theme": merged,
            "editable": scene_theme_studio_editable_keys(),
        })),
    )
        .into_response()
}
