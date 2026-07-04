use axum::{
    extract::{Extension, OriginalUri, Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
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
    access_page_cache_key, build_scene_revision_payload, insert_page_render_cache_hit_header,
    render_access_page_template, resolve_access_page_html, store_access_page_template,
    take_access_page_template,
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
    pub file: Option<String>,
    pub scope: Option<String>,
    pub focus: Option<String>,
    pub chrome: Option<String>,
    /// Requested data mode (`eval` | `fixture` | `static`), clamped to serve ceiling.
    pub data_mode: Option<String>,
    /// Build / review projection depth (`plane`, `plane_region`, …).
    pub review_projection: Option<String>,
}

pub async fn app_page(
    State(state): State<SharedState>,
    State(auth): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path((mode, app_tail)): Path<(String, String)>,
    Query(query): Query<AppQuery>,
) -> Response {
    if mode == "speaker" {
        let tail = app_tail.trim_start_matches('/');
        let location = format!(
            "/apps/copilot/{}",
            tail.replace("/tour/", "/presentation/")
        );
        return Redirect::temporary(location.as_str()).into_response();
    }
    let route_mode = UiRouteMode::from_slug(mode.as_str());
    let app_tail = app_tail.trim_start_matches('/').to_string();
    let (app_id, scene_id, tour_id) = parse_app_scene_path(&app_tail, query.scene.as_deref(), route_mode);
    if app_id.is_empty() {
        return (StatusCode::NOT_FOUND, "app not found").into_response();
    }
    let copilot_presentation_id = tour_id.as_deref();
    if route_mode.is_access_like() {
        let starting_location = {
            let workspace_root = {
                let guard = state.read().expect("state lock");
                guard.ctx.workspace_root.clone()
            };
            if let Err(error) = crate::startup::try_ensure_app_registry_materialized(
                workspace_root.as_path(),
                app_id.as_str(),
            ) {
                tracing::warn!(
                    app_id = %app_id,
                    error = %error,
                    "lazy app import before access gate failed"
                );
            }
            let mut guard = state.write().expect("state lock");
            crate::build_ops::refresh_materialization_flags(&mut guard);
        let axes =
                crate::review_axes::resolve_page_render_axes(&guard, &query, route_mode);
            let readiness = crate::startup::evaluate_access_readiness(
                &guard,
                app_id.as_str(),
                scene_id.as_str(),
                route_mode,
                axes,
            );
            if readiness.ready {
                None
            } else {
                Some(crate::startup::build_starting_location(
                    &uri,
                    app_id.as_str(),
                    scene_id.as_str(),
                    mode.as_str(),
                ))
            }
        };
        if let Some(location) = starting_location {
            return Redirect::temporary(location.as_str()).into_response();
        }
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let package_root = guard.package_root.as_path();
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
        && route_mode != UiRouteMode::Config
        && route_mode != UiRouteMode::Upload
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            format!("route mode `{}` not supported in mei-host-shell yet", mode),
        )
            .into_response();
    }
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
    if route_mode == UiRouteMode::Config || route_mode == UiRouteMode::Upload {
        let app_title = apps
            .iter()
            .find(|app| app.id == app_id)
            .map(|app| app.title.as_str())
            .unwrap_or(app_id.as_str());
        let scene_for_links = query
            .scene
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(response) = crate::light_pages::try_render_light_page(
            crate::light_pages::LightPageContext {
                workspace_root,
                _package_root: package_root,
                route_mode,
                app_id: app_id.as_str(),
                apps: apps.as_slice(),
                app_title,
                topbar_menu: &topbar_menu,
                lightweight_scene: scene_for_links,
                request_file: query.file.as_deref(),
                auth_enabled,
                account_view: account_view.as_ref(),
            },
        ) {
            return response;
        }
        return (
            StatusCode::NOT_FOUND,
            format!("route mode `{}` is not available for this app", mode),
        )
            .into_response();
    }
    let axes_resolution =
        crate::review_axes::resolve_page_render_axes_detailed(&guard, &query, route_mode);
    let axes = axes_resolution.axes;
    let chrome_hidden = query
        .chrome
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("none"))
        .unwrap_or(route_mode == UiRouteMode::Run || route_mode == UiRouteMode::Copilot);
    let data_mode_ceiling_notice_owned = if axes_resolution.data_mode_clamped {
        Some(format!(
            "当前部署 data_mode ceiling 为 `{}`，已将请求降档为 `{}`。",
            guard.data_mode_ceiling.slug(),
            axes.data_mode.slug()
        ))
    } else {
        None
    };
    let cache_key = access_page_cache_key(
        workspace_root,
        app_id.as_str(),
        scene_id.as_str(),
        route_mode,
        axes,
        chrome_hidden,
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
                    package_root,
                    apps.as_slice(),
                    &topbar_menu,
                    app_id.as_str(),
                    scene_id.as_str(),
                    route_mode,
                    &query,
                    axes,
                    auth_enabled,
                    account_view.as_ref(),
                    copilot_presentation_id,
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
                package_root,
                apps.as_slice(),
                &topbar_menu,
                app_id.as_str(),
                scene_id.as_str(),
                route_mode,
                &query,
                axes,
                auth_enabled,
                account_view.as_ref(),
                copilot_presentation_id,
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
        let mut outcome = match assemble_result {
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
        if route_mode.is_build() {
            crate::build_layout_tuning::apply_build_session_layout_tuning_draft(
                &mut outcome.compiled,
                workspace_root,
                app_id.as_str(),
                &headers,
            );
        }
        let workspace = load_workspace_config(workspace_root);
        let target_file = query
            .file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(outcome.compiled.active_target_file.as_str());
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
                            Some(target_file),
                            None,
                            None,
                            Some(scene_id.as_str()),
                            None,
                            query.tab.as_deref(),
                            None,
                            None,
                            None,
                            None,
                            query.node.as_deref(),
                            query.scope.as_deref(),
                            query.focus.as_deref(),
                            None,
                            None,
                            chrome_hidden,
                            false,
                            None,
                            &[],
                            auth_enabled,
                            account_view.as_ref(),
                            None,
                            theme_style.as_str(),
                            runtime_roots_ref,
                            runtime_json_ref,
                            Some(axes.data_mode.slug()),
                            Some(axes.review_projection.slug()),
                            data_mode_ceiling_notice_owned.as_deref(),
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
    if route_mode == UiRouteMode::Runtime {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-cache, must-revalidate"),
        );
    }
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

#[derive(Debug, Deserialize, Default)]
pub struct HostStartingQuery {
    #[serde(default, rename = "return")]
    pub return_path: String,
    pub app: Option<String>,
    pub scene: Option<String>,
    pub mode: Option<String>,
}

pub async fn host_starting_page(
    State(state): State<SharedState>,
    Query(query): Query<HostStartingQuery>,
) -> Response {
    let (workspace, default_app, phase, detail, error) = {
        let mut guard = state.write().expect("state lock");
        crate::build_ops::refresh_materialization_flags(&mut guard);
        (
            guard.ctx.workspace_root.clone(),
            guard.ctx.app_id.clone(),
            guard.startup_phase.clone(),
            guard.startup_detail.clone(),
            guard.startup_error.clone(),
        )
    };
    if let Some(message) = error {
        return mei_host_auth::startup_failed_html_response(workspace.as_path(), message.as_str());
    }
    let return_path = {
        let raw = query.return_path.trim();
        crate::startup::sanitize_return_path(if raw.is_empty() { "/" } else { raw })
    };
    let (poll_app, poll_scene, poll_mode) = if query.app.as_deref().is_some_and(|value| !value.trim().is_empty()) {
        (
            query.app.unwrap_or(default_app.clone()),
            query
                .scene
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "home".to_string()),
            query
                .mode
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "app".to_string()),
        )
    } else {
        let (app, scene, mode) =
            crate::startup::parse_warm_poll_from_path(return_path.as_str(), default_app.as_str());
        (app, scene, mode)
    };
    let route_mode = UiRouteMode::from_slug(poll_mode.as_str());
    let already_ready = {
        if let Err(error) = crate::startup::try_ensure_app_registry_materialized(
            workspace.as_path(),
            poll_app.as_str(),
        ) {
            tracing::warn!(
                app_id = %poll_app,
                error = %error,
                "lazy app import on starting page failed"
            );
        }
        let mut guard = state.write().expect("state lock");
        crate::build_ops::refresh_materialization_flags(&mut guard);
        let axes = crate::review_axes::resolve_page_render_axes(
            &guard,
            &AppQuery::default(),
            route_mode,
        );
        crate::startup::evaluate_access_readiness(
            &guard,
            poll_app.as_str(),
            poll_scene.as_str(),
            route_mode,
            axes,
        )
        .ready
    };
    if already_ready {
        tracing::info!(
            app_id = %poll_app,
            scene_id = %poll_scene,
            return_path = %return_path,
            "access already ready — redirecting from starting page"
        );
        return Redirect::temporary(return_path.as_str()).into_response();
    }
    mei_host_auth::host_starting_html_response(
        workspace.as_path(),
        detail.as_deref().unwrap_or(phase.as_str()),
        return_path.as_str(),
        poll_app.as_str(),
        poll_scene.as_str(),
        poll_mode.as_str(),
    )
}

