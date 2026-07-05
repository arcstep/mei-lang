//! Unified view-revision API: revision-first layer negotiation.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_host_auth::AuthServeState;
use mei_lang_app::UiRouteMode;
use serde::Deserialize;
use serde_json::json;

use crate::artifact_observability::{ArtifactHitMatrix, LayerArtifactObservability};
use crate::pages::AppQuery;
use crate::review_axes::resolve_page_render_axes;
use crate::scene_manifest::{
    build_scene_view_manifest, materialize_layers_for_request, resolve_route_mode_from_surface,
};
use crate::state::SharedState;

#[derive(Debug, Deserialize, Default)]
pub struct ViewRevisionQuery {
    pub app_id: String,
    pub scene: Option<String>,
    #[serde(default)]
    pub surface: Option<String>,
    #[serde(default)]
    pub compose: Option<String>,
    #[serde(default)]
    pub client_layers: Option<String>,
    #[serde(default)]
    pub local_miss: Option<String>,
    #[serde(default)]
    pub missing_layers: Option<String>,
    #[serde(default)]
    pub node: Option<String>,
    #[serde(default)]
    pub data_mode: Option<String>,
    #[serde(default)]
    pub review_projection: Option<String>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub chrome: Option<String>,
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

fn parse_bool_flag(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

fn parse_client_layers(raw: Option<&str>) -> Vec<mei_host_graph::ClientLayerHolding> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };
    serde_json::from_str(raw).unwrap_or_default()
}

fn parse_missing_layers(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };
    if raw.starts_with('[') {
        return serde_json::from_str(raw).unwrap_or_default();
    }
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_compose_request(
    raw: Option<&str>,
    fallback: &mei_host_graph::ComposeRequest,
) -> mei_host_graph::ComposeRequest {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return fallback.clone();
    };
    serde_json::from_str(raw).unwrap_or_else(|_| fallback.clone())
}

fn surface_revision_digest(manifest: &mei_host_graph::SceneViewManifest) -> Option<String> {
    mei_host_graph::surface_revision_digest_from_manifest(manifest)
}

fn compose_from_query(query: &ViewRevisionQuery, route_mode: UiRouteMode) -> mei_host_graph::ComposeRequest {
    mei_host_graph::ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: query.tab.clone(),
        chrome: query.chrome.clone(),
        review_projection: query.review_projection.clone(),
        data_mode: query.data_mode.clone(),
        focus: query.focus.clone(),
        scope: query.scope.clone(),
    }
}

pub(crate) fn resolve_view_revision_for_surface(
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    data_mode: mei_lang_kernel::DataMode,
    compose: &mei_host_graph::ComposeRequest,
    draft_session: &str,
    draft_digest: &str,
    client_layers: Vec<mei_host_graph::ClientLayerHolding>,
    local_miss: bool,
    missing_layers: Vec<String>,
    hits: &mut ArtifactHitMatrix,
) -> anyhow::Result<mei_host_graph::ViewRevisionResponse> {
    let manifest = build_scene_view_manifest(
        workspace_root,
        app_id,
        scene_id,
        route_mode,
        data_mode,
        compose,
        draft_session,
        draft_digest,
        hits,
    )?;
    let surface_digest = surface_revision_digest(&manifest);
    let mut response = mei_host_graph::resolve_view_revision(&mei_host_graph::ViewRevisionInput {
        manifest: manifest.clone(),
        client_layers,
        local_miss,
        missing_layers,
        surface_revision_digest: surface_digest,
    });
    if response.status == mei_host_graph::ViewRevisionStatus::Refetch
        && !response.changed_layers.is_empty()
        && response.changed_layers.len() <= 5
    {
        let inline = materialize_layers_for_request(
            workspace_root,
            app_id,
            scene_id,
            route_mode,
            data_mode,
            compose,
            draft_session,
            draft_digest,
            &response.changed_layers,
            hits,
        )?;
        response.inline_layers = Some(inline);
    }
    Ok(response)
}

fn apply_view_revision_headers(response: &mut Response, revision: &mei_host_graph::ViewRevisionResponse) {
    let status = match revision.status {
        mei_host_graph::ViewRevisionStatus::Refetch => "refetch",
        mei_host_graph::ViewRevisionStatus::AssembleLocal => "assemble_local",
    };
    if let Ok(value) = HeaderValue::from_str(status) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-view-revision-status"),
            value,
        );
    }
    let assemble_local = revision.status == mei_host_graph::ViewRevisionStatus::AssembleLocal;
    if let Ok(value) = HeaderValue::from_str(if assemble_local { "1" } else { "0" }) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-assemble-local"), value);
    }
}

