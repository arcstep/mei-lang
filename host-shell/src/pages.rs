use axum::{
    extract::{Extension, OriginalUri, Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri},
    response::{Html, IntoResponse, Json, Redirect, Response},
};
use mei_host_auth::{
    account_view_for_principal, filter_apps_for_principal, AuthEnforcement, AuthPrincipal,
    AuthServeState,
};
use mei_lang_app::{
    load_topbar_menu_context, page_body_theme_style, render_host_ssr_bootstrap_head_revision_only,
    render_page, scene_drilldown_context_json_for_host_ssr, UiRouteMode,
};
use mei_lang_kernel::{
    load_workspace_config, resolve_app_root, resolve_build_view_query, LegacyBuildQuery,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use crate::build_info::fill_page_shell_placeholders;
use crate::landing::discover_workspace_apps;
use crate::page_observability::{
    fill_manage_wall_clock_placeholders, fill_page_load_observability_placeholders,
    measure_page_html_payload,
};
use crate::state::SharedState;

#[derive(Debug, Deserialize, Default, Clone)]
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
    /// Structure tree max ui role depth (`content`, `section`, …).
    pub tree_max: Option<String>,
    /// Unified view surface (`app` | `layout` | `prototype`); canonical on `/apps/{id}/view`.
    pub surface: Option<String>,
}

/// Access visit while App Runtime is not running: record INFO and send user to the
/// friendly starting gate (`应用未启动`) instead of Host legacy assemble + client ERROR.
fn redirect_access_app_not_running(uri: &Uri, app_id: &str, scene_id: &str) -> Response {
    let scene = scene_id.trim();
    let scene = if scene.is_empty() { "home" } else { scene };
    tracing::info!(
        target: "mei.startup",
        app_id = %app_id,
        scene_id = %scene,
        path = %uri.path(),
        "access visit while app runtime not running — redirecting to starting page"
    );
    let location = crate::startup::build_starting_location(uri, app_id, scene, "app");
    Redirect::temporary(location.as_str()).into_response()
}

fn resolve_build_node_for_query(query: &AppQuery) -> Option<String> {
    if let Some(node) = query
        .node
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(node.to_string());
    }
    let legacy = LegacyBuildQuery {
        file: query.file.clone(),
        scene: query.scene.clone(),
        world_metric: None,
        world_dataset: None,
        explain: None,
        tab: query.tab.clone(),
    };
    resolve_build_view_query(None, query.scope.as_deref(), query.tab.as_deref(), &legacy)
        .map(|resolved| resolved.node.encode())
}

/// Access 规范：`/apps/{app_id}/{stage_id}` → 反代 app-runtime；未启动则引导到 starting 页。
pub async fn app_stage_page(
    State(http): State<crate::state::HostHttpState>,
    principal: Option<Extension<AuthPrincipal>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path((app_id, stage_id)): Path<(String, String)>,
    Query(_query): Query<AppQuery>,
) -> Response {
    if crate::shell_redirects::is_reserved_stage_segment(stage_id.as_str()) {
        return (StatusCode::NOT_FOUND, "unknown app route").into_response();
    }
    let stage = stage_id.trim();
    if stage.is_empty() {
        return (StatusCode::NOT_FOUND, "stage required").into_response();
    }
    let workspace_root = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    if let Some(location) = crate::shell_chrome::redirect_unknown_access_stage(
        workspace_root.as_path(),
        app_id.as_str(),
        stage,
        uri.query(),
    ) {
        return Redirect::temporary(location.as_str()).into_response();
    }
    match crate::app_runtime_proxy::access_get_gateway(
        &http,
        app_id.as_str(),
        uri.path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or(uri.path()),
        &headers,
        principal.as_ref().map(|ext| ext.0.clone()),
        "access",
    )
    .await
    {
        crate::app_runtime_proxy::GatewayProxyOutcome::Proxied(response) => response,
        crate::app_runtime_proxy::GatewayProxyOutcome::RequiredUnavailable(_)
        | crate::app_runtime_proxy::GatewayProxyOutcome::LegacyFallback => {
            redirect_access_app_not_running(&uri, app_id.as_str(), stage)
        }
    }
}

/// Phase 8.5: `/apps/{app}/~/{scope_or_node}` temporary Stage Access routes.
///
/// Opens a full Access session whose assemble/view-revision closure is the MCG
/// neighborhood of the target — not a carve of the "current" stage URL.
pub async fn app_temp_stage_page(
    State(http): State<crate::state::HostHttpState>,
    principal: Option<Extension<AuthPrincipal>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path((app_id, target_tail)): Path<(String, String)>,
    Query(mut query): Query<AppQuery>,
) -> Response {
    let target = target_tail.trim().trim_matches('/');
    if target.is_empty() {
        return (StatusCode::NOT_FOUND, "temporary stage target required").into_response();
    }

    let Some(hint) = mei_host_graph::parse_temp_stage_target(target) else {
        return (
            StatusCode::BAD_REQUEST,
            format!("unrecognized temporary stage target: {target}"),
        )
            .into_response();
    };

    let stage_guess = mei_host_graph::infer_stage_from_temp_target(target);
    let workspace_root = {
        let guard = http.shell.read().expect("state lock");
        guard.ctx.workspace_root.clone()
    };
    let mut hits = mei_host_graph::ArtifactHitMatrix::default();
    let resolved = match mei_host_graph::ensure_manifest_index(
        workspace_root.as_path(),
        app_id.as_str(),
        stage_guess.as_str(),
        mei_lang_kernel::DataMode::Eval,
        &mut hits,
        None,
    ) {
        Ok(index) => {
            let structure_key = index
                .semantic_layer_refs
                .get("structure.full")
                .map(|layer| layer.artifact_id.clone());
            let doc = structure_key
                .as_deref()
                .and_then(|key| mei_host_graph::take_layer(key))
                .and_then(|bytes| {
                    serde_json::from_slice::<mei_host_graph::StructureFullDocument>(bytes.as_slice())
                        .ok()
                });
            match doc {
                Some(document) => mei_host_graph::resolve_scope_target(&document, hint),
                None => Err(mei_host_graph::ScopeTargetResolveError::NotFound(
                    "structure.full unavailable for temporary stage resolve".to_string(),
                )),
            }
        }
        Err(error) => Err(mei_host_graph::ScopeTargetResolveError::NotFound(
            error.to_string(),
        )),
    };

    match resolved {
        Ok(target_resolved) => {
            let canonical = target_resolved.canonical_path(app_id.as_str());
            let current_path = uri.path().trim_end_matches('/');
            let canonical_trim = canonical.trim_end_matches('/');
            if current_path != canonical_trim {
                let location = match uri.query() {
                    Some(q) if !q.is_empty() => format!("{canonical}?{q}"),
                    _ => canonical,
                };
                return Redirect::temporary(location.as_str()).into_response();
            }
            query.scene = Some(target_resolved.stage_id.clone());
            query.scope = Some(target_resolved.preview_scope.clone());
            query.focus = Some(target_resolved.node_id.clone());
            query.node = Some(target_resolved.node_id.clone());
            query.surface = Some(UiRouteMode::App.slug().to_string());
            query.chrome = Some("none".to_string());
        }
        Err(mei_host_graph::ScopeTargetResolveError::Ambiguous {
            hint,
            count,
            candidates,
        }) => {
            return (
                StatusCode::CONFLICT,
                format!("ambiguous scope target `{hint}` matches {count} nodes: {candidates}"),
            )
                .into_response();
        }
        Err(error) => {
            return (StatusCode::NOT_FOUND, error.to_string()).into_response();
        }
    }

    match crate::app_runtime_proxy::access_get_gateway(
        &http,
        app_id.as_str(),
        uri.path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or(uri.path()),
        &headers,
        principal.as_ref().map(|ext| ext.0.clone()),
        "access",
    )
    .await
    {
        crate::app_runtime_proxy::GatewayProxyOutcome::Proxied(response) => response,
        crate::app_runtime_proxy::GatewayProxyOutcome::RequiredUnavailable(_)
        | crate::app_runtime_proxy::GatewayProxyOutcome::LegacyFallback => {
            redirect_access_app_not_running(
                &uri,
                app_id.as_str(),
                query.scene.as_deref().unwrap_or(stage_guess.as_str()),
            )
        }
    }
}