#[derive(Debug, Deserialize, Default)]
pub struct AccessReadinessQuery {
    pub app: String,
    pub scene: Option<String>,
    pub mode: Option<String>,
    pub data_mode: Option<String>,
}

pub async fn api_host_access_readiness(
    State(state): State<SharedState>,
    Query(query): Query<AccessReadinessQuery>,
) -> Response {
    let app_id = query.app.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app is required"})),
        )
            .into_response();
    }
    let scene_id = query
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home");
    let route_mode = query
        .mode
        .as_deref()
        .map(UiRouteMode::from_slug)
        .unwrap_or(UiRouteMode::App);
    let (readiness, startup_phase, startup_detail, startup_error, bootstrap_reason) = {
        let workspace_root = {
            let guard = state.read().expect("state lock");
            guard.ctx.workspace_root.clone()
        };
        if let Err(error) = crate::startup::try_ensure_app_registry_materialized(
            workspace_root.as_path(),
            app_id,
        ) {
            tracing::warn!(
                app_id = %app_id,
                error = %error,
                "lazy app import on access-readiness failed"
            );
        }
        let mut guard = state.write().expect("state lock");
        crate::build_ops::refresh_materialization_flags(&mut guard);
        let axes = crate::review_axes::resolve_page_render_axes(
            &guard,
            &AppQuery {
                data_mode: query.data_mode.clone(),
                ..Default::default()
            },
            route_mode,
        );
        let readiness = crate::startup::evaluate_access_readiness(
            &guard,
            app_id,
            scene_id,
            route_mode,
            axes,
        );
        let bootstrap_reason = if route_mode.is_access_like() {
            Some(
                mei_host_graph::bootstrap_embed_status(
                    guard.ctx.workspace_root.as_path(),
                    app_id,
                    scene_id,
                )
                .reason,
            )
        } else {
            None
        };
        (
            readiness,
            guard.startup_phase.clone(),
            guard.startup_detail.clone(),
            guard.startup_error.clone(),
            bootstrap_reason,
        )
    };
    if readiness.ready {
        tracing::info!(
            target: "mei.startup",
            app_id = %app_id,
            scene_id = %scene_id,
            startup_phase = %startup_phase,
            gate_reason = %readiness.reason,
            bootstrap_reason = bootstrap_reason.as_deref().unwrap_or("-"),
            "app access ready for requests"
        );
    }
    Json(json!({
        "ready": readiness.ready,
        "reason": readiness.reason,
        "bootstrapReason": bootstrap_reason,
        "startupPhase": startup_phase,
        "startupDetail": startup_detail,
        "startupError": startup_error,
        "appId": app_id,
        "sceneId": scene_id,
    }))
    .into_response()
}

