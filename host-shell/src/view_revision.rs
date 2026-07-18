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
use std::time::Instant;

use crate::artifact_observability::{ArtifactHitMatrix, LayerArtifactObservability};
use crate::pages::AppQuery;
use crate::review_axes::resolve_page_render_axes_for_stage;
use crate::scene_manifest::{
    resolve_route_mode_from_surface, resolve_view_revision_for_surface, SceneChromeHostContext,
};
use crate::state::SharedState;

#[derive(Debug, Deserialize, Default)]
pub struct ViewRevisionQuery {
    #[serde(default, alias = "app")]
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

fn blank_option(value: &mut Option<String>) {
    if value
        .as_ref()
        .map(|part| part.trim().is_empty())
        .unwrap_or(false)
    {
        *value = None;
    }
}

fn normalize_compose_request(compose: &mut mei_host_graph::ComposeRequest) {
    blank_option(&mut compose.route_mode);
    blank_option(&mut compose.tab);
    blank_option(&mut compose.chrome);
    blank_option(&mut compose.review_projection);
    blank_option(&mut compose.data_mode);
    blank_option(&mut compose.focus);
    blank_option(&mut compose.scope);
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

fn compose_from_query(
    query: &ViewRevisionQuery,
    route_mode: UiRouteMode,
) -> mei_host_graph::ComposeRequest {
    mei_host_graph::ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: query.tab.clone(),
        chrome: query.chrome.clone(),
        review_projection: query.review_projection.clone(),
        data_mode: query.data_mode.clone(),
        focus: query.focus.clone(),
        scope: query.scope.clone(),
        scope_target: None,
    }
}

fn apply_view_revision_headers(
    response: &mut Response,
    revision: &mei_host_graph::ViewRevisionResponse,
) {
    let status = match revision.status {
        mei_host_graph::ViewRevisionStatus::Refetch => "refetch",
        mei_host_graph::ViewRevisionStatus::AssembleLocal => "assemble_local",
    };
    if let Ok(value) = HeaderValue::from_str(status) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-view-revision-status"), value);
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
    let request_started = Instant::now();
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app_id is required"})),
        )
            .into_response();
    }
    let scene_id = match query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(scene) => scene.to_string(),
        None => {
            let workspace = {
                let guard = state.read().expect("state lock");
                guard.ctx.workspace_root.clone()
            };
            crate::shell_chrome::default_access_scene(workspace.as_path(), app_id)
        }
    };
    let gate_started = Instant::now();
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
    let gate_ms = gate_started.elapsed().as_millis();
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
        let stage_kind = match mei_host_graph::assemble_scope_from_registry(
            guard.ctx.workspace_root.as_path(),
            app_id,
            scene_id.as_str(),
        ) {
            Ok(Some(outcome)) => crate::review_axes::StageKind::resolve(
                &outcome.compiled.stage_registry,
                &outcome.compiled.scene_routes,
                scene_id.as_str(),
            ),
            _ => crate::review_axes::StageKind::Scene,
        };
        let axes = resolve_page_render_axes_for_stage(
            &guard,
            &AppQuery {
                data_mode: query.data_mode.clone(),
                review_projection: query.review_projection.clone(),
                ..Default::default()
            },
            route_mode,
            stage_kind,
        );
        (guard.ctx.workspace_root.clone(), axes)
    };
    let workspace_root = workspace_root.as_path();

    // Compose axes drive scoped temporary-Stage revision (MCG closure via preview_scope).
    let fallback_compose = compose_from_query(&query, route_mode);
    let mut compose = parse_compose_request(query.compose.as_deref(), &fallback_compose);
    normalize_compose_request(&mut compose);
    if compose.data_mode.is_none() {
        compose.data_mode = Some(axes.data_mode.slug().to_string());
    }
    if compose.review_projection.is_none() {
        compose.review_projection = Some(axes.review_projection.slug().to_string());
    }
    let preview_scope = compose
        .scope_target
        .as_ref()
        .map(|t| t.preview_scope.clone())
        .or_else(|| compose.scope.clone())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

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

    let discovery_started = Instant::now();
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let (apps, admin_nav) = {
        let guard = state.read().expect("state lock");
        let apps = crate::shell_chrome::apps_for_topbar(&guard);
        let admin_nav = crate::admin_nav::admin_nav_items_for_app(
            &guard.admin_registry,
            workspace_root,
            app_id,
            None,
        );
        (apps, admin_nav)
    };
    let chrome_host = SceneChromeHostContext {
        apps: apps.as_slice(),
        topbar_menu: Some(&topbar_menu),
        auth_enabled: false,
        auth_account: None,
        admin_nav_items: admin_nav.as_slice(),
    };
    let discovery_ms = discovery_started.elapsed().as_millis();

    let mut hits = ArtifactHitMatrix::default();
    let revision_started = Instant::now();
    let revision = match resolve_view_revision_for_surface(
        workspace_root,
        app_id,
        scene_id.as_str(),
        route_mode,
        axes.data_mode,
        preview_scope.as_deref(),
        client_manifest_digest,
        client_surface_digest,
        recover,
        local_miss,
        &mut hits,
        Some(&chrome_host),
    ) {
        Ok(value) => value,
        Err(err) => {
            let message = err.to_string();
            let status = if message.contains("assembly view not found")
                || message.contains("scene not found")
            {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            return (status, Json(json!({"error": message}))).into_response();
        }
    };
    let revision_ms = revision_started.elapsed().as_millis();

    let obs = LayerArtifactObservability { hits };
    let serialize_started = Instant::now();
    let mut response = Json(&revision).into_response();
    let serialize_ms = serialize_started.elapsed().as_millis();
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
    let server_timing = format!(
        "gate;dur={gate_ms}, app_discovery;dur={discovery_ms}, revision;dur={revision_ms}, serialize;dur={serialize_ms}, handler;dur={}",
        request_started.elapsed().as_millis()
    );
    if let Ok(value) = HeaderValue::from_str(server_timing.as_str()) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("server-timing"), value);
    }
    response
}
