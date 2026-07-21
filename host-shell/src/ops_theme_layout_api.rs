//! `ops.themes.*.layout` overlay hot-read + apply (0327 D3).

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use mei_lang_kernel::{
    apply_ops_patch_with_journal, load_mei_config_for_app, merge_theme_layout_draft_into_theme,
    ops_theme_layout_revision_digest, resolve_app_root, resolve_mei_config_path,
    theme_layout_overlay_keys, OpsConfigPatch,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::draft_session::resolve_draft_session_id;
use crate::state::SharedState;

#[derive(Debug, serde::Serialize)]
struct ThemeLayoutOverlayResponse {
    app_id: String,
    theme_id: String,
    session_id: String,
    revision: String,
    themes_revision: String,
    draft_active: bool,
    entries: BTreeMap<String, Value>,
}

fn resolve_scene_theme_id(
    workspace: Option<&mei_lang_kernel::WorkspaceConfig>,
    config: &mei_lang_kernel::MeiConfig,
) -> String {
    mei_lang_kernel::resolve_active_scene_theme_id(workspace, config, None)
}

pub async fn api_ops_theme_layout_overlay_get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
) -> impl IntoResponse {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app_id is required"})),
        )
            .into_response();
    }
    let session_id = resolve_draft_session_id(&headers);
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.clone();
    let app_ctx = guard.host_ctx_for_app(app_id);
    let config =
        load_mei_config_for_app(app_ctx.app_root().as_path(), Some(workspace_root.as_path()));
    let workspace = mei_lang_kernel::load_workspace_config(workspace_root.as_path());
    let theme_id = resolve_scene_theme_id(Some(&workspace), &config);
    let assembled = mei_lang_kernel::resolve_assembled_scene_theme(
        Some(&workspace),
        &config,
        theme_id.as_str(),
    );
    let persisted_layout = assembled
        .as_ref()
        .and_then(|theme| theme.get("layout"))
        .or_else(|| config.ops.extensions.get("layout"));
    let revision = ops_theme_layout_revision_digest(&config, theme_id.as_str());
    let themes_revision =
        mei_lang_kernel::ops_active_theme_revision_digest(Some(&workspace), &config);
    let entries = persisted_layout
        .map(theme_layout_overlay_keys)
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(ThemeLayoutOverlayResponse {
            app_id: app_id.to_string(),
            theme_id,
            session_id,
            revision,
            themes_revision,
            draft_active: false,
            entries,
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct ThemeLayoutDraftRequest {
    #[serde(default)]
    pub layout: Value,
}

pub async fn api_ops_theme_layout_apply_post(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(app_id): Path<String>,
    body: Option<Json<ThemeLayoutDraftRequest>>,
) -> impl IntoResponse {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app_id is required"})),
        )
            .into_response();
    }
    let _session_id = resolve_draft_session_id(&headers);
    let workspace_root = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let client_layout = body
        .as_ref()
        .map(|Json(req)| req.layout.clone())
        .filter(|value| !value.is_null() && value.as_object().is_some_and(|obj| !obj.is_empty()));
    let Some(draft_value) = client_layout else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "no theme.layout draft",
                "hint": "POST layout from MeiDraftLayerStore themeLayout session",
            })),
        )
            .into_response();
    };
    let app_root = resolve_app_root(workspace_root.as_path(), app_id);
    let config_path = resolve_mei_config_path(app_root.as_path(), Some(workspace_root.as_path()));
    let config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root.as_path()));
    let workspace = mei_lang_kernel::load_workspace_config(workspace_root.as_path());
    let theme_id = resolve_scene_theme_id(Some(&workspace), &config);
    let existing_theme = config
        .ops
        .themes
        .get(theme_id.as_str())
        .cloned()
        .or_else(|| config.ops.themes.get("_layout").cloned())
        .unwrap_or_else(|| {
            config
                .ops
                .extensions
                .get("layout")
                .cloned()
                .map(|layout| json!({ "layout": layout }))
                .unwrap_or_else(|| json!({}))
        });
    let merged_theme = merge_theme_layout_draft_into_theme(&existing_theme, &draft_value);
    // Keep layout under app-owned theme slot `_layout` so color library themes stay clean.
    let patch = OpsConfigPatch {
        themes: Some(BTreeMap::from([("_layout".to_string(), merged_theme)])),
        ..Default::default()
    };
    match apply_ops_patch_with_journal(
        app_root.as_path(),
        config_path.as_path(),
        "build-theme-layout",
        "apply theme.layout draft to ops.themes._layout",
        &patch,
    ) {
        Ok((updated, entry)) => {
            crate::access_page_cache::clear_legacy_page_render_cache_for_app(
                workspace_root.as_path(),
                app_id,
            );
            (
                StatusCode::OK,
                Json(json!({
                    "ok": true,
                    "revision": entry.revision,
                    "theme_layout_revision": ops_theme_layout_revision_digest(&updated, "_layout"),
                    "themes_revision": mei_lang_kernel::ops_active_theme_revision_digest(
                        Some(&workspace),
                        &updated,
                    ),
                })),
            )
                .into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_layout_overlay_response_fields_serialize() {
        let resp = ThemeLayoutOverlayResponse {
            app_id: "zhifa".to_string(),
            theme_id: "cockpit".to_string(),
            session_id: "s".to_string(),
            revision: "r".to_string(),
            themes_revision: "t".to_string(),
            draft_active: false,
            entries: BTreeMap::new(),
        };
        assert_eq!(resp.theme_id, "cockpit");
    }
}