#[derive(Debug, Deserialize, Default)]
pub struct SceneRevisionQuery {
    pub app: String,
    pub scene: Option<String>,
    pub mode: Option<String>,
    pub chrome: Option<String>,
    pub data_mode: Option<String>,
    pub review_projection: Option<String>,
}

pub async fn api_scene_revision(
    State(state): State<SharedState>,
    State(auth): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<SceneRevisionQuery>,
) -> Response {
    let app_id = query.app.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app is required"})),
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
    let route_mode = query
        .mode
        .as_deref()
        .map(UiRouteMode::from_slug)
        .unwrap_or(UiRouteMode::App);
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
    if !route_mode.is_access_like() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "mode must be access-like"})),
        )
            .into_response();
    }
    let bootstrap = mei_host_graph::bootstrap_embed_status(
        state
            .read()
            .expect("state lock")
            .ctx
            .workspace_root
            .as_path(),
        app_id,
        scene_id.as_str(),
    );
    if !bootstrap.allowed {
        return Json(json!({
            "ready": false,
            "reason": bootstrap.reason,
            "app_id": app_id,
            "scene_id": scene_id,
        }))
        .into_response();
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let package_root = guard.package_root.as_path();
    let axes = crate::review_axes::resolve_page_render_axes(
        &guard,
        &AppQuery {
            data_mode: query.data_mode.clone(),
            review_projection: query.review_projection.clone(),
            ..Default::default()
        },
        route_mode,
    );
    let discovered = discover_workspace_apps(workspace_root).unwrap_or_default();
    let apps = filter_apps_for_principal(
        discovered.as_slice(),
        principal.as_ref().map(|Extension(p)| p),
    );
    if !apps.iter().any(|app| app.id == app_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    }
    let app_ctx = guard.host_ctx_for_app(app_id);
    let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
        app_ctx.app_root().as_path(),
        Some(workspace_root),
        None,
    );
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal.as_ref().map(|Extension(p)| p));
    let outcome = match mei_host_graph::assemble_scope_from_registry(
        workspace_root,
        app_id,
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
    let Some(payload) = build_scene_revision_payload(
        workspace_root,
        package_root,
        app_id,
        scene_id.as_str(),
        route_mode,
        axes,
        query
            .chrome
            .as_deref()
            .map(|value| value.eq_ignore_ascii_case("none"))
            .unwrap_or(route_mode == UiRouteMode::Run || route_mode == UiRouteMode::Copilot),
        auth_enabled,
        account_view.as_ref(),
        &gis,
        outcome.compiled.component_assets.as_slice(),
    ) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "revision unavailable; warmup bootstrap first"})),
        )
            .into_response();
    };
    Json(payload).into_response()
}

