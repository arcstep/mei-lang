//! Scoped panel preview HTML for build-view debugging (content panels + scene panels).

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use mei_lang_app::render_build_preview_fragment;
use mei_lang_kernel::{
    resolve_app_root, resolve_components_root, CompileOptions,
};
use serde::Deserialize;
use serde_json::json;

use crate::graph::feature::graph_registry_enabled;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::http::compile_cache::{
    build_preview_diagnostic_error_count, resolve_build_preview_compile,
};
use crate::AppState;

use super::panel_lookup::{find_panel_contract_node, panel_preview_target};

#[derive(Debug, Deserialize)]
pub struct PanelRenderQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
    /// Content ref (`content/inspection-stats`) or scene panel (`home:left_rail`).
    #[serde(rename = "panelKey")]
    pub panel_key: String,
    #[serde(rename = "sceneId", default)]
    pub scene_id: Option<String>,
}

pub async fn api_build_panel_render(
    State(state): State<AppState>,
    Query(query): Query<PanelRenderQuery>,
) -> impl IntoResponse {
    if !graph_registry_enabled() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "MEI_GRAPH_REGISTRY disabled"})),
        )
            .into_response();
    }

    let app_id = query.app_id.trim();
    let panel_key = query.panel_key.trim();
    if app_id.is_empty() || panel_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "appId and panelKey are required"})),
        )
            .into_response();
    }

    let scene_id = query
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home");

    let mcg = McgRegistryWriter::load(state.source_root.as_path(), app_id);
    let node = find_panel_contract_node(&mcg, panel_key, scene_id);
    let preview_target = panel_preview_target(panel_key);
    let app_root = resolve_app_root(state.source_root.as_path(), app_id);
    let preview_path = app_root.join(&preview_target);
    if !preview_path.is_file() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "panel source file missing",
                "panelKey": panel_key,
                "previewTarget": preview_target,
            })),
        )
            .into_response();
    }

    let components_root = resolve_components_root(&state.source_root);
    let compile_options = CompileOptions {
        scene: Some(scene_id.to_string()),
        preview_target: Some(preview_target.clone()),
        ..Default::default()
    };
    let compile_result = resolve_build_preview_compile(
        &state,
        app_id,
        &compile_options,
        components_root.as_path(),
    );
    let outcome = match compile_result {
        Ok(Some(outcome)) => outcome,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "compile artifact missing for panel preview"})),
            )
                .into_response();
        }
        Err(failure) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": failure.error.to_string()})),
            )
                .into_response();
        }
    };

    if build_preview_diagnostic_error_count(&outcome.compiled) > 0 {
        tracing::warn!(
            app_id = %app_id,
            panel_key = %panel_key,
            error_count = build_preview_diagnostic_error_count(&outcome.compiled),
            "panel render preview compiled with diagnostics"
        );
    }

    let app_path = app_id.to_string();
    let Some(fragment) = render_build_preview_fragment(
        &[],
        &outcome.compiled,
        app_path.as_str(),
        None,
        None,
        None,
        Some("preview"),
    ) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "failed to render panel preview fragment"})),
        )
            .into_response();
    };

    (
        StatusCode::OK,
        Json(json!({
            "panelKey": panel_key,
            "sceneId": scene_id,
            "previewTarget": preview_target,
            "mcgNode": node,
            "compileRevision": outcome.compile_revision,
            "previewHtml": fragment.preview_html,
            "drilldownScript": fragment.drilldown_script,
            "workspaceScripts": fragment.workspace_scripts,
        })),
    )
        .into_response()
}
