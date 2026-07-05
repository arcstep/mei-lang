//! Unified view-revision API: revision-first layer negotiation.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use mei_host_auth::AuthServeState;
use mei_lang_app::{load_topbar_menu_context, UiRouteMode};
use serde::Deserialize;
use serde_json::json;

use crate::artifact_observability::{ArtifactHitMatrix, LayerArtifactObservability};
use crate::landing::discover_workspace_apps;
use crate::pages::AppQuery;
use crate::review_axes::resolve_page_render_axes;
use crate::scene_manifest::{
    ensure_manifest_index, manifest_for_surface, materialize_layers_for_request,
    resolve_route_mode_from_surface, SceneChromeHostContext,
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
    pub manifest_revision_digest: Option<String>,
    #[serde(default)]
    pub surface_revision_digest: Option<String>,
    #[serde(default)]
    pub recover: Option<String>,
    /// Deprecated: treated as `recover`.
    #[serde(default)]
    pub local_miss: Option<String>,
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
    client_manifest_digest: Option<String>,
    client_surface_digest: Option<String>,
    recover: bool,
    local_miss: bool,
    hits: &mut ArtifactHitMatrix,
    chrome_host: Option<&SceneChromeHostContext<'_>>,
) -> anyhow::Result<mei_host_graph::ViewRevisionResponse> {
    let index = ensure_manifest_index(
        workspace_root,
        app_id,
        scene_id,
        data_mode,
        hits,
        chrome_host,
    )?;
    let manifest = manifest_for_surface(&index, route_mode).ok_or_else(|| {
        anyhow::anyhow!("manifest index missing surface {}", route_mode.slug())
    })?;
    let surface_digest = surface_revision_digest(&manifest);
    let mut response = mei_host_graph::resolve_view_revision(&mei_host_graph::ViewRevisionInput {
        manifest: manifest.clone(),
        client_manifest_digest,
        client_surface_digest,
        recover,
        local_miss,
        client_layers: Vec::new(),
        missing_layers: Vec::new(),
        surface_revision_digest: surface_digest,
    });
    if response.status == mei_host_graph::ViewRevisionStatus::Refetch
        && !response.changed_layers.is_empty()
        && response.changed_layers.len() <= 24
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
            chrome_host,
        )?;
        response.inline_layers = Some(inline);
        if response.manifest.is_none() {
            response.manifest = Some(manifest);
        }
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
    _headers: HeaderMap,
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

    let fallback_compose = compose_from_query(&query, route_mode);
    let mut compose = parse_compose_request(query.compose.as_deref(), &fallback_compose);
    if compose.data_mode.is_none() {
        compose.data_mode = Some(axes.data_mode.slug().to_string());
    }
    if compose.review_projection.is_none() {
        compose.review_projection = Some(axes.review_projection.slug().to_string());
    }

    let client_manifest_digest = query
        .manifest_revision_digest
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let client_surface_digest = query
        .surface_revision_digest
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let recover = parse_bool_flag(query.recover.as_deref());
    let local_miss = parse_bool_flag(query.local_miss.as_deref());

    let topbar_menu = load_topbar_menu_context(workspace_root);
    let discovered = discover_workspace_apps(workspace_root).unwrap_or_default();
    let apps = crate::landing::enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
    let chrome_host = SceneChromeHostContext {
        apps: apps.as_slice(),
        topbar_menu: Some(&topbar_menu),
        auth_enabled: false,
        auth_account: None,
    };

    let mut hits = ArtifactHitMatrix::default();
    let revision = match resolve_view_revision_for_surface(
        workspace_root,
        app_id,
        scene_id.as_str(),
        route_mode,
        axes.data_mode,
        &compose,
        "",
        "",
        client_manifest_digest,
        client_surface_digest,
        recover,
        local_miss,
        &mut hits,
        Some(&chrome_host),
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
    if recover || local_miss {
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
