use axum::{
    extract::{Query, State},
    http::{header, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_lang_app::{render_build_preview_fragment, UiRouteMode};
use mei_lang_kernel::{
    preview_target_from_build_node_with_app, resolve_build_view_query, resolve_components_root,
    BuildViewTab, CompileOptions, LegacyBuildQuery,
};
use serde::Serialize;
use serde_json::json;

use crate::http::build_preview::resolve_build_node_compile_hints;
use crate::http::compile_cache::{
    build_preview_diagnostic_error_count, resolve_build_preview_compile,
};
use crate::http::pages::AppQuery;
use crate::readiness::scope_gate::resolve_scope_gate;
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
    #[serde(default)]
    pub review_projection: Option<String>,
    #[serde(default)]
    pub data_mode: Option<String>,
}

#[derive(Debug, Serialize)]
struct BuildWorkspaceFragmentResponse {
    compile_revision: String,
    compile_coordinate: mei_lang_kernel::BuildCompileCoordinate,
    preview_html: String,
    drilldown_script: String,
    workspace_scripts: Vec<String>,
    node: String,
    focus: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_projection: Option<String>,
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
    let hints = resolve_build_node_compile_hints(
        &state,
        app_id,
        &resolved.node,
        components_root.as_path(),
    );
    let preview_target = hints.preview_target;
    let scene_hint = hints.scene;
    let compile_options = CompileOptions {
        scene: scene_hint.clone(),
        preview_target: preview_target.clone(),
        ..Default::default()
    };
    let gate_query = AppQuery {
        file: preview_target.clone(),
        scene: scene_hint.clone(),
        tab: Some("preview".to_string()),
        diag_filter: None,
        world_metric: None,
        world_dataset: None,
        explain: None,
        node: Some(node_raw.to_string()),
        scope: query.scope.clone(),
        focus: query.focus.clone(),
        chrome: None,
        catalog: None,
        pack: None,
        data_mode: None,
        review_projection: None,
    };
    let gate = resolve_scope_gate(
        state.source_root.as_path(),
        app_id,
        UiRouteMode::Build,
        compile_options.scene.as_deref(),
        &gate_query,
    );
    let compile_result = if !gate.shell_ready
        && compile_options
            .preview_target
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
    {
        resolve_build_preview_compile(
            &state,
            app_id,
            &compile_options,
            components_root.as_path(),
        )
    } else if gate.shell_ready {
        resolve_build_preview_compile(
            &state,
            app_id,
            &compile_options,
            components_root.as_path(),
        )
    } else {
        Ok(None)
    };
    if compile_result.as_ref().ok().and_then(|value| value.as_ref()).is_none() && !gate.shell_ready {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "error": "preview scope not ready",
                "blockers": gate.blockers,
                "retry_after_ms": 3000,
            })),
        )
            .into_response();
    }
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
    let app_path = app_id.to_string();
    let Some(fragment) = render_build_preview_fragment(
        &[],
        &outcome.compiled,
        app_path.as_str(),
        Some(node_raw),
        query.scope.as_deref(),
        query.focus.as_deref(),
        Some("preview"),
        query.data_mode.as_deref(),
        query.review_projection.as_deref(),
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
        workspace_scripts: fragment.workspace_scripts,
        node: fragment.node,
        focus: fragment.focus,
        data_mode: query.data_mode.clone(),
        review_projection: query.review_projection.clone(),
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
    if let Ok(value) = HeaderValue::from_str(if compile_cache_hit { "1" } else { "0" }) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-compile-cache-hit"), value);
    }
    if let Ok(value) = HeaderValue::from_str(outcome.compile_revision.as_str()) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-compile-revision"), value);
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
