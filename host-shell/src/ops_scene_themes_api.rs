//! Workspace scene theme library catalog (read-only for Admin selects).

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::state::SharedState;

/// `GET /api/ops/scene-themes` → `{ themes: [{ id, label, value }] }`
pub async fn api_ops_scene_themes_get(State(state): State<SharedState>) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    drop(guard);
    let workspace = mei_lang_kernel::load_workspace_config(workspace_root.as_path());
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
        })),
    )
        .into_response()
}