/// Legacy `/apps/{app}/{stage}/{tier…}` → redirect to `/apps/{app}/~/{scope}`.
pub async fn app_scoped_stage_page(
    State(http): State<crate::state::HostHttpState>,
    principal: Option<Extension<AuthPrincipal>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path((app_id, stage_id, scoped_tail)): Path<(String, String, String)>,
    Query(query): Query<AppQuery>,
) -> Response {
    if crate::shell_redirects::is_reserved_stage_segment(stage_id.as_str()) {
        return (StatusCode::NOT_FOUND, "unknown app route").into_response();
    }
    if stage_id.trim() == "~" {
        return app_temp_stage_page(
            State(http),
            principal,
            OriginalUri(uri),
            headers,
            Path((app_id, scoped_tail)),
            Query(query),
        )
        .await;
    }
    let stage = stage_id.trim();
    let tail = scoped_tail.trim().trim_matches('/');
    if stage.is_empty() || tail.is_empty() {
        return (StatusCode::NOT_FOUND, "scoped stage path required").into_response();
    }

    let Some(parsed) = mei_host_graph::parse_scoped_route_tail(tail) else {
        return (
            StatusCode::BAD_REQUEST,
            format!("unrecognized scoped route tail: {tail}"),
        )
            .into_response();
    };

    match parsed {
        mei_host_graph::ScopedRouteParse::T2Page { page_scene_id } => {
            // Keep T2 page under stage path (page scene assemble).
            let mut q = query;
            q.scene = Some(page_scene_id);
            q.scope = Some(format!("{stage}/t2"));
            q.surface = Some(UiRouteMode::App.slug().to_string());
            match crate::app_runtime_proxy::access_get_gateway(
                &http,
                app_id.as_str(),
                uri.path_and_query()
                    .map(|pq| pq.as_str())
                    .unwrap_or(uri.path()),
                &headers,
                principal.as_ref().map(|ext| ext.0.clone()),
                "access",
            )
            .await
            {
                crate::app_runtime_proxy::GatewayProxyOutcome::Proxied(response) => response,
                crate::app_runtime_proxy::GatewayProxyOutcome::RequiredUnavailable(_)
                | crate::app_runtime_proxy::GatewayProxyOutcome::LegacyFallback => {
                    redirect_access_app_not_running(
                        &uri,
                        app_id.as_str(),
                        q.scene.as_deref().unwrap_or(stage),
                    )
                }
            }
        }
        mei_host_graph::ScopedRouteParse::Structure { hint } => {
            let workspace_root = {
                let guard = http.shell.read().expect("state lock");
                guard.ctx.workspace_root.clone()
            };
            let mut hits = mei_host_graph::ArtifactHitMatrix::default();
            let resolved = match mei_host_graph::ensure_manifest_index(
                workspace_root.as_path(),
                app_id.as_str(),
                stage,
                mei_lang_kernel::DataMode::Eval,
                &mut hits,
                None,
            ) {
                Ok(index) => {
                    let structure_key = index
                        .semantic_layer_refs
                        .get("structure.full")
                        .map(|layer| layer.artifact_id.clone());
                    let doc = structure_key
                        .as_deref()
                        .and_then(|key| mei_host_graph::take_layer(key))
                        .and_then(|bytes| {
                            serde_json::from_slice::<mei_host_graph::StructureFullDocument>(
                                bytes.as_slice(),
                            )
                            .ok()
                        });
                    match doc {
                        Some(document) => mei_host_graph::resolve_scope_target(&document, hint),
                        None => Err(mei_host_graph::ScopeTargetResolveError::NotFound(
                            "structure.full unavailable for scope resolve".to_string(),
                        )),
                    }
                }
                Err(error) => Err(mei_host_graph::ScopeTargetResolveError::NotFound(
                    error.to_string(),
                )),
            };
            match resolved {
                Ok(target) => {
                    let location = match uri.query() {
                        Some(q) if !q.is_empty() => {
                            format!("{}?{q}", target.canonical_path(app_id.as_str()))
                        }
                        _ => target.canonical_path(app_id.as_str()),
                    };
                    return Redirect::temporary(location.as_str()).into_response();
                }
                Err(mei_host_graph::ScopeTargetResolveError::Ambiguous {
                    hint,
                    count,
                    candidates,
                }) => {
                    return (
                        StatusCode::CONFLICT,
                        format!(
                            "ambiguous scope target `{hint}` matches {count} nodes: {candidates}"
                        ),
                    )
                        .into_response();
                }
                Err(error) => {
                    return (StatusCode::NOT_FOUND, error.to_string()).into_response();
                }
            }
        }
    }
}

