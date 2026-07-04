use axum::{
    extract::{Query, State},
    http::{header, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_lang_app::render_build_preview_fragment;
use mei_lang_kernel::{resolve_build_view_query, BuildViewTab, LegacyBuildQuery};
use serde::Serialize;
use serde_json::json;

use crate::build_api::assemble::{assemble_enriched_for_build_node, AssembleBuildError};
use crate::state::SharedState;

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
    State(state): State<SharedState>,
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

    let workspace_root = {
        let guard = state.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };

    let assembled = match assemble_enriched_for_build_node(
        workspace_root.as_path(),
        app_id,
        node_raw,
        None,
    ) {
        Ok(value) => value,
        Err(AssembleBuildError::InvalidNode) => {
            return json_error(StatusCode::BAD_REQUEST, "invalid node id");
        }
        Err(error) => {
            let status = match &error {
                AssembleBuildError::NotAssembled(_) => StatusCode::SERVICE_UNAVAILABLE,
                AssembleBuildError::AssembleFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
                AssembleBuildError::InvalidNode => StatusCode::BAD_REQUEST,
            };
            return (
                status,
                Json(json!({
                    "error": error.message(),
                })),
            )
                .into_response();
        }
    };

    let app_path = app_id.to_string();
    let Some(fragment) = render_build_preview_fragment(
        &[],
        &assembled.compiled,
        app_path.as_str(),
        Some(node_raw),
        query.scope.as_deref(),
        query.focus.as_deref(),
        Some("preview"),
        query.review_projection.as_deref(),
    ) else {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to render preview fragment",
        );
    };

    let body = BuildWorkspaceFragmentResponse {
        compile_revision: assembled.compile_revision.clone(),
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
    response.headers_mut().insert(
        HeaderName::from_static("x-mei-nav-tier"),
        HeaderValue::from_static("fragment"),
    );
    if let Ok(value) = HeaderValue::from_str(assembled.compile_revision.as_str()) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-compile-revision"), value);
    }
    response
}

fn json_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}
