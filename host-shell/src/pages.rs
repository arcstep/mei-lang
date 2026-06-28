use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Redirect, Response},
};
use mei_host_auth::{
    account_view_for_principal, v2_index_landing_location, AuthEnforcement, AuthPrincipal,
    AuthServeState,
};
use mei_lang_app::{page_body_theme_style, render_page, UiRouteMode};
use mei_lang_kernel::{load_workspace_config, WorkspaceAppMeta};
use serde::Deserialize;
use serde_json::json;

use crate::build_info::fill_host_build_placeholders;
use crate::state::SharedState;

#[derive(Debug, Deserialize, Default)]
pub struct AppQuery {
    pub tab: Option<String>,
    pub scene: Option<String>,
}

pub async fn app_page(
    State(state): State<SharedState>,
    State(auth): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    Path((mode, app_tail)): Path<(String, String)>,
    Query(query): Query<AppQuery>,
) -> Response {
    let route_mode = UiRouteMode::from_slug(mode.as_str());
    let app_tail = app_tail.trim_start_matches('/').to_string();
    let (app_id, scene_id) = parse_app_scene_path(&app_tail, query.scene.as_deref());
    let guard = state.read().expect("state lock");
    if guard.ctx.app_id != app_id {
        return (StatusCode::NOT_FOUND, "app not found").into_response();
    }
    if !route_mode.is_access_like() && route_mode != UiRouteMode::Runtime {
        return (
            StatusCode::NOT_IMPLEMENTED,
            format!("route mode `{}` not supported in mei-host-shell yet", mode),
        )
            .into_response();
    }
    let scene_id = scene_id.unwrap_or_else(|| "home".to_string());
    let assemble_result = mei_host_graph::assemble_scope_from_registry(
        guard.ctx.workspace_root.as_path(),
        app_id.as_str(),
        scene_id.as_str(),
    );
    let outcome = match assemble_result {
        Ok(Some(outcome)) => outcome,
        Ok(None) => {
            tracing::warn!(app_id = %app_id, scene_id = %scene_id, "assemble returned None (empty registry or missing scene)");
            return (StatusCode::NOT_FOUND, "scene not assembled").into_response();
        }
        Err(error) => {
            tracing::warn!(
                app_id = %app_id,
                scene_id = %scene_id,
                error = %error,
                "assemble failed"
            );
            return (
                StatusCode::NOT_FOUND,
                format!("scene not assembled: {error}"),
            )
                .into_response();
        }
    };
    let workspace = load_workspace_config(guard.ctx.workspace_root.as_path());
    let theme_style = page_body_theme_style(&workspace, Some(&outcome.compiled), None);
    let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
        guard.ctx.app_root().as_path(),
        Some(guard.ctx.workspace_root.as_path()),
        None,
    );
    let apps = vec![WorkspaceAppMeta {
        id: app_id.clone(),
        title: outcome.compiled.title.clone(),
        root: guard.ctx.app_root().display().to_string(),
    }];
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal.as_ref().map(|Extension(p)| p));
    let html = crate::gis_config::fill_gis_tiles_placeholders(
        inject_layer_plane_scripts(
            inject_client_bootstrap_script(
                fill_host_build_placeholders(
                    render_page(
                &apps,
                &outcome.compiled,
                app_id.as_str(),
                None,
                route_mode,
                Some(outcome.compiled.active_target_file.as_str()),
                None,
                None,
                Some(scene_id.as_str()),
                None,
                query.tab.as_deref(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                None,
                &[],
                auth_enabled,
                account_view.as_ref(),
                None,
                theme_style.as_str(),
                None,
                None,
                    ),
                    guard.ctx.workspace_root.as_path(),
                ),
                guard.ctx.workspace_root.as_path(),
                app_id.as_str(),
                scene_id.as_str(),
            ),
            &outcome,
        ),
        &gis,
    );
    Html(html).into_response()
}