/// Access 应用根：`/apps/{app_id}` → 反代；未启动则引导到 starting 页。
pub async fn app_root_page(
    State(http): State<crate::state::HostHttpState>,
    principal: Option<Extension<AuthPrincipal>>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(app_id): Path<String>,
    Query(_query): Query<AppQuery>,
) -> Response {
    match crate::app_runtime_proxy::access_get_gateway(
        &http,
        app_id.as_str(),
        uri.path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or(uri.path()),
        &headers,
        principal.as_ref().map(|ext| ext.0.clone()),
        "access",
    )
    .await
    {
        crate::app_runtime_proxy::GatewayProxyOutcome::Proxied(response) => response,
        crate::app_runtime_proxy::GatewayProxyOutcome::RequiredUnavailable(_)
        | crate::app_runtime_proxy::GatewayProxyOutcome::LegacyFallback => {
            let workspace_root = {
                let guard = http.shell.read().expect("state lock");
                guard.ctx.workspace_root.clone()
            };
            let default_scene =
                crate::shell_chrome::default_access_scene(workspace_root.as_path(), app_id.as_str());
            redirect_access_app_not_running(&uri, app_id.as_str(), default_scene.as_str())
        }
    }
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
    // DEPRECATED: Host in-process Access assemble. Prefer `/apps/{app}/{stage}` gateway
    // reverse-proxy to mei-app-runtime when LaunchManifest route is active.
    let request_started = Instant::now();
    if matches!(
        mode.as_str(),
        "run" | "copilot" | "presentation" | "slides" | "speaker"
    ) {
        return (
            StatusCode::NOT_FOUND,
            "legacy /apps/run/* and /apps/copilot/* routes are removed; use /apps/{app_id}/app and presentation actions",
        )
            .into_response();
    }
    // `view` 经 from_slug 归一为 App；若仍落到此处也按 App 处理。
    let route_mode = match mode.as_str() {
        "view" => UiRouteMode::App,
        other => UiRouteMode::from_slug(other),
    };
    let app_tail = app_tail.trim_start_matches('/').to_string();
    let mut query = query;
    if mode == "build" || mode == "manage" {
        return (
            StatusCode::NOT_FOUND,
            "legacy /apps/build/* and /apps/manage/* routes are removed; use /apps/{app_id}/{stage_id}",
        )
            .into_response();
    }
    if route_mode == UiRouteMode::App {
        let workspace_root = {
            let guard = state.read().expect("state lock");
            guard.ctx.workspace_root.clone()
        };
        if let Some(target) = crate::app_surface::legacy_app_access_redirect(
            workspace_root.as_path(),
            app_tail.as_str(),
        ) {
            return Redirect::permanent(target.as_str()).into_response();
        }
    }
    let (app_id, scene_id, tour_id) = if route_mode.is_app_surface() {
        crate::app_surface::merge_surface_query_defaults(&mut query, route_mode);
        crate::app_surface::parse_app_surface_tail(
            app_tail.as_str(),
            query.scene.as_deref(),
            route_mode,
        )
    } else {
        parse_app_scene_path(&app_tail, query.scene.as_deref(), route_mode)
    };
    if app_id.is_empty() {
        return (StatusCode::NOT_FOUND, "app not found").into_response();
    }
    let mut scene_id = scene_id;
    if scene_id == "__default_access__" {
        let workspace_root = {
            let guard = state.read().expect("state lock");
            guard.ctx.workspace_root.clone()
        };
        let app_root = mei_lang_kernel::resolve_app_root(workspace_root.as_path(), app_id.as_str());
        scene_id = mei_lang_kernel::resolve_default_scene_from_root(&app_root)
            .ok()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "home".to_string());
    }
    let _copilot_presentation_id = tour_id.as_deref();
    let revision_first_shell = wants_revision_first_shell(route_mode, &query);
    let thin_template_cache_key =
        (revision_first_shell && auth.auth_enforcement != AuthEnforcement::Required).then(|| {
            thin_shell_template_cache_key(app_id.as_str(), scene_id.as_str(), route_mode, &query)
        });
    let needs_access_readiness_gate = route_mode.is_access_like();
    if needs_access_readiness_gate {
        let starting_location = {
            let (workspace_root, data_plane_enabled) = {
                let guard = state.read().expect("state lock");
                (guard.ctx.workspace_root.clone(), guard.data_plane_enabled)
            };
            if data_plane_enabled {
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
            }
            let mut guard = state.write().expect("state lock");
            crate::build_ops::refresh_materialization_flags(&mut guard);
            let axes = crate::review_axes::resolve_page_render_axes(&guard, &query, route_mode);
            let readiness = crate::startup::evaluate_access_readiness(
                &guard,
                app_id.as_str(),
                scene_id.as_str(),
                route_mode,
                axes,
            );
            let assemble_ready = readiness.ready;
            if readiness.ready && assemble_ready {
                None
            } else {
                // Access 舞台页：starting 的 mode 必须是 app（勿传 view，否则 from_slug 历史误判）
                Some(crate::startup::build_starting_location(
                    &uri,
                    app_id.as_str(),
                    scene_id.as_str(),
                    "app",
                ))
            }
        };
        if let Some(location) = starting_location {
            return Redirect::temporary(location.as_str()).into_response();
        }
    }
    let gate_ms = request_started.elapsed().as_millis() as u64;
    if let Some(entry) = thin_template_cache_key
        .as_deref()
        .and_then(crate::thin_shell_page_cache::get)
    {
        return cached_thin_shell_response(entry, &headers, request_started);
    }
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let package_root = guard.package_root.as_path();
    let discovery_started = Instant::now();
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
    let app_discovery_ms = discovery_started.elapsed().as_millis() as u64;
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
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let apps = crate::shell_chrome::apps_for_topbar(&guard);
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
        if let Some(response) =
            crate::light_pages::try_render_light_page(crate::light_pages::LightPageContext {
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
            })
        {
            return response;
        }
        return (
            StatusCode::NOT_FOUND,
            format!("route mode `{}` is not available for this app", mode),
        )
            .into_response();
    }
    let shell_assemble_outcome = mei_host_graph::assemble_scope_from_registry(
        workspace_root,
        app_id.as_str(),
        scene_id.as_str(),
    )
    .ok()
    .flatten();
    let stage_kind = match shell_assemble_outcome.as_ref() {
        Some(outcome) => crate::review_axes::StageKind::resolve(
            &outcome.compiled.stage_registry,
            &outcome.compiled.scene_routes,
            scene_id.as_str(),
        ),
        _ => crate::review_axes::StageKind::Scene,
    };
    let axes_resolution =
        crate::review_axes::resolve_page_render_axes_with_ceiling_detailed_for_stage(
            guard.data_mode_ceiling,
            &query,
            route_mode,
            stage_kind,
        );
    let axes = axes_resolution.axes;
    let chrome_hidden = query
        .chrome
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("none"))
        .unwrap_or(route_mode == UiRouteMode::Run || route_mode == UiRouteMode::Copilot);
    let shell_compose = compose_request_for_shell(route_mode, &query, axes, chrome_hidden);
    let data_mode_ceiling_notice_owned = if axes_resolution.data_mode_clamped {
        Some(format!(
            "当前部署 data_mode ceiling 为 `{}`，已将请求降档为 `{}`。",
            guard.data_mode_ceiling.slug(),
            axes.data_mode.slug()
        ))
    } else {
        None
    };
    let chrome_host = crate::scene_manifest::SceneChromeHostContext {
        apps: apps.as_slice(),
        topbar_menu: Some(&topbar_menu),
        auth_enabled,
        auth_account: account_view.as_ref(),
    };
    let (mut html, ssr_emit_ms, page_cache_hit) = if route_mode.is_access_like() {
        let render_started = Instant::now();
        let template = render_thin_access_shell(
            thin_access_shell_document(app_id.as_str(), scene_id.as_str()),
            workspace_root,
            package_root,
            app_id.as_str(),
            scene_id.as_str(),
            &shell_compose,
            (!chrome_hidden).then_some(&chrome_host),
            shell_assemble_outcome.as_ref(),
        );
        (template, render_started.elapsed().as_millis() as u64, false)
    } else if route_mode.is_app_surface() && !route_mode.is_app() {
        let render_started = Instant::now();
        let node = resolve_build_node_for_query(&query).unwrap_or_default();
        let template = render_thin_scene_shell(
            thin_workspace_shell_document(
                app_id.as_str(),
                scene_id.as_str(),
                route_mode,
                node.as_str(),
                axes.data_mode.slug(),
                crate::review_axes::ssr_review_projection_for_axes(route_mode, stage_kind, axes)
                    .slug(),
                query.tree_max.as_deref().unwrap_or(""),
            ),
            workspace_root,
            package_root,
            app_id.as_str(),
            scene_id.as_str(),
            route_mode,
            &shell_compose,
            "",
            "",
            Some(&chrome_host),
            shell_assemble_outcome.as_ref(),
        );
        (template, render_started.elapsed().as_millis() as u64, false)
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
                    format!("scene not assembled for app `{app_id}`; run prebuild for this app"),
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
            runtime_roots_owned =
                crate::runtime_snapshot::management_roots_from_snapshot(&snapshot);
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
        let rendered = crate::gis_config::fill_gis_tiles_placeholders(
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
                            Some(
                                crate::review_axes::ssr_review_projection_for_axes(
                                    route_mode, stage_kind, axes,
                                )
                                .slug(),
                            ),
                            data_mode_ceiling_notice_owned.as_deref(),
                            query.tree_max.as_deref(),
                            None,
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
        let ssr_emit_ms = render_started.elapsed().as_millis() as u64;
        (rendered, ssr_emit_ms, false)
    };
    html = crate::gis_config::fill_gis_tiles_placeholders(html, &gis);
    let mut etag_hasher = DefaultHasher::new();
    html.hash(&mut etag_hasher);
    let etag = format!("W/\"{:x}\"", etag_hasher.finish());
    let handler_html_ready_ms = request_started.elapsed().as_millis() as u64;
    html = fill_manage_wall_clock_placeholders(html, ssr_emit_ms, handler_html_ready_ms);
    let serialize_started = Instant::now();
    let payload_stats = measure_page_html_payload(html.as_str());
    html = fill_page_load_observability_placeholders(
        html,
        ssr_emit_ms,
        false,
        payload_stats.html_bytes,
        payload_stats.data_props_bytes,
        payload_stats.data_props_count,
    );
    let serialize_ms = serialize_started.elapsed().as_millis() as u64;
    let _ = mei_host_graph::flush_telemetry_to_registry(workspace_root, app_id.as_str());
    if let Some(cache_key) = thin_template_cache_key {
        crate::thin_shell_page_cache::put(app_id.as_str(), cache_key, html.clone(), etag.clone());
    }
    if revision_first_shell
        && headers
            .get(axum::http::header::IF_NONE_MATCH)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.split(',').any(|part| part.trim() == etag))
    {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        if let Ok(value) = HeaderValue::from_str(etag.as_str()) {
            response
                .headers_mut()
                .insert(axum::http::header::ETAG, value);
        }
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-cache, must-revalidate"),
        );
        return response;
    }
    let mut response = Html(html).into_response();
    if let Ok(value) = HeaderValue::from_str(etag.as_str()) {
        response
            .headers_mut()
            .insert(axum::http::header::ETAG, value);
    }
    let server_timing = format!(
        "gate;dur={gate_ms}, app_discovery;dur={app_discovery_ms}, ssr_emit;dur={ssr_emit_ms}, serialize;dur={serialize_ms}, handler;dur={handler_html_ready_ms}"
    );
    if let Ok(value) = HeaderValue::from_str(server_timing.as_str()) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("server-timing"), value);
    }
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
    if revision_first_shell {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-view-revision-status"),
            HeaderValue::from_static("bootstrap"),
        );
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-assemble-local"),
            HeaderValue::from_static("0"),
        );
        let page_cache_status = if page_cache_hit {
            "template-hit"
        } else {
            "template-bypass"
        };
        if let Ok(value) = HeaderValue::from_str(page_cache_status) {
            response
                .headers_mut()
                .insert(HeaderName::from_static("x-mei-page-cache"), value);
        }
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-cache, must-revalidate"),
        );
    }
    if route_mode == UiRouteMode::Runtime && !revision_first_shell {
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
    #[serde(alias = "app_id")]
    pub app: Option<String>,
    #[serde(alias = "scene_id")]
    pub scene: Option<String>,
    pub mode: Option<String>,
}