#[derive(Debug, Deserialize, Default)]
pub struct SceneBootstrapQuery {
    pub app: String,
    pub scene: Option<String>,
}

pub async fn api_scene_bootstrap(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<SceneBootstrapQuery>,
) -> Response {
    let app_id = query.app.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app is required"})),
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
    let guard = state.read().expect("state lock");
    if !guard.data_mode_ceiling.allows_eval_api() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": format!(
                    "scene bootstrap unavailable under data mode ceiling `{}`",
                    guard.data_mode_ceiling.slug()
                )
            })),
        )
            .into_response();
    }
    let workspace_root = guard.ctx.workspace_root.as_path();
    let discovered = discover_workspace_apps(workspace_root).unwrap_or_default();
    let apps = filter_apps_for_principal(
        discovered.as_slice(),
        principal.as_ref().map(|Extension(p)| p),
    );
    if !apps.iter().any(|app| app.id == app_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    }
    let bootstrap = mei_host_graph::bootstrap_embed_status(workspace_root, app_id, scene_id.as_str());
    if bootstrap.allowed && bootstrap.reason == "no_client_bootstrap_required" {
        return Json(mei_host_graph::empty_client_bootstrap_payload(
            workspace_root,
            app_id,
            scene_id.as_str(),
        ))
        .into_response();
    }
    let Some(payload) =
        mei_host_graph::build_client_bootstrap_payload(workspace_root, app_id, scene_id.as_str())
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "bootstrap unavailable"})),
        )
            .into_response();
    };
    let _ = mei_host_graph::write_scene_bootstrap_artifact(
        workspace_root,
        app_id,
        scene_id.as_str(),
        &payload,
    );
    Json(payload).into_response()
}