#[derive(Debug, Deserialize, Default)]
pub struct PresentationMapQuery {
    pub scene: Option<String>,
}

pub async fn api_presentation_map(
    State(state): State<SharedState>,
    Path(app_id): Path<String>,
    Query(query): Query<PresentationMapQuery>,
) -> Response {
    let guard = state.read().expect("state lock");
    if guard.ctx.app_id != app_id {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "app mismatch"}))).into_response();
    }
    let scene_id = query.scene.unwrap_or_else(|| "home".to_string());
    let outcome = match mei_host_graph::assemble_scope_from_registry(
        guard.ctx.workspace_root.as_path(),
        app_id.as_str(),
        scene_id.as_str(),
    ) {
        Ok(Some(outcome)) => outcome,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "scene not assembled"})),
            )
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("assemble failed: {error}")})),
            )
                .into_response();
        }
    };
    Json(outcome.presentation_map).into_response()
}

pub async fn index(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    let app = WorkspaceAppMeta {
        id: guard.ctx.app_id.clone(),
        title: guard.ctx.app_id.clone(),
        root: guard.ctx.app_root().display().to_string(),
    };
    let location = v2_index_landing_location(
        guard.ctx.workspace_root.as_path(),
        &app,
        principal.as_ref().map(|Extension(p)| p),
    );
    Redirect::temporary(&location)
}

fn parse_app_scene_path(app_tail: &str, scene_query: Option<&str>) -> (String, Option<String>) {
    let parts: Vec<&str> = app_tail.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return (String::new(), scene_query.map(str::to_string));
    }
    let app_id = parts[0].to_string();
    let scene = if parts.len() >= 3 && parts[1] == "scene" {
        Some(parts[2].to_string())
    } else {
        scene_query.map(str::to_string)
    };
    (app_id, scene)
}

fn inject_layer_plane_scripts(html: String, outcome: &mei_host_graph::AssembleOutcome) -> String {
    let layer_plan =
        serde_json::to_string(&outcome.layer_plan).unwrap_or_else(|_| "{}".to_string());
    let presentation_map =
        serde_json::to_string(&outcome.presentation_map).unwrap_or_else(|_| "{}".to_string());
    let overlay_defaults = serde_json::to_string(&outcome.overlay_defaults)
        .unwrap_or_else(|_| "{}".to_string());
    let scripts = format!(
        r#"<script type="application/json" id="mei-layer-plan">{layer_plan}</script><script type="application/json" id="mei-presentation-map">{presentation_map}</script><script>window.__mei=window.__mei||{{}};window.__mei.overlay_defaults={overlay_defaults};</script>"#
    );
    if let Some(pos) = html.find("</head>") {
        let mut out = String::with_capacity(html.len() + scripts.len());
        out.push_str(&html[..pos]);
        out.push_str(&scripts);
        out.push_str(&html[pos..]);
        out
    } else {
        format!("{scripts}{html}")
    }
}

fn inject_client_bootstrap_script(
    html: String,
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
) -> String {
    let Some(fragment) = mei_host_graph::build_client_bootstrap_head_fragment(
        workspace_root,
        app_id,
        scene_id,
    ) else {
        if mei_host_graph::read_client_bootstrap(workspace_root, app_id, scene_id).is_some() {
            tracing::debug!(
                app_id = %app_id,
                scope = %scene_id,
                "client bootstrap embed rejected by MRG revision gate"
            );
        } else {
            tracing::debug!(
                app_id = %app_id,
                scope = %scene_id,
                "client bootstrap manifest missing for SSR inject"
            );
        }
        return html;
    };
    if let Some(pos) = html.find("</head>") {
        let mut out = String::with_capacity(html.len() + fragment.len());
        out.push_str(&html[..pos]);
        out.push_str(&fragment);
        out.push_str(&html[pos..]);
        out
    } else {
        format!("{fragment}{html}")
    }
}