pub async fn host_starting_page(
    State(http): State<crate::state::HostHttpState>,
    State(auth): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<HostStartingQuery>,
) -> Response {
    let state = &http.shell;
    let (workspace, default_app, error, running_apps) = {
        let mut guard = state.write().expect("state lock");
        crate::build_ops::refresh_materialization_flags(&mut guard);
        let running = crate::shell_chrome::apps_for_topbar(&guard);
        (
            guard.ctx.workspace_root.clone(),
            guard.ctx.app_id.clone(),
            guard.startup_error.clone(),
            running,
        )
    };
    if let Some(message) = error {
        return mei_host_auth::startup_failed_html_response(workspace.as_path(), message.as_str());
    }
    let return_path = {
        let raw = query.return_path.trim();
        crate::startup::sanitize_return_path(if raw.is_empty() { "/" } else { raw })
    };
    let (poll_app, poll_scene, poll_mode) = if query
        .app
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
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
    if poll_app.trim().is_empty() {
        return Redirect::temporary("/runtime").into_response();
    }
    let route_mode = if poll_mode.as_str() == "view" {
        let surface = return_path
            .split('?')
            .nth(1)
            .and_then(|query| {
                query.split('&').find_map(|pair| {
                    let (key, value) = pair.split_once('=')?;
                    (key == "surface").then(|| value)
                })
            })
            .unwrap_or("app");
        crate::scene_manifest::resolve_route_mode_from_surface(Some(surface))
    } else {
        UiRouteMode::from_slug(poll_mode.as_str())
    };
    let (already_ready, gate_status) = {
        let data_plane_enabled = state.read().expect("state lock").data_plane_enabled;
        if data_plane_enabled {
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
        }
        let mut guard = state.write().expect("state lock");
        crate::build_ops::refresh_materialization_flags(&mut guard);
        let axes =
            crate::review_axes::resolve_page_render_axes(&guard, &AppQuery::default(), route_mode);
        let readiness = crate::startup::evaluate_access_readiness(
            &guard,
            poll_app.as_str(),
            poll_scene.as_str(),
            route_mode,
            axes,
        );
        let assemble_ready = matches!(
            mei_host_graph::assemble_scope_from_registry(
                workspace.as_path(),
                poll_app.as_str(),
                poll_scene.as_str(),
            ),
            Ok(Some(_))
        );
        let supervisor = http.app_runtime.lock().ok();
        let runtime_ready = supervisor.as_ref().is_some_and(|slot| {
            crate::state::runtime_identity_for_app(&guard, slot, poll_app.as_str(), None).is_some()
        });
        let runtime_gate_ready = !matches!(
            crate::legacy_compat::decide_data_plane_gate(runtime_ready),
            crate::legacy_compat::DataPlaneGate::RuntimeRequired
        );
        let gate_reason = if !readiness.ready {
            readiness.reason
        } else if !assemble_ready {
            "assembling"
        } else if !runtime_gate_ready {
            "runtime_starting"
        } else {
            readiness.reason
        };
        let title = crate::access_gate_status::resolve_access_gate_title(
            &guard,
            supervisor.as_ref().and_then(|slot| slot.as_ref()),
            poll_app.as_str(),
            gate_reason,
        );
        (
            readiness.ready && assemble_ready && runtime_gate_ready,
            title,
        )
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
    let topbar_menu = load_topbar_menu_context(workspace.as_path());
    let auth_enabled = auth.auth_enforcement == AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal.as_ref().map(|Extension(p)| p));
    let body_html = mei_host_auth::render_startup_warming_main_html(gate_status);
    let poll_script = mei_host_auth::startup_warming_poll_script(
        return_path.as_str(),
        poll_app.as_str(),
        poll_scene.as_str(),
        poll_mode.as_str(),
    );
    let html = crate::workspace_page::render_workspace_shell_page(
        workspace.as_path(),
        running_apps.as_slice(),
        &topbar_menu,
        mei_lang_app::WorkspaceShellNav::Home,
        gate_status,
        body_html.as_str(),
        auth_enabled,
        account_view.as_ref(),
    );
    let html = if let Some(idx) = html.rfind("</body>") {
        let mut out = String::with_capacity(html.len() + poll_script.len());
        out.push_str(&html[..idx]);
        out.push_str(poll_script.as_str());
        out.push_str(&html[idx..]);
        out
    } else {
        format!("{html}{poll_script}")
    };
    Html(html).into_response()
}

#[derive(Debug, Deserialize, Default)]
pub struct AccessReadinessQuery {
    pub app: String,
    pub scene: Option<String>,
    pub mode: Option<String>,
    pub data_mode: Option<String>,
}

pub async fn api_host_access_readiness(
    State(http): State<crate::state::HostHttpState>,
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
    let runtime_ready = {
        let shell = http.shell.read().expect("state lock");
        let supervisor = http.app_runtime.lock().expect("app-runtime lock");
        crate::state::runtime_identity_for_app(&shell, &supervisor, app_id, None).is_some()
    };
    if runtime_ready {
        return Json(json!({
            "ready": true,
            "reason": "runtime_ready",
            "title": "应用已就绪",
            "bootstrapReason": null,
            "startupPhase": "ready",
            "startupDetail": "App Runtime 已就绪",
            "startupError": null,
            "appId": app_id,
            "sceneId": scene_id,
        }))
        .into_response();
    }
    let state = &http.shell;
    let (ready, reason, startup_phase, startup_detail, startup_error, bootstrap_reason, title) = {
        let workspace_root = {
            let guard = state.read().expect("state lock");
            guard.ctx.workspace_root.clone()
        };
        let data_plane_enabled = state.read().expect("state lock").data_plane_enabled;
        if data_plane_enabled {
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
        let readiness =
            crate::startup::evaluate_access_readiness(&guard, app_id, scene_id, route_mode, axes);
        let assemble_ready = readiness.ready
            && matches!(
                mei_host_graph::assemble_scope_from_registry(
                    guard.ctx.workspace_root.as_path(),
                    app_id,
                    scene_id,
                ),
                Ok(Some(_))
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
        let runtime_gate_ready = !matches!(
            crate::legacy_compat::decide_data_plane_gate(false),
            crate::legacy_compat::DataPlaneGate::RuntimeRequired
        );
        let gate_ready = readiness.ready && assemble_ready && runtime_gate_ready;
        let gate_reason = if !readiness.ready {
            readiness.reason
        } else if !assemble_ready {
            "assembling"
        } else if !runtime_gate_ready {
            "runtime_starting"
        } else {
            readiness.reason
        };
        let supervisor = http.app_runtime.lock().ok();
        let title = crate::access_gate_status::resolve_access_gate_title(
            &guard,
            supervisor.as_ref().and_then(|slot| slot.as_ref()),
            app_id,
            gate_reason,
        );
        (
            gate_ready,
            gate_reason,
            guard.startup_phase.clone(),
            guard.startup_detail.clone(),
            guard.startup_error.clone(),
            bootstrap_reason,
            title,
        )
    };
    if ready {
        tracing::info!(
            target: "mei.startup",
            app_id = %app_id,
            scene_id = %scene_id,
            startup_phase = %startup_phase,
            gate_reason = %reason,
            bootstrap_reason = bootstrap_reason.as_deref().unwrap_or("-"),
            "app access ready for requests"
        );
    }
    Json(json!({
        "ready": ready,
        "reason": reason,
        "title": title,
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
pub struct SceneBootstrapQuery {
    pub app: String,
    pub scene: Option<String>,
    pub fingerprint: Option<String>,
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
    if !guard.data_mode_ceiling_for(app_id).allows_eval_api() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": format!(
                    "scene bootstrap unavailable under data mode ceiling `{}`",
                    guard.data_mode_ceiling_for(app_id).slug()
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
    let bootstrap =
        mei_host_graph::bootstrap_embed_status(workspace_root, app_id, scene_id.as_str());
    if bootstrap.allowed && bootstrap.reason == "no_client_bootstrap_required" {
        return Json(mei_host_graph::empty_client_bootstrap_payload(
            workspace_root,
            app_id,
            scene_id.as_str(),
        ))
        .into_response();
    }
    // Prefer a real pack payload; if bootstrap is stale/missing, degrade to empty so Access
    // can continue via eval layers instead of hard-failing with 404.
    let payload =
        mei_host_graph::build_client_bootstrap_payload(workspace_root, app_id, scene_id.as_str())
            .unwrap_or_else(|| {
                mei_host_graph::empty_client_bootstrap_payload(
                    workspace_root,
                    app_id,
                    scene_id.as_str(),
                )
            });
    let _ = mei_host_graph::write_scene_bootstrap_artifact(
        workspace_root,
        app_id,
        scene_id.as_str(),
        &payload,
    );
    let mut response = Json(payload).into_response();
    response.headers_mut().insert(
        axum::http::HeaderName::from_static("deprecation"),
        axum::http::HeaderValue::from_static("true"),
    );
    response.headers_mut().insert(
        axum::http::HeaderName::from_static("link"),
        axum::http::HeaderValue::from_static(
            "</api/host/scene-eval-pack>; rel=\"successor-version\"",
        ),
    );
    if let Some(fingerprint) = query
        .fingerprint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Ok(value) = axum::http::HeaderValue::from_str(fingerprint) {
            response.headers_mut().insert(
                axum::http::HeaderName::from_static("x-mei-eval-fingerprint"),
                value,
            );
        }
    }
    response
}

#[derive(Debug, Deserialize, Default)]
pub struct SceneEvalPackQuery {
    pub app: String,
    pub scene: Option<String>,
    pub scope: Option<String>,
    pub fingerprint: Option<String>,
    #[serde(rename = "client_revision")]
    pub client_revision: Option<String>,
    #[serde(rename = "neighbor_hops")]
    pub neighbor_hops: Option<usize>,
    #[allow(dead_code)]
    pub pack: Option<String>,
}

pub async fn api_scene_eval_pack(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<SceneEvalPackQuery>,
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
        .scope
        .as_deref()
        .or(query.scene.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home")
        .to_string();
    let guard = state.read().expect("state lock");
    if !guard.data_mode_ceiling_for(app_id).allows_eval_api() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": format!(
                    "scene eval pack unavailable under data mode ceiling `{}`",
                    guard.data_mode_ceiling_for(app_id).slug()
                )
            })),
        )
            .into_response();
    }
    let workspace_root = guard.ctx.workspace_root.clone();
    let discovered = discover_workspace_apps(workspace_root.as_path()).unwrap_or_default();
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
    drop(guard);
    let pack = mei_host_graph::build_scene_eval_pack(
        workspace_root.as_path(),
        app_id,
        scene_id.as_str(),
        mei_host_graph::SceneEvalPackBuildOptions {
            client_revision: query
                .client_revision
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            fingerprint: query
                .fingerprint
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            neighbor_hops: query.neighbor_hops,
        },
    );
    if pack.status == mei_host_graph::SceneEvalPackStatus::PackMiss {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "eval pack unavailable", "status": "pack_miss"})),
        )
            .into_response();
    }
    let _ = mei_host_graph::write_scene_bootstrap_artifact(
        workspace_root.as_path(),
        app_id,
        scene_id.as_str(),
        &mei_host_graph::build_client_bootstrap_payload(
            workspace_root.as_path(),
            app_id,
            scene_id.as_str(),
        )
        .unwrap_or_else(|| {
            mei_host_graph::empty_client_bootstrap_payload(
                workspace_root.as_path(),
                app_id,
                scene_id.as_str(),
            )
        }),
    );
    let mut response = Json(pack).into_response();
    if let Some(fingerprint) = query
        .fingerprint
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Ok(value) = axum::http::HeaderValue::from_str(fingerprint) {
            response.headers_mut().insert(
                axum::http::HeaderName::from_static("x-mei-eval-fingerprint"),
                value,
            );
        }
    }
    response
}