#[derive(Debug, Deserialize, Default)]
pub struct SceneFragmentQuery {
    pub app: String,
    pub scene: Option<String>,
    pub chrome: Option<String>,
    pub data_mode: Option<String>,
    pub review_projection: Option<String>,
}

pub async fn api_scene_fragment(
    State(state): State<SharedState>,
    State(auth): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<SceneFragmentQuery>,
) -> Response {
    let app_id = query.app.trim();
    if app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "app is required"})),
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
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let package_root = guard.package_root.as_path();
    let axes = crate::review_axes::resolve_page_render_axes(
        &guard,
        &AppQuery {
            data_mode: query.data_mode.clone(),
            review_projection: query.review_projection.clone(),
            ..Default::default()
        },
        UiRouteMode::App,
    );
    let discovered = discover_workspace_apps(workspace_root).unwrap_or_default();
    let apps = filter_apps_for_principal(
        discovered.as_slice(),
        principal.as_ref().map(|Extension(p)| p),
    );
    if !apps.iter().any(|app| app.id == app_id) {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "app not found"})),
        )
            .into_response();
    }
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let apps = enrich_discovered_apps(apps.as_slice(), &topbar_menu);
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal.as_ref().map(|Extension(p)| p));
    let app_ctx = guard.host_ctx_for_app(app_id);
    let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
        app_ctx.app_root().as_path(),
        Some(workspace_root),
        None,
    );
    let outcome = match mei_host_graph::assemble_scope_from_registry(
        workspace_root,
        app_id,
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
    let revision_payload = build_scene_revision_payload(
        workspace_root,
        package_root,
        app_id,
        scene_id.as_str(),
        UiRouteMode::App,
        axes,
        query
            .chrome
            .as_deref()
            .map(|value| value.eq_ignore_ascii_case("none"))
            .unwrap_or(false),
        auth_enabled,
        account_view.as_ref(),
        &gis,
        outcome.compiled.component_assets.as_slice(),
    );
    let resolved = match resolve_access_page_html(
        workspace_root,
        package_root,
        apps.as_slice(),
        &topbar_menu,
        app_id,
        scene_id.as_str(),
        UiRouteMode::App,
        &AppQuery {
            chrome: query.chrome.clone(),
            data_mode: query.data_mode.clone(),
            review_projection: query.review_projection.clone(),
            ..Default::default()
        },
        axes,
        auth_enabled,
        account_view.as_ref(),
        None,
    ) {
        Ok(value) => value,
        Err(error) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("render failed: {error}")})),
            )
                .into_response();
        }
    };
    let html = resolved.html;
    let surface_html = extract_preview_surface_html(html.as_str());
    let shell_html = extract_shell_inner_html(html.as_str());
    let title = extract_document_title(html.as_str());
    let mut response = Json(json!({
        "appId": app_id,
        "sceneId": scene_id,
        "title": title,
        "shellHtml": shell_html,
        "surfaceHtml": surface_html,
        "revisionDigest": revision_payload.as_ref().map(|payload| payload.revision_digest.clone()),
        "clientRevision": revision_payload.as_ref().map(|payload| payload.client_revision.clone()),
        "pageRenderCacheHit": resolved.page_render_cache_hit,
    }))
    .into_response();
    insert_page_render_cache_hit_header(&mut response, resolved.page_render_cache_hit);
    response
}

fn extract_document_title(html: &str) -> String {
    let start = match html.find("<title>") {
        Some(value) => value + "<title>".len(),
        None => return String::new(),
    };
    let end = match html[start..].find("</title>") {
        Some(value) => start + value,
        None => return String::new(),
    };
    html[start..end].trim().to_string()
}

fn extract_shell_inner_html(html: &str) -> Option<String> {
    let marker = r#"class="shell""#;
    let start = html.find(marker)?;
    let open_end = html[start..].find('>')? + start + 1;
    let mut depth = 1usize;
    let mut cursor = open_end;
    while cursor < html.len() {
        if html[cursor..].starts_with("<div") {
            depth += 1;
            cursor += 4;
            continue;
        }
        if html[cursor..].starts_with("</div>") {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(html[open_end..cursor].to_string());
            }
            cursor += 6;
            continue;
        }
        cursor += 1;
    }
    None
}