pub async fn api_host_view_revision(
    State(state): State<SharedState>,
    State(_auth): State<AuthServeState>,
    headers: HeaderMap,
    Query(query): Query<ViewRevisionQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app_id is required"})),
        )
            .into_response();
    }
    let scene_id = query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home")
        .to_string();
    {
        let mut guard = state.write().expect("state lock");
        crate::build_ops::refresh_materialization_flags(&mut guard);
        if !guard.imported {
            return Json(json!({
                "ready": false,
                "startup_phase": guard.startup_phase,
                "startup_detail": guard.startup_detail,
                "app_id": app_id,
                "scene_id": scene_id,
            }))
            .into_response();
        }
    }
    let route_mode = resolve_route_mode_from_surface(query.surface.as_deref());
    let scene_id = if route_mode.is_build() {
        query
            .node
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(crate::build_fragment_cache::scene_id_from_build_node)
            .unwrap_or(scene_id)
    } else {
        scene_id
    };
    let (workspace_root, axes) = {
        let guard = state.read().expect("state lock");
        let axes = resolve_page_render_axes(
            &guard,
            &AppQuery {
                data_mode: query.data_mode.clone(),
                review_projection: query.review_projection.clone(),
                ..Default::default()
            },
            route_mode,
        );
        (guard.ctx.workspace_root.clone(), axes)
    };
    let workspace_root = workspace_root.as_path();

    let draft_session = mei_host_core::resolve_draft_session_id(&headers);
    let draft = if route_mode.is_build() {
        crate::build_layout_tuning::build_session_layout_tuning_draft(
            workspace_root,
            app_id,
            mei_host_core::layout_tuning_draft_storage_key(app_id, draft_session.as_str()).as_str(),
        )
    } else {
        None
    };
    let draft_digest = draft
        .as_ref()
        .map(|value| crate::build_fragment_cache::draft_digest_for_tuning(Some(value)))
        .unwrap_or_default();

    let fallback_compose = compose_from_query(&query, route_mode);
    let mut compose = parse_compose_request(query.compose.as_deref(), &fallback_compose);
    if compose.data_mode.is_none() {
        compose.data_mode = Some(axes.data_mode.slug().to_string());
    }
    if compose.review_projection.is_none() {
        compose.review_projection = Some(
            crate::review_axes::ssr_review_projection(route_mode, axes.data_mode)
                .slug()
                .to_string(),
        );
    }

    let client_layers = parse_client_layers(query.client_layers.as_deref());
    let local_miss = parse_bool_flag(query.local_miss.as_deref());
    let missing_layers = parse_missing_layers(query.missing_layers.as_deref());

    let mut hits = ArtifactHitMatrix::default();
    let revision = match resolve_view_revision_for_surface(
        workspace_root,
        app_id,
        scene_id.as_str(),
        route_mode,
        axes.data_mode,
        &compose,
        draft_session.as_str(),
        draft_digest.as_str(),
        client_layers,
        local_miss,
        missing_layers,
        &mut hits,
    ) {
        Ok(value) => value,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": err.to_string()})),
            )
                .into_response();
        }
    };

    let obs = LayerArtifactObservability { hits };
    let mut response = Json(&revision).into_response();
    apply_view_revision_headers(&mut response, &revision);
    if local_miss {
        if let Ok(value) = HeaderValue::from_str("1") {
            response
                .headers_mut()
                .insert(HeaderName::from_static("x-mei-local-miss"), value);
        }
    }
    for (name, value) in obs.response_headers() {
        if let Ok(header_value) = HeaderValue::from_str(value.as_str()) {
            response
                .headers_mut()
                .insert(HeaderName::from_static(name), header_value);
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_missing_layers_accepts_csv_and_json() {
        assert_eq!(
            parse_missing_layers(Some("structure.full,theme.tokens")),
            vec!["structure.full".to_string(), "theme.tokens".to_string()]
        );
        assert_eq!(
            parse_missing_layers(Some(r#"["layout.overlay"]"#)),
            vec!["layout.overlay".to_string()]
        );
    }
}