#[derive(Debug, Deserialize, Default)]
pub struct SceneDrilldownQuery {
    pub app: String,
    pub scene: Option<String>,
}

pub async fn api_scene_drilldown_context(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<SceneDrilldownQuery>,
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
    let Some(outcome) =
        mei_host_graph::assemble_scope_from_registry(workspace_root, app_id, scene_id.as_str())
            .ok()
            .flatten()
    else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "scene not found"})),
        )
            .into_response();
    };
    let payload_text =
        scene_drilldown_context_json_for_host_ssr(&outcome.compiled, Some(scene_id.as_str()));
    let payload: serde_json::Value =
        serde_json::from_str(payload_text.as_str()).unwrap_or(json!({}));
    let etag = format!(
        "\"{app_id}:{scene_id}:{}\"",
        outcome.compile_revision.trim()
    );
    let mut response = Json(payload).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    if let Ok(value) = HeaderValue::from_str(etag.as_str()) {
        response
            .headers_mut()
            .insert(axum::http::header::ETAG, value);
    }
    response
}

fn parse_app_scene_path(
    app_tail: &str,
    scene_query: Option<&str>,
    route_mode: UiRouteMode,
) -> (String, String, Option<String>) {
    let parts: Vec<&str> = app_tail
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
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
    if route_mode.is_access_like() && parts.len() >= 2 && parts[1] == "access" {
        let scene = if parts.len() >= 4 && parts[2] == "scene" {
            parts[3].to_string()
        } else {
            scene_query
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("__default_access__")
                .to_string()
        };
        return (app_id, scene, None);
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

fn build_preview_route_requested(query: &AppQuery) -> bool {
    if matches!(
        query
            .tab
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("preview")
    ) {
        return true;
    }
    resolve_build_node_for_query(query).is_some()
}

fn wants_revision_first_shell(route_mode: UiRouteMode, query: &AppQuery) -> bool {
    if route_mode.is_access_like() || route_mode.is_app_surface() {
        return true;
    }
    route_mode.is_build() && build_preview_route_requested(query)
}

fn thin_shell_template_cache_key(
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    query: &AppQuery,
) -> String {
    format!("{app_id}:{scene_id}:{}:{query:?}", route_mode.slug())
}

fn cached_thin_shell_response(
    entry: crate::thin_shell_page_cache::ThinShellCacheEntry,
    request_headers: &HeaderMap,
    request_started: Instant,
) -> Response {
    let not_modified = request_headers
        .get(axum::http::header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.split(',').any(|part| part.trim() == entry.etag));
    let mut response = if not_modified {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        Html(entry.html).into_response()
    };
    if let Ok(value) = HeaderValue::from_str(entry.etag.as_str()) {
        response
            .headers_mut()
            .insert(axum::http::header::ETAG, value);
    }
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache, must-revalidate"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-mei-page-cache"),
        HeaderValue::from_static("template-hit"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("x-mei-view-revision-status"),
        HeaderValue::from_static("bootstrap"),
    );
    let elapsed_ms = request_started.elapsed().as_millis();
    if let Ok(value) = HeaderValue::from_str(elapsed_ms.to_string().as_str()) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-handler-html-ready-ms"),
            value,
        );
    }
    if let Ok(value) =
        HeaderValue::from_str(format!("thin_template;dur=0, handler;dur={elapsed_ms}").as_str())
    {
        response
            .headers_mut()
            .insert(HeaderName::from_static("server-timing"), value);
    }
    response
}

fn compose_request_for_shell(
    route_mode: UiRouteMode,
    query: &AppQuery,
    axes: crate::review_axes::PageRenderAxes,
    chrome_hidden: bool,
) -> mei_host_graph::ComposeRequest {
    mei_host_graph::ComposeRequest {
        route_mode: Some(route_mode.slug().to_string()),
        tab: Some(
            query
                .tab
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(if route_mode.uses_workspace_tree() {
                    "preview"
                } else {
                    "scene"
                })
                .to_string(),
        ),
        chrome: Some(if chrome_hidden { "none" } else { "full" }.to_string()),
        review_projection: Some(axes.review_projection.slug().to_string()),
        data_mode: Some(axes.data_mode.slug().to_string()),
        focus: query
            .focus
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        scope: query
            .scope
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        scope_target: None,
    }
}

pub(crate) fn render_thin_access_shell(
    html: String,
    workspace_root: &std::path::Path,
    package_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    compose: &mei_host_graph::ComposeRequest,
    chrome_host: Option<&crate::scene_manifest::SceneChromeHostContext<'_>>,
    assemble_outcome: Option<&mei_host_graph::AssembleOutcome>,
) -> String {
    render_thin_scene_shell(
        html,
        workspace_root,
        package_root,
        app_id,
        scene_id,
        mei_lang_app::UiRouteMode::App,
        compose,
        "",
        "",
        chrome_host,
        assemble_outcome,
    )
}

