use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Json, Redirect, Response},
};
use mei_host_auth::{
    account_view_for_principal, filter_apps_for_principal, v2_index_landing_location,
    AuthEnforcement, AuthPrincipal, AuthServeState,
};
use mei_lang_app::{load_topbar_menu_context, page_body_theme_style, render_page, UiRouteMode};
use mei_lang_kernel::load_workspace_config;
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;

use crate::build_info::fill_page_shell_placeholders;
use crate::landing::{choose_default_app, discover_workspace_apps, enrich_discovered_apps};
use crate::access_page_cache::{
    access_page_cache_key, insert_page_render_cache_hit_header, render_access_page_template,
    store_access_page_template, take_access_page_template,
};
use crate::page_observability::{
    fill_manage_wall_clock_placeholders, fill_page_load_observability_placeholders,
    measure_page_html_payload,
};
use crate::state::SharedState;

#[derive(Debug, Deserialize, Default)]
pub struct AppQuery {
    pub tab: Option<String>,
    pub scene: Option<String>,
    pub node: Option<String>,
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
    if app_id.is_empty() {
        return (StatusCode::NOT_FOUND, "app not found").into_response();
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let discovered = match discover_workspace_apps(workspace_root) {
        Ok(apps) => apps,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("discover apps failed: {error}"),
            )
                .into_response();
        }
    };
    let apps = filter_apps_for_principal(
        discovered.as_slice(),
        principal.as_ref().map(|Extension(p)| p),
    );
    if !apps.iter().any(|app| app.id == app_id) {
        return (StatusCode::NOT_FOUND, "app not found").into_response();
    }
    if !route_mode.is_access_like()
        && route_mode != UiRouteMode::Runtime
        && !route_mode.is_build()
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            format!("route mode `{}` not supported in mei-host-shell yet", mode),
        )
            .into_response();
    }
    let scene_id = scene_id.unwrap_or_else(|| "home".to_string());
    let request_started = Instant::now();
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let apps = enrich_discovered_apps(apps.as_slice(), &topbar_menu);
    let app_ctx = guard.host_ctx_for_app(app_id.as_str());
    let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
        app_ctx.app_root().as_path(),
        Some(workspace_root),
        None,
    );
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal.as_ref().map(|Extension(p)| p));
    let cache_key = access_page_cache_key(
        workspace_root,
        app_id.as_str(),
        scene_id.as_str(),
        route_mode,
        auth_enabled,
        account_view.as_ref(),
        &gis,
    );
    let mut page_render_cache_hit = false;
    let (mut html, ssr_emit_ms) = if route_mode.is_access_like() {
        if let Some(ref key) = cache_key {
            if let Some(cached) = take_access_page_template(
                workspace_root,
                app_id.as_str(),
                scene_id.as_str(),
                key.as_str(),
            ) {
                page_render_cache_hit = true;
                (cached, 0)
            } else {
                let render_started = Instant::now();
                match render_access_page_template(
                    workspace_root,
                    apps.as_slice(),
                    &topbar_menu,
                    app_id.as_str(),
                    scene_id.as_str(),
                    route_mode,
                    &query,
                    auth_enabled,
                    account_view.as_ref(),
                ) {
                    Ok(template) => {
                        let ssr_emit_ms = render_started.elapsed().as_millis() as u64;
                        let _ = store_access_page_template(
                            workspace_root,
                            app_id.as_str(),
                            scene_id.as_str(),
                            key.as_str(),
                            template.as_str(),
                        );
                        (template, ssr_emit_ms)
                    }
                    Err(error) => {
                        tracing::warn!(
                            app_id = %app_id,
                            scene_id = %scene_id,
                            error = %error,
                            "access page render failed"
                        );
                        return (
                            StatusCode::NOT_FOUND,
                            format!("scene not assembled: {error}"),
                        )
                            .into_response();
                    }
                }
            }
        } else {
            let render_started = Instant::now();
            match render_access_page_template(
                workspace_root,
                apps.as_slice(),
                &topbar_menu,
                app_id.as_str(),
                scene_id.as_str(),
                route_mode,
                &query,
                auth_enabled,
                account_view.as_ref(),
            ) {
                Ok(template) => (template, render_started.elapsed().as_millis() as u64),
                Err(error) => {
                    tracing::warn!(
                        app_id = %app_id,
                        scene_id = %scene_id,
                        error = %error,
                        "access page render failed"
                    );
                    return (
                        StatusCode::NOT_FOUND,
                        format!("scene not assembled: {error}"),
                    )
                        .into_response();
                }
            }
        }
    } else {
        let assemble_result = mei_host_graph::assemble_scope_from_registry(
            workspace_root,
            app_id.as_str(),
            scene_id.as_str(),
        );
        let outcome = match assemble_result {
            Ok(Some(outcome)) => outcome,
            Ok(None) => {
                tracing::warn!(app_id = %app_id, scene_id = %scene_id, "assemble returned None (empty registry or missing scene)");
                return (
                    StatusCode::NOT_FOUND,
                    format!(
                        "scene not assembled for app `{app_id}`; run prebuild for this app"
                    ),
                )
                    .into_response();
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
        let workspace = load_workspace_config(workspace_root);
        let theme_style = page_body_theme_style(&workspace, Some(&outcome.compiled), None);
        let runtime_snapshot_json_owned;
        let runtime_roots_owned;
        if route_mode == UiRouteMode::Runtime {
            let snapshot = crate::runtime_snapshot::build_runtime_snapshot(&guard, app_id.as_str());
            runtime_snapshot_json_owned =
                serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string());
            runtime_roots_owned = crate::runtime_snapshot::management_roots_from_snapshot(&snapshot);
        } else {
            runtime_snapshot_json_owned = String::new();
            runtime_roots_owned = Vec::new();
        }
        let runtime_roots_ref = if route_mode == UiRouteMode::Runtime {
            Some(runtime_roots_owned.as_slice())
        } else {
            None
        };
        let runtime_json_ref = if route_mode == UiRouteMode::Runtime {
            Some(runtime_snapshot_json_owned.as_str())
        } else {
            None
        };
        let render_started = Instant::now();
        let html = crate::gis_config::fill_gis_tiles_placeholders(
            inject_layer_plane_scripts(
                inject_client_bootstrap_script(
                    fill_page_shell_placeholders(
                        render_page(
                            apps.as_slice(),
                            &outcome.compiled,
                            app_id.as_str(),
                            Some(&topbar_menu),
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
                            query.node.as_deref(),
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
                            runtime_roots_ref,
                            runtime_json_ref,
                        ),
                        workspace_root,
                    ),
                    workspace_root,
                    app_id.as_str(),
                    scene_id.as_str(),
                ),
                &outcome,
            ),
            &gis,
        );
        (html, render_started.elapsed().as_millis() as u64)
    };
    let handler_html_ready_ms = request_started.elapsed().as_millis() as u64;
    html = fill_manage_wall_clock_placeholders(html, ssr_emit_ms, handler_html_ready_ms);
    let payload_stats = measure_page_html_payload(html.as_str());
    html = fill_page_load_observability_placeholders(
        html,
        ssr_emit_ms,
        page_render_cache_hit,
        payload_stats.html_bytes,
        payload_stats.data_props_bytes,
        payload_stats.data_props_count,
    );
    let _ = mei_host_graph::flush_telemetry_to_registry(workspace_root, app_id.as_str());
    let mut response = Html(html).into_response();
    if let Ok(value) = HeaderValue::from_str(&handler_html_ready_ms.to_string()) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-handler-html-ready-ms"),
            value,
        );
    }
    if let Ok(value) = HeaderValue::from_str(&ssr_emit_ms.to_string()) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-ssr-http-response-body-ms"),
            value,
        );
    }
    if let Ok(value) = HeaderValue::from_str(&payload_stats.data_props_bytes.to_string()) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-data-props-bytes"), value);
    }
    if let Ok(value) = HeaderValue::from_str(&payload_stats.data_props_count.to_string()) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-mei-data-props-count"), value);
    }
    insert_page_render_cache_hit_header(&mut response, page_render_cache_hit);
    response
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
    let workspace_root = guard.ctx.workspace_root.as_path();
    let discovered = discover_workspace_apps(workspace_root).unwrap_or_default();
    let apps = filter_apps_for_principal(
        discovered.as_slice(),
        principal.as_ref().map(|Extension(p)| p),
    );
    let app = choose_default_app(workspace_root, apps.as_slice()).or_else(|| apps.first());
    let Some(app) = app else {
        return (
            StatusCode::NOT_FOUND,
            "no discoverable app with prebuilt access entry; run prebuild",
        )
            .into_response();
    };
    let location = v2_index_landing_location(
        workspace_root,
        app,
        principal.as_ref().map(|Extension(p)| p),
    );
    Redirect::temporary(&location).into_response()
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

pub(crate) fn inject_layer_plane_scripts(html: String, outcome: &mei_host_graph::AssembleOutcome) -> String {
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

pub(crate) fn inject_client_bootstrap_script(
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
        let status = mei_host_graph::bootstrap_embed_status(workspace_root, app_id, scene_id);
        if status.allowed {
            tracing::info!(
                app_id = %app_id,
                scope = %scene_id,
                reason = %status.reason,
                metric_count = status.metric_count,
                "client bootstrap SSR inject skipped despite allowed status"
            );
        } else {
            tracing::info!(
                app_id = %app_id,
                scope = %scene_id,
                reason = %status.reason,
                metric_count = status.metric_count,
                client_revision = ?status.client_revision,
                expected_revision = ?status.expected_revision,
                "client bootstrap SSR inject rejected"
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
