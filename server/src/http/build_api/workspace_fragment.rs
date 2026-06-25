use axum::{
    extract::{Query, State},
    http::{header, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_lang_app::render_build_preview_fragment;
use mei_lang_kernel::{
    catalog_preview_target_for_build_node, compile_scene_from_build_node,
    compile_scene_from_build_node_with_app, preview_target_from_build_node,
    preview_target_from_build_node_with_app, resolve_app_root, resolve_build_view_query,
    resolve_components_root, BuildViewTab, CompileOptions, LegacyBuildQuery,
};
use serde::Serialize;
use serde_json::json;

use crate::http::compile_cache::{
    build_preview_diagnostic_error_count, resolve_build_preview_compile,
};
use crate::http::compile_cache::load_compile_artifact_only;
use crate::AppState;

#[derive(Debug, serde::Deserialize)]
pub struct BuildWorkspaceFragmentQuery {
    pub app_id: String,
    pub node: String,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Serialize)]
struct BuildWorkspaceFragmentResponse {
    compile_revision: String,
    compile_coordinate: mei_lang_kernel::BuildCompileCoordinate,
    preview_html: String,
    drilldown_script: String,
    node: String,
    focus: String,
}

pub async fn api_build_workspace_fragment(
    State(state): State<AppState>,
    Query(query): Query<BuildWorkspaceFragmentQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "app_id is required");
    }
    let node_raw = query.node.trim();
    if node_raw.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "node is required");
    }
    let legacy = LegacyBuildQuery {
        file: None,
        scene: None,
        world_metric: None,
        world_dataset: None,
        explain: None,
        tab: query.tab.clone(),
    };
    let Some(resolved) = resolve_build_view_query(
        Some(node_raw),
        query.scope.as_deref(),
        query.tab.as_deref(),
        &legacy,
    ) else {
        return json_error(StatusCode::BAD_REQUEST, "invalid node id");
    };
    let tab = query
        .tab
        .as_deref()
        .and_then(BuildViewTab::parse_slug)
        .unwrap_or(resolved.tab);
    if tab != BuildViewTab::Preview {
        return json_error(
            StatusCode::BAD_REQUEST,
            "workspace fragment requires preview tab",
        );
    }

    let components_root = resolve_components_root(&state.source_root);
    let mut preview_target = preview_target_from_build_node(&resolved.node);
    let mut scene_hint = compile_scene_from_build_node(&resolved.node);
    if preview_target.is_none() || scene_hint.is_none() {
        if let Some(probe) = load_compile_artifact_only(
            &state,
            app_id,
            &CompileOptions::default(),
            components_root.as_path(),
        ) {
            if preview_target.is_none() {
                preview_target = preview_target_from_build_node_with_app(
                    &resolved.node,
                    Some(&probe.compiled),
                );
            }
            if scene_hint.is_none() {
                scene_hint = compile_scene_from_build_node_with_app(
                    &resolved.node,
                    Some(&probe.compiled),
                );
            }
        }
    }
    if preview_target.is_none() {
        preview_target = catalog_preview_target_for_build_node(
            resolve_app_root(state.source_root.as_path(), app_id).as_path(),
            &resolved.node,
        );
    }
    let compile_options = CompileOptions {
        scene: scene_hint,
        preview_target: preview_target.clone(),
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
            return json_error(
                StatusCode::NOT_FOUND,
                "compile artifact missing for node scope",
            );
        }
        Err(failure) => {
            let message = failure.error.to_string();
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, message.as_str());
        }
    };
    if build_preview_diagnostic_error_count(&outcome.compiled) > 0 {
        tracing::warn!(
            app_id = %app_id,
            node = %node_raw,
            error_count = build_preview_diagnostic_error_count(&outcome.compiled),
            "build preview fragment compiled with diagnostics; rendering degraded preview"
        );
    }
    let compile_cache_hit = outcome.cache_hit;
    let preview_target = preview_target.or_else(|| {
        preview_target_from_build_node_with_app(&resolved.node, Some(&outcome.compiled))
    });
    let _ = preview_target;
    let app_path = format!("/apps/build/{app_id}");
    let Some(fragment) = render_build_preview_fragment(
        &[],
        &outcome.compiled,
        app_path.as_str(),
        Some(node_raw),
        query.scope.as_deref(),
        query.focus.as_deref(),
        Some("preview"),
    ) else {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to render preview fragment",
        );
    };
    let body = BuildWorkspaceFragmentResponse {
        compile_revision: outcome.compile_revision.clone(),
        compile_coordinate: fragment.compile_coordinate.clone(),
        preview_html: fragment.preview_html,
        drilldown_script: fragment.drilldown_script,
        node: fragment.node,
        focus: fragment.focus,
    };
    let mut response = Json(body).into_response();
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    if let Ok(value) = HeaderValue::from_str(if compile_cache_hit { "hit" } else { "miss" }) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-compile-cache"), value);
    }
    response.headers_mut().insert(
        HeaderName::from_static("x-mei-nav-tier"),
        HeaderValue::from_static("fragment"),
    );
    response
}

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}