/// Thin Access / App shell：revision-first 页面不含完整 Leptos shell，须内嵌 FAB DOM，
/// 否则 `copilot-fab-context` 找不到 `#access-chat-fab`。
const THIN_SHELL_ACCESS_FAB_HTML: &str = concat!(
    r#"<div id="access-chat-floating-root" class="access-chat-floating-root" data-open="false" data-mei-stage-kind="scene" data-mei-fab-policy="required">"#,
    r#"<button id="access-chat-fab" class="access-chat-fab" type="button" aria-label="展开 Copilot 工具条" title="展开 Copilot 工具条" data-mei-fab-policy="required">"#,
    r#"<img class="access-chat-fab-icon" src="/app-assets/favicon.svg" alt="" />"#,
    r#"</button></div>"#,
);

pub(crate) fn thin_access_shell_document(app_id: &str, scene_id: &str) -> String {
    format!(
        r#"<!DOCTYPE html><html lang="zh-CN"><head><meta charset="utf-8"><title>{app_id}</title></head><body class="__MEI_THIN_BODY_CLASS__" style="__MEI_PAGE_BODY_THEME_STYLE__" data-mei-view="app" data-app-id="{app_id}" data-scene-id="{scene_id}"><div class="shell shell-surface scene-shell mei-text-primary min-h-screen flex min-h-0 flex-col" id="mei-compose-host" data-scene="{scene_id}"><div id="mei-host-topbar-slot" data-mei-host-chrome="top"></div><main class="main flex min-h-0 flex-1 flex-col overflow-hidden"><div class="preview-pane-scroll shell-inner min-h-0 flex-1 overflow-auto" id="mei-compose-root" data-scene="{scene_id}" data-mei-compose-placeholder="1" aria-busy="true"></div><div id="mei-thin-shell-fallback" class="mei-thin-shell-fallback mei-p-4 mei-text-muted hidden" role="status" hidden>正在加载场景内容…</div></main><div id="mei-host-statusbar-slot" data-mei-host-chrome="bottom"></div></div>{fab}</body></html>"#,
        app_id = app_id,
        scene_id = scene_id,
        fab = THIN_SHELL_ACCESS_FAB_HTML,
    )
}

const THIN_WORKSPACE_ROOT_INNER: &str = concat!(
    r#"<div id="workspace-root" class="workspace manage-workspace chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0 build-thin-shell-root">"#,
    r#"<aside class="sidebar left workspace-panel workspace-panel-side workspace-panel-nav h-full min-h-0 min-w-0 overflow-hidden flex flex-col px-4 py-2.5">"#,
    r#"<div class="sidebar-scroll flex-1 min-h-0 overflow-auto"><div class="build-tree-shell" data-build-tree-shell="true"><div class="build-reachability-tree" data-build-tree-mode-active="structure" aria-label="场景原型导航"></div></div></div></aside>"#,
    r#"<div class="splitter splitter-left" data-workspace-splitter="left" role="separator" aria-orientation="vertical" aria-label="调整左侧资源栏宽度"></div>"#,
    r#"<main class="main h-full min-w-0 min-h-0 overflow-hidden px-0">"#,
    r#"<section class="main-pane workspace-panel workspace-panel-main min-w-0 min-h-0 flex h-full flex-col overflow-hidden px-2 py-3.5">"#,
    r#"<div class="manage-tab-stage min-h-0 min-w-0 flex flex-1 flex-col overflow-hidden">"#,
    r#"<section class="manage-tab-panel preview-pane min-h-0 min-w-0 flex flex-col overflow-hidden" data-manage-tab-panel="preview">"#,
    r#"<div class="preview-pane-scroll min-h-0 min-w-0 flex-1 overflow-auto" data-review-projection="{review_projection}" data-data-mode="{data_mode}" data-mei-compose-placeholder="1" aria-busy="true"></div>"#,
    r#"<div id="build-inspect-bar" class="build-inspect-bar shrink-0 border-t mei-border-default px-3 py-2 mei-font-1 mei-text-muted" data-build-inspect-bar="true">"#,
    r#"<span id="build-inspect-bar-label">在左侧体验树选择 Panel/Block，或在预览中点击组件以指认上下文。</span></div></section>"#,
    r#"<section class="manage-tab-panel min-h-0 min-w-0 overflow-auto" data-manage-tab-panel="exec" hidden></section>"#,
    r#"<section class="manage-tab-panel min-h-0 min-w-0 overflow-auto" data-manage-tab-panel="semantic" hidden></section>"#,
    r#"<section class="manage-tab-panel min-h-0 min-w-0 overflow-auto" data-manage-tab-panel="eval" hidden></section>"#,
    r#"<section class="manage-tab-panel min-h-0 min-w-0 overflow-auto" data-manage-tab-panel="artifact" hidden></section>"#,
    r#"</div></section></main></div>"#,
);

fn thin_workspace_shell_main(data_mode: &str, review_projection: &str) -> String {
    THIN_WORKSPACE_ROOT_INNER
        .replace("{data_mode}", data_mode)
        .replace("{review_projection}", review_projection)
}

pub(crate) fn thin_workspace_shell_document(
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    node: &str,
    data_mode: &str,
    review_projection: &str,
    tree_max_ui_role: &str,
) -> String {
    let route_slug = route_mode.slug();
    let tree_max_attr = if tree_max_ui_role.trim().is_empty() {
        String::new()
    } else {
        format!(r#" data-build-tree-max-ui-role="{tree_max_ui_role}""#)
    };
    let workspace_main = thin_workspace_shell_main(data_mode, review_projection);
    format!(
        r#"<!DOCTYPE html><html lang="zh-CN"><head><meta charset="utf-8"><title>{app_id}</title></head><body class="__MEI_THIN_BODY_CLASS__" style="__MEI_PAGE_BODY_THEME_STYLE__" data-app-id="{app_id}" data-scene-id="{scene_id}" data-route-mode="{route_slug}"><div id="mei-host-topbar-slot" data-mei-host-chrome="top"></div><div class="shell build-thin-shell" data-scene="{scene_id}" data-build-node="{node}" data-data-mode="{data_mode}" data-review-projection="{review_projection}"{tree_max_attr}>{workspace_main}</div><nav id="mei-build-reachability-tree" class="build-reachability-tree" hidden aria-hidden="true"></nav><script id="mei-build-reachability-tree" type="application/json">[]</script><div id="mei-host-statusbar-slot" data-mei-host-chrome="bottom"></div><div id="mei-thin-shell-fallback" class="mei-thin-shell-fallback mei-p-4 mei-text-muted hidden" role="status" hidden>正在加载场景内容…</div></body></html>"#
    )
}

pub(crate) fn render_thin_scene_shell(
    html: String,
    workspace_root: &std::path::Path,
    package_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    route_mode: mei_lang_app::UiRouteMode,
    compose: &mei_host_graph::ComposeRequest,
    draft_session: &str,
    draft_digest: &str,
    chrome_host: Option<&crate::scene_manifest::SceneChromeHostContext<'_>>,
    assemble_outcome: Option<&mei_host_graph::AssembleOutcome>,
) -> String {
    let handler_started = std::time::Instant::now();
    let html = inject_thin_shell_body_presentation(
        html,
        workspace_root,
        route_mode,
        assemble_outcome.map(|outcome| &outcome.compiled),
    );
    let html = inject_thin_shell_runtime_assets(html, route_mode);
    let html = fill_page_shell_placeholders(html, workspace_root);
    let mut html = inject_scene_manifest_refs_for_route(
        html,
        workspace_root,
        app_id,
        scene_id,
        route_mode,
        compose,
        draft_session,
        draft_digest,
        chrome_host,
    );
    if let Some(outcome) = assemble_outcome {
        let data_mode = compose
            .data_mode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let bootstrap_html = render_host_ssr_bootstrap_head_revision_only(
            &outcome.compiled,
            app_id,
            app_id,
            Some(scene_id),
            data_mode,
        );
        if !bootstrap_html.trim().is_empty() {
            html = inject_html_before_head_close(html, bootstrap_html.as_str());
        }
        let scene_bundle_url = resolve_thin_shell_scene_bundle_url(
            package_root,
            workspace_root,
            app_id,
            scene_id,
            route_mode,
            &outcome.compiled,
        );
        html = inject_thin_shell_component_scripts(
            html,
            &outcome.compiled,
            scene_bundle_url.as_deref(),
        );
        html = inject_presentation_manifest_script(html, workspace_root, app_id, None);
    }
    inject_handler_html_ready_ms(html, handler_started)
}

fn resolve_thin_shell_scene_bundle_url(
    package_root: &std::path::Path,
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    route_mode: mei_lang_app::UiRouteMode,
    compiled: &mei_lang_kernel::CompiledApp,
) -> Option<String> {
    if !route_mode.is_access_like() {
        return None;
    }
    let app_root = resolve_app_root(workspace_root, app_id);
    if !crate::scene_bundle::should_build_scene_bundle(app_root.as_path(), route_mode, scene_id) {
        return None;
    }
    let probe = crate::scene_bundle::probe_scene_component_bundle(
        package_root,
        workspace_root,
        app_id,
        scene_id,
        &compiled.component_assets,
    );
    if let Some(build) = probe.build.as_ref() {
        crate::scene_bundle::schedule_scene_component_bundle_build(
            package_root,
            workspace_root,
            build,
        );
    }
    probe.bundle.map(|bundle| bundle.url)
}

fn inject_html_before_head_close(html: String, fragment: &str) -> String {
    if let Some(pos) = html.find("</head>") {
        let mut out = String::with_capacity(html.len() + fragment.len());
        out.push_str(&html[..pos]);
        out.push_str(fragment);
        out.push_str(&html[pos..]);
        out
    } else {
        format!("{fragment}{html}")
    }
}

fn inject_thin_shell_component_scripts(
    html: String,
    compiled: &mei_lang_kernel::CompiledApp,
    scene_bundle_url: Option<&str>,
) -> String {
    use mei_host_auth::html_escape;
    let scripts = if let Some(url) = scene_bundle_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let escaped = html_escape(url);
        format!(
            r#"<link rel="modulepreload" href="{escaped}"/><script type="module" src="{escaped}" data-mei-scene-bundle="true" data-mei-persistent-script="{escaped}"></script>"#
        )
    } else {
        compiled
            .component_assets
            .iter()
            .map(|asset| {
                let src = html_escape(format!("/workspace-components/{}", asset.script).as_str());
                format!(r#"<script type="module" src="{src}"></script>"#)
            })
            .collect::<Vec<_>>()
            .join("")
    };
    if scripts.is_empty() {
        return html;
    }
    inject_html_before_head_close(html, scripts.as_str())
}

fn thin_shell_body_class(route_mode: mei_lang_app::UiRouteMode) -> &'static str {
    match route_mode {
        mei_lang_app::UiRouteMode::App => "app-view sl-theme-dark",
        mei_lang_app::UiRouteMode::Layout => "layout-view sl-theme-dark",
        mei_lang_app::UiRouteMode::Prototype => "prototype-view sl-theme-dark",
        mei_lang_app::UiRouteMode::Run => "run-view chrome-none sl-theme-dark",
        mei_lang_app::UiRouteMode::Copilot => "copilot-view chrome-none sl-theme-dark",
        mei_lang_app::UiRouteMode::Runtime => "runtime-view sl-theme-dark",
        mei_lang_app::UiRouteMode::Config => "config-view sl-theme-dark",
        mei_lang_app::UiRouteMode::Upload => "upload-view sl-theme-dark",
    }
}