fn extract_preview_surface_html(html: &str) -> Option<String> {
    for selector in [
        r#"data-mei-frame-viewport"#,
        r#"class="preview-surface preview-stage""#,
        r#"class="preview-surface""#,
    ] {
        let Some(pos) = html.find(selector) else {
            continue;
        };
        let fragment = &html[pos.saturating_sub(120)..];
        let Some(start) = fragment.rfind('<') else {
            continue;
        };
        let tail = &html[pos.saturating_sub(120) + start..];
        if let Some(end_rel) = tail.find("</div>") {
            return Some(tail[..end_rel + "</div>".len()].to_string());
        }
    }
    None
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

fn parse_app_scene_path(
    app_tail: &str,
    scene_query: Option<&str>,
    route_mode: UiRouteMode,
) -> (String, String, Option<String>) {
    let parts: Vec<&str> = app_tail.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return (String::new(), "home".to_string(), None);
    }
    let app_id = parts[0].to_string();
    if route_mode == UiRouteMode::Copilot
        && parts.len() >= 3
        && (parts[1] == "presentation" || parts[1] == "tour")
    {
        let presentation_id = parts[2].to_string();
        return (app_id, "home".to_string(), Some(presentation_id));
    }
    let scene = if parts.len() >= 3 && parts[1] == "scene" {
        parts[2].to_string()
    } else {
        scene_query
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("home")
            .to_string()
    };
    (app_id, scene, None)
}

const DEFAULT_ACCESS_PRESENTATION_ID: &str = "intro";

pub(crate) fn inject_presentation_manifest_script(
    html: String,
    workspace_root: &std::path::Path,
    app_id: &str,
    presentation_id: Option<&str>,
) -> String {
    if html.contains("window.__mei.presentation_manifest_prefetch=false") {
        return html;
    }
    let pid = presentation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| crate::presentation_scripts::read_default_script_id(workspace_root, app_id))
        .unwrap_or_else(|| DEFAULT_ACCESS_PRESENTATION_ID.to_string());
    let script = format!(
        r#"<script>window.__mei=window.__mei||{{}};window.__mei.presentation_manifest_prefetch=false;window.__mei.presentation_manifest_mode="library";window.__mei.presentation_manifest_id={pid:?};window.__mei.presentation_default_script_id={pid:?};</script>"#
    );
    if let Some(pos) = html.find("</head>") {
        let mut out = String::with_capacity(html.len() + script.len());
        out.push_str(&html[..pos]);
        out.push_str(&script);
        out.push_str(&html[pos..]);
        out
    } else {
        format!("{script}{html}")
    }
}

pub(crate) fn inject_layer_plane_scripts(html: String, outcome: &mei_host_graph::AssembleOutcome) -> String {
    let layer_plan =
        serde_json::to_string(&outcome.layer_plan).unwrap_or_else(|_| "{}".to_string());
    let presentation_map =
        serde_json::to_string(&outcome.presentation_map).unwrap_or_else(|_| "{}".to_string());
    let world_plan =
        serde_json::to_string(&outcome.world_plan).unwrap_or_else(|_| "{}".to_string());
    let map_projection =
        serde_json::to_string(&outcome.map_projection).unwrap_or_else(|_| "{}".to_string());
    let overlay_defaults = serde_json::to_string(&outcome.overlay_defaults)
        .unwrap_or_else(|_| "{}".to_string());
    let scripts = format!(
        r#"<script type="application/json" id="mei-layer-plan">{layer_plan}</script><script type="application/json" id="mei-presentation-map">{presentation_map}</script><script type="application/json" id="mei-world-plan">{world_plan}</script><script type="application/json" id="mei-map-projection">{map_projection}</script><script>window.__mei=window.__mei||{{}};window.__mei.layer_plan={layer_plan};window.__mei.presentation_map={presentation_map};window.__mei.world_plan={world_plan};window.__mei.map_projection={map_projection};window.__mei.overlay_defaults={overlay_defaults};window.__mei.t2_overlay_defaults={overlay_defaults};window.__mei.page_overlay_defaults={overlay_defaults};</script>"#
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
