mod access_gate;
pub(super) mod compile;
mod light_pages;
mod render;

use std::time::Instant;

use axum::{
    extract::{Extension, Path as AxumPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{
    compile_scene_from_build_node, compile_scene_from_build_node_with_app, discover_apps,
    preview_target_from_build_node, preview_target_from_build_node_with_app, resolve_app_root,
    resolve_default_scene_from_root, BuildNodeId, CompileOptions,
};

use crate::{
    auth::{AuthEnforcement, AuthPrincipal},
    http::host_error_page::{self, HostShellAction},
    AppError, AppState,
};

use super::super::components::resolve_components_root;
use super::super::menus::load_segment_topbar_menus;
use super::super::util::{elapsed_ms, is_script_target};
use super::page_render::{
    access_only_surface_enabled, account_view_for_principal, app_title_for,
    lightweight_access_scene, list_upload_files, upload_rel_from_config,
};
use super::query::{
    access_canonical_location, access_sanitized_redirect_location, legacy_access_redirect_location,
    legacy_manage_redirect_location, parse_access_scene_path,
    presentation_sanitized_redirect_location, scene_projection_canonical_location, AppQuery,
};

use crate::http::compile_cache::load_compile_artifact_only;
use crate::http::compile_cache::CompileWithCacheOutcome;

use access_gate::check_access_scene_gate;
use compile::{maybe_handle_compile_bootstrap_probe, resolve_compile_outcome, CompileResolution};
use light_pages::{try_render_light_page, LightPageContext};

pub async fn app_page(
    State(state): State<AppState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath((mode, app_id_raw)): AxumPath<(String, String)>,
    Query(query): Query<AppQuery>,
) -> Result<Response, AppError> {
    let principal = principal.map(|Extension(value)| value);
    let auth_enabled = state.auth_enforcement == AuthEnforcement::Required;
    let account_view = if auth_enabled {
        account_view_for_principal(principal.as_ref())
    } else {
        None
    };
    let app_started = Instant::now();
    if mode == "access" {
        if let Some(location) = legacy_access_redirect_location(&app_id_raw, &query) {
            return Ok(Redirect::temporary(&location).into_response());
        }
    }
    if mode == "manage" {
        let location = legacy_manage_redirect_location(&app_id_raw, &query);
        return Ok(Redirect::temporary(&location).into_response());
    }
    let route_mode = UiRouteMode::from_slug(&mode);
    let app_id_trimmed = app_id_raw.trim_start_matches('/').to_string();
    let (app_id, url_path_scene) = match parse_access_scene_path(&app_id_trimmed) {
        Ok(None) => (app_id_trimmed, None),
        Ok(Some((app, scene))) => (app, Some(scene)),
        Err(()) => {
            return Ok((
                StatusCode::NOT_FOUND,
                Html(host_error_page::render_error_page(
                    StatusCode::NOT_FOUND,
                    "场景路径无效",
                    "地址中的 /scene/<id> 格式无效，请检查链接是否正确。",
                    Some("/apps/app/.../scene/<id>"),
                    &[HostShellAction {
                        href: "/".to_string(),
                        label: "返回首页".to_string(),
                        primary: true,
                    }],
                )),
            )
                .into_response());
        }
    };
    let access_path_scene = if route_mode.uses_scene_route() {
        url_path_scene.clone()
    } else {
        None
    };
    if app_id.is_empty() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "missing app id in route",
        ));
    }
    let app_root = resolve_app_root(state.source_root.as_path(), &app_id);
    let access_only_surface = access_only_surface_enabled();
    if auth_enabled {
        if let Some(ref auth_principal) = principal {
            if !auth_principal.can_access_host_route_mode(route_mode.slug()) {
                return Ok(host_error_page::forbidden_html_response(&format!(
                    "当前角色无法访问「{}」视图",
                    route_mode.label()
                )));
            }
        }
    }
    if access_only_surface && !route_mode.is_access_like() {
        let desired_scene = url_path_scene
            .as_deref()
            .map(str::trim)
            .filter(|scene| !scene.is_empty())
            .map(str::to_string)
            .or_else(|| {
                query
                    .scene
                    .as_deref()
                    .map(str::trim)
                    .filter(|scene| !scene.is_empty())
                    .map(str::to_string)
            })
            .or_else(|| resolve_default_scene_from_root(&app_root).ok().flatten());
        if let Some(scene_id) = desired_scene {
            return Ok(Redirect::temporary(&access_canonical_location(
                &app_id,
                &scene_id,
                query.tab.as_deref(),
                Some("none"),
            ))
            .into_response());
        }
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "access-only surface requires a resolvable scene entrypoint",
        ));
    }
    tracing::info!(
        app_id = %app_id,
        route_mode = route_mode.slug(),
        request_scene = %query.scene.as_deref().unwrap_or("-"),
        request_file = %query.file.as_deref().unwrap_or("-"),
        request_tab = %query.tab.as_deref().unwrap_or("-"),
        phase = "start",
        "app page request started"
    );
    let request_file = query
        .file
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let manage_file = if route_mode == UiRouteMode::Build {
        request_file.clone()
    } else {
        None
    };
    let manage_script_file = manage_file
        .as_deref()
        .filter(|t| is_script_target(t))
        .map(ToString::to_string);
    let build_node = if route_mode == UiRouteMode::Build {
        query.node.as_deref().and_then(BuildNodeId::parse)
    } else {
        None
    };
    let (build_node_compile_scene, build_node_preview_target) = if route_mode == UiRouteMode::Build
    {
        if let Some(node) = build_node.as_ref() {
            let mut scene_hint = compile_scene_from_build_node(node);
            let mut preview_target = preview_target_from_build_node(node);
            if scene_hint.is_none() || preview_target.is_none() {
                let probe_components_root = resolve_components_root(&state.source_root);
                if let Some(outcome) = load_compile_artifact_only(
                    &state,
                    &app_id,
                    &CompileOptions {
                        scene: scene_hint.clone(),
                        preview_target: None,
                    },
                    probe_components_root.as_path(),
                ) {
                    if scene_hint.is_none() {
                        scene_hint =
                            compile_scene_from_build_node_with_app(node, Some(&outcome.compiled));
                    }
                    if preview_target.is_none() {
                        preview_target =
                            preview_target_from_build_node_with_app(node, Some(&outcome.compiled));
                    }
                }
            }
            (scene_hint, preview_target)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    let normalized_preview_target = if route_mode == UiRouteMode::Build {
        manage_script_file.clone().or(build_node_preview_target)
    } else {
        None
    };
    let mut compile_scene = if route_mode.uses_scene_route() || route_mode == UiRouteMode::Build {
        url_path_scene
            .clone()
            .or_else(|| query.scene.clone())
            .or_else(|| build_node_compile_scene.clone())
    } else {
        query.scene.clone()
    };
    if route_mode == UiRouteMode::Build && compile_scene.is_none() {
        if let Some(ref target) = normalized_preview_target {
            if target.ends_with(".board.mei") {
                let probe_components_root = resolve_components_root(&state.source_root);
                if let Some(outcome) = load_compile_artifact_only(
                    &state,
                    &app_id,
                    &CompileOptions {
                        scene: None,
                        preview_target: None,
                    },
                    probe_components_root.as_path(),
                ) {
                    let exports = outcome
                        .compiled
                        .build_board_index
                        .exports_for_board_file(target.as_str());
                    if exports.len() == 1 {
                        compile_scene = Some(exports[0].scene_id.clone());
                    }
                }
            }
        }
    }
    let components_root = resolve_components_root(&state.source_root);
    let compile_options = CompileOptions {
        scene: compile_scene.clone(),
        preview_target: normalized_preview_target.clone(),
    };
    if let Some(response) = maybe_handle_compile_bootstrap_probe(
        &state,
        route_mode,
        &app_id,
        &query,
        &compile_options,
        components_root.as_path(),
        access_path_scene.as_deref(),
    ) {
        return Ok(response);
    }
    if route_mode == UiRouteMode::Presentation && request_file.is_some() {
        return Ok(
            Redirect::temporary(&presentation_sanitized_redirect_location(&app_id, &query))
                .into_response(),
        );
    }
    if route_mode == UiRouteMode::Build {
        if request_file
            .as_deref()
            .map(str::trim)
            .is_some_and(|file| file == ".mei-config.json")
        {
            return Ok(Redirect::temporary(&format!("/apps/config/{app_id}")).into_response());
        }
    }
    if route_mode.uses_scene_route() {
        if let Some(ref file) = request_file {
            if is_script_target(file) {
                let location = if route_mode == UiRouteMode::Presentation {
                    presentation_sanitized_redirect_location(&app_id, &query)
                } else {
                    access_sanitized_redirect_location(&app_id, &query)
                };
                return Ok(Redirect::temporary(&location).into_response());
            }
        }
    }
    if route_mode == UiRouteMode::Upload {
        if upload_rel_from_config(&app_root, &state.source_root).is_none() {
            return Err(AppError::status(
                axum::http::StatusCode::NOT_FOUND,
                "app has no paths.upload configured",
            ));
        }
    }
    let access_static_file = if route_mode == UiRouteMode::App {
        request_file
            .as_ref()
            .filter(|t| !is_script_target(t))
            .cloned()
    } else {
        None
    };
    if route_mode.uses_scene_route() && access_static_file.is_none() {
        let q_scene = query
            .scene
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(ref ps) = access_path_scene {
            if let Some(qs) = q_scene {
                if qs != ps {
                    return Ok(Redirect::temporary(&scene_projection_canonical_location(
                        route_mode,
                        &app_id,
                        ps,
                        query.tab.as_deref(),
                        query.chrome.as_deref(),
                    ))
                    .into_response());
                }
            }
        } else if let Some(qs) = q_scene {
            return Ok(Redirect::temporary(&scene_projection_canonical_location(
                route_mode,
                &app_id,
                qs,
                query.tab.as_deref(),
                query.chrome.as_deref(),
            ))
            .into_response());
        } else if let Ok(Some(default_scene)) =
            resolve_default_scene_from_root(&resolve_app_root(state.source_root.as_path(), &app_id))
        {
            return Ok(Redirect::temporary(&scene_projection_canonical_location(
                route_mode,
                &app_id,
                &default_scene,
                query.tab.as_deref(),
                query.chrome.as_deref(),
            ))
            .into_response());
        }
    }
    let discover_started = Instant::now();
    let mut apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    if auth_enabled {
        if let Some(ref auth_principal) = principal {
            apps.retain(|app| auth_principal.can_access_app(app.id.as_str()));
        }
    }
    let discover_ms = elapsed_ms(discover_started);
    let app_title = app_title_for(&apps, &app_id);
    let chrome_hidden = route_mode == UiRouteMode::Presentation
        || access_only_surface
        || query
            .chrome
            .as_deref()
            .map(|value| value.eq_ignore_ascii_case("none"))
            .unwrap_or(false);
    let topbar_menus = load_segment_topbar_menus(&state.source_root);
    let upload_rel = upload_rel_from_config(&app_root, &state.source_root);
    let upload_enabled = upload_rel.is_some();
    let upload_files = upload_rel
        .as_ref()
        .map(|rel| list_upload_files(&app_root.join(rel), rel))
        .unwrap_or_default();
    let upload_root_label = upload_rel.as_deref().unwrap_or("upload").to_string();
    let lightweight_scene = lightweight_access_scene(&app_root, query.scene.as_deref());
    if let Some(response) = try_render_light_page(LightPageContext {
        state: &state,
        route_mode,
        app_id: &app_id,
        query: &query,
        apps: &apps,
        app_title: app_title.as_str(),
        topbar_menus: &topbar_menus,
        lightweight_scene: lightweight_scene.as_deref(),
        upload_enabled,
        upload_root_label: upload_root_label.as_str(),
        upload_files: &upload_files,
        auth_enabled,
        account_view: account_view.as_ref(),
        request_file: request_file.as_deref(),
        manage_file: manage_file.as_deref(),
        app_started,
    }) {
        return Ok(response);
    }
    let compile_resolution = resolve_compile_outcome(
        &state,
        route_mode,
        &app_id,
        &query,
        compile_options,
        components_root,
        access_path_scene.as_deref(),
        manage_file.as_deref(),
        &apps,
        &topbar_menus,
        normalized_preview_target.as_deref(),
        chrome_hidden,
        upload_enabled,
        upload_root_label.as_str(),
        &upload_files,
        auth_enabled,
        account_view.as_ref(),
        discover_ms,
        app_started,
    );
    let resolved_compile = match compile_resolution {
        CompileResolution::EarlyResponse(response) => return Ok(response),
        CompileResolution::Outcome(outcome) => outcome,
    };
    let compile::ResolvedCompileOutcome {
        outcome: compile_outcome,
        feedback: compile_feedback,
    } = resolved_compile;
    if let Some(response) = check_access_scene_gate(
        route_mode,
        &app_id,
        access_static_file.as_deref(),
        access_path_scene.as_deref(),
        &compile_outcome.compiled,
        principal.as_ref(),
        query.tab.as_deref(),
        query.chrome.as_deref(),
    ) {
        return Ok(response);
    }
    let CompileWithCacheOutcome {
        compiled,
        cache_hit,
        artifact_cache_hit: _,
        compile_revision,
        revision_scope,
        cache_validation,
        cache_lookup_ms,
        artifact_load_ms: _,
        compile_cache_lock_wait_ms: _,
        compile_ms,
    } = compile_outcome;
    let mut compiled = compiled;
    Ok(render::render_compiled_success(
        &state,
        route_mode,
        &app_id,
        &query,
        &apps,
        &topbar_menus,
        &mut compiled,
        cache_hit,
        &compile_revision,
        &revision_scope,
        &cache_validation,
        cache_lookup_ms,
        compile_ms,
        &compile_feedback,
        access_static_file.as_deref(),
        access_path_scene.as_deref(),
        manage_file.as_deref(),
        normalized_preview_target.as_deref(),
        chrome_hidden,
        upload_enabled,
        upload_root_label.as_str(),
        &upload_files,
        auth_enabled,
        account_view.as_ref(),
        discover_ms,
        app_started,
    ))
}