fn inject_thin_shell_body_presentation(
    mut html: String,
    workspace_root: &std::path::Path,
    route_mode: mei_lang_app::UiRouteMode,
    compiled: Option<&mei_lang_kernel::CompiledApp>,
) -> String {
    use mei_host_auth::html_escape;

    let workspace = load_workspace_config(workspace_root);
    let theme_style = page_body_theme_style(&workspace, compiled, None);
    html = html.replace(
        "__MEI_PAGE_BODY_THEME_STYLE__",
        html_escape(theme_style.as_str()).as_str(),
    );
    html = html.replace("__MEI_THIN_BODY_CLASS__", thin_shell_body_class(route_mode));
    html = html.replace("__MEI_ROUTE_MODE_SLUG__", route_mode.slug());
    html
}

fn inject_handler_html_ready_ms(mut html: String, started: std::time::Instant) -> String {
    if html.contains("data-mei-handler-html-ready-ms=") {
        return html;
    }
    let ms = started.elapsed().as_millis();
    if let Some(body_start) = html.find("<body") {
        if let Some(rel_close) = html[body_start..].find('>') {
            let insert_at = body_start + rel_close;
            html.insert_str(
                insert_at,
                format!(r#" data-mei-handler-html-ready-ms="{}""#, ms).as_str(),
            );
        }
    }
    html
}

fn inject_thin_shell_runtime_assets(html: String, route_mode: mei_lang_app::UiRouteMode) -> String {
    let (preload_href, bundle_src) = if route_mode.uses_workspace_tree() {
        (
            "/app-bundles/manage.js?v=__MEI_HOST_ASSET_VERSION__",
            "/app-bundles/manage.js?v=__MEI_HOST_ASSET_VERSION__",
        )
    } else {
        (
            "/app-bundles/access.js?v=__MEI_HOST_ASSET_VERSION__",
            "/app-bundles/access.js?v=__MEI_HOST_ASSET_VERSION__",
        )
    };
    let runtime = format!(
        concat!(
            r#"<meta name="viewport" content="width=device-width, initial-scale=1"/>"#,
            r#"<meta name="mei-tiles-base-url" content="__MEI_TILES_BASE_URL__"/>"#,
            r#"<meta name="mei-tiles-json-path" content="__MEI_TILES_JSON_PATH__"/>"#,
            r#"<meta name="mei-host-version" content="__MEI_HOST_VERSION__"/>"#,
            r#"<meta name="mei-host-version-label" content="__MEI_HOST_VERSION_LABEL__"/>"#,
            r#"<meta name="mei-host-icp-record" content="__MEI_HOST_ICP_RECORD__"/>"#,
            r#"<meta name="mei-host-psb-record" content="__MEI_HOST_PSB_RECORD__"/>"#,
            r#"<meta name="mei-host-copyright" content="__MEI_HOST_COPYRIGHT__"/>"#,
            r#"<meta name="mei-workspace-label" content="__MEI_WORKSPACE_LABEL__"/>"#,
            r#"<meta name="mei-view" content="{route_slug}"/>"#,
            r#"<link rel="icon" href="/app-assets/favicon.svg" type="image/svg+xml"/>"#,
            r#"<link rel="stylesheet" href="/app-bundles/styles.css?v=__MEI_HOST_ASSET_VERSION__"/>"#,
            r#"<script src="/app-assets/spa-navigation/visit-history-store.js"></script>"#,
            r#"<script src="/app-assets/page-load-progress-shell.js"></script>"#,
            r#"<script>(function(){{try{{if(window.MeiPageLoadProgress){{window.MeiPageLoadProgress.mountEarlyHandoffOverlay();}}}}catch(e){{}}}})();</script>"#,
            r#"<link rel="preload" href="{preload_href}" as="script"/>"#,
            r#"<script defer src="/app-assets/host-http-feedback.js"></script>"#,
            r#"<script defer src="/app-bundles/shoelace.js"></script>"#,
            r#"<script defer src="{bundle_src}"></script>"#
        ),
        route_slug = route_mode.slug(),
        preload_href = preload_href,
        bundle_src = bundle_src,
    );
    if let Some(pos) = html.find("</head>") {
        let mut out = String::with_capacity(html.len() + runtime.len());
        out.push_str(&html[..pos]);
        out.push_str(&runtime);
        out.push_str(&html[pos..]);
        out
    } else {
        format!("{runtime}{html}")
    }
}

const DEFAULT_ACCESS_PRESENTATION_ID: &str = "intro";

pub(crate) fn inject_scene_manifest_refs_for_route(
    html: String,
    workspace_root: &std::path::Path,
    app_id: &str,
    scene_id: &str,
    route_mode: mei_lang_app::UiRouteMode,
    compose: &mei_host_graph::ComposeRequest,
    draft_session: &str,
    draft_digest: &str,
    chrome_host: Option<&crate::scene_manifest::SceneChromeHostContext<'_>>,
) -> String {
    let mut hits = crate::artifact_observability::ArtifactHitMatrix::default();
    let data_mode = compose
        .data_mode
        .as_deref()
        .and_then(mei_lang_kernel::DataMode::parse)
        .unwrap_or(mei_lang_kernel::DataMode::Eval);
    let envelope = if draft_session.trim().is_empty() && draft_digest.trim().is_empty() {
        crate::scene_manifest::ensure_manifest_index(
            workspace_root,
            app_id,
            scene_id,
            data_mode,
            &mut hits,
            chrome_host,
        )
        .ok()
        .map(|index| {
            let surface = index
                .surfaces
                .iter()
                .find(|surface| surface.route_mode == route_mode.slug());
            json!({
                "schema_version": "mei.view-revision-envelope.v1",
                "app_id": index.app_id.as_str(),
                "scene_id": index.scene_id.as_str(),
                "manifest_revision_digest": index.manifest_revision_digest.as_str(),
                "surface_revision_digest": surface.map(|value| value.surface_revision_digest.as_str()),
                "compose_defaults": surface.map(|value| &value.compose_defaults).unwrap_or(compose),
                "scene_bundle_url": format!(
                    "/api/host/scene-manifest?app_id={}&scene={}&surface={}",
                    app_id,
                    scene_id,
                    route_mode.slug()
                ),
            })
        })
    } else {
        crate::scene_manifest::build_scene_view_manifest(
            workspace_root,
            app_id,
            scene_id,
            route_mode,
            data_mode,
            compose,
            draft_session,
            draft_digest,
            &mut hits,
            chrome_host,
        )
        .ok()
        .map(|value| {
            json!({
                "schema_version": "mei.view-revision-envelope.v1",
                "app_id": value.app_id,
                "scene_id": value.scene_id,
                "manifest_revision_digest": value.revision_digest,
                "surface_revision_digest": value.surface_revision_digest,
                "compose_defaults": value.compose_defaults,
                "scene_bundle_url": format!(
                    "/api/host/scene-manifest?app_id={}&scene={}&surface={}",
                    app_id,
                    scene_id,
                    route_mode.slug()
                ),
            })
        })
    }
    .unwrap_or_else(|| {
            json!({
                "schema_version": "mei.view-revision-envelope.v1",
                "app_id": app_id,
                "scene_id": scene_id,
            })
    });
    let envelope_json = serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string());
    let hits_json = serde_json::to_string(&hits).unwrap_or_else(|_| "{}".to_string());
    let dev_eval_json =
        serde_json::to_string(&crate::dev_eval_scope::current_for_app(app_id).client_payload())
            .unwrap_or_else(|_| {
                "{\"profile\":\"full\",\"scopes\":[],\"fill\":\"placeholder\"}".to_string()
            });
    let script = format!(
        r#"<script>window.__mei=window.__mei||{{}};window.__mei.view_revision_envelope={envelope_json};window.__mei.scene_manifest_refs={envelope_json};window.__mei.dev_eval={dev_eval_json};window.__mei.thin_shell=true;window.__mei.artifact_hits={hits_json};window.__mei.view_revision_enabled=true;</script>"#,
        envelope_json = envelope_json,
        hits_json = hits_json,
        dev_eval_json = dev_eval_json,
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

#[cfg(test)]
mod inject_scene_manifest_tests {
    use super::inject_scene_manifest_refs_for_route;

    #[test]
    fn wants_revision_first_shell_defaults_on_app_surfaces() {
        use super::{wants_revision_first_shell, AppQuery, UiRouteMode};
        let query = AppQuery::default();
        assert!(wants_revision_first_shell(UiRouteMode::App, &query));
        assert!(wants_revision_first_shell(UiRouteMode::Layout, &query));
        assert!(wants_revision_first_shell(UiRouteMode::Prototype, &query));
        assert!(!wants_revision_first_shell(UiRouteMode::Runtime, &query));
    }

    #[test]
    fn inject_scene_manifest_refs_sets_thin_shell_flags() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"id":"test","version":"20260628"}}"#,
        )
        .expect("workspace.json");
        std::fs::create_dir_all(tmp.path().join("apps/demo")).expect("app dir");
        std::fs::write(
            tmp.path().join("apps/demo/app.config.json"),
            r#"{"schemaVersion":1,"app":{"id":"demo"}}"#,
        )
        .expect("app.config");
        let env_v1 = tmp.path().join("apps/demo/env/v1/var");
        std::fs::create_dir_all(&env_v1).expect("env var dir");
        #[cfg(unix)]
        std::os::unix::fs::symlink("v1", tmp.path().join("apps/demo/env/current"))
            .expect("symlink");
        #[cfg(not(unix))]
        std::fs::create_dir_all(tmp.path().join("apps/demo/env/current/var")).expect("env current");
        let compose = mei_host_graph::ComposeRequest {
            route_mode: Some(mei_lang_app::UiRouteMode::App.slug().to_string()),
            tab: Some("scene".to_string()),
            chrome: Some("full".to_string()),
            review_projection: Some(
                crate::review_axes::ssr_review_projection(
                    mei_lang_app::UiRouteMode::App,
                    crate::review_axes::StageKind::Scene,
                    mei_lang_kernel::DataMode::Eval,
                )
                .slug()
                .to_string(),
            ),
            data_mode: Some(mei_lang_kernel::DataMode::Eval.slug().to_string()),
            focus: None,
            scope: None,
            scope_target: None,
        };
        let html = "<html><head></head><body></body></html>".to_string();
        let out = inject_scene_manifest_refs_for_route(
            html,
            tmp.path(),
            "demo",
            "home",
            mei_lang_app::UiRouteMode::App,
            &compose,
            "",
            "",
            None,
        );
        assert!(out.contains("thin_shell"));
        assert!(out.contains("scene_manifest_refs"));
        assert!(out.contains("view_revision_envelope"));
        assert!(out.contains("dev_eval"));
        assert!(out.contains("artifact_hits"));
        assert!(!out.contains(r#""layers":"#));
    }
}

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

pub(crate) fn inject_layer_plane_scripts(
    html: String,
    outcome: &mei_host_graph::AssembleOutcome,
) -> String {
    let layer_plan =
        serde_json::to_string(&outcome.layer_plan).unwrap_or_else(|_| "{}".to_string());
    let presentation_map =
        serde_json::to_string(&outcome.presentation_map).unwrap_or_else(|_| "{}".to_string());
    let world_plan =
        serde_json::to_string(&outcome.world_plan).unwrap_or_else(|_| "{}".to_string());
    let map_projection =
        serde_json::to_string(&outcome.map_projection).unwrap_or_else(|_| "{}".to_string());
    let overlay_defaults =
        serde_json::to_string(&outcome.overlay_defaults).unwrap_or_else(|_| "{}".to_string());
    let component_assets = serde_json::to_string(&outcome.compiled.component_assets)
        .unwrap_or_else(|_| "[]".to_string());
    let stage_registry =
        serde_json::to_string(&mei_host_graph::stage_registry_bootstrap(&outcome.compiled))
            .unwrap_or_else(|_| "{}".to_string());
    let stage_programs =
        serde_json::to_string(&mei_host_graph::stage_programs_bootstrap(&outcome.compiled))
            .unwrap_or_else(|_| "{}".to_string());
    let narration_catalogs =
        serde_json::to_string(&mei_host_graph::narration_catalogs_bootstrap(&outcome.compiled))
            .unwrap_or_else(|_| "{}".to_string());
    let scene_routes =
        serde_json::to_string(&outcome.compiled.scene_routes).unwrap_or_else(|_| "[]".to_string());
    let scripts = format!(
        r#"<script type="application/json" id="mei-layer-plan">{layer_plan}</script><script type="application/json" id="mei-presentation-map">{presentation_map}</script><script type="application/json" id="mei-world-plan">{world_plan}</script><script type="application/json" id="mei-map-projection">{map_projection}</script><script>window.__mei=window.__mei||{{}};window.__mei.layer_plan={layer_plan};window.__mei.presentation_map={presentation_map};window.__mei.world_plan={world_plan};window.__mei.map_projection={map_projection};window.__mei.overlay_defaults={overlay_defaults};window.__mei.t2_overlay_defaults={overlay_defaults};window.__mei.page_overlay_defaults={overlay_defaults};window.__mei.component_assets={component_assets};window.__mei.scene_routes={scene_routes};window.__mei.stage_registry={stage_registry};window.__mei.stage_programs={stage_programs};window.__mei.narration_catalogs={narration_catalogs};</script>"#
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
    let Some(fragment) =
        mei_host_graph::build_client_bootstrap_head_fragment(workspace_root, app_id, scene_id)
    else {
        let status = mei_host_graph::bootstrap_embed_status(workspace_root, app_id, scene_id);
        if status.allowed {
            if status.reason == "no_client_bootstrap_required" {
                tracing::debug!(
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
                    "client bootstrap SSR inject skipped despite allowed status"
                );
            }
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
