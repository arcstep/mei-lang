use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use mei_lang_app::{render_page, UiRouteMode};
use mei_lang_kernel::{
    discover_apps, read_source_file, resolve_default_scene_from_root, CompileOptions, CompiledApp,
    Severity,
};

use crate::{AppError, AppState};

use super::super::super::compile_cache::{
    compile_app_with_cache, peek_compile_cache_hit, recent_compile_failure,
    start_compile_in_background_if_needed, CompileWithCacheFailure, CompileWithCacheOutcome,
};
use super::super::app_render::{compile_error_fallback_app, source_panel_meta};
use super::super::components::resolve_components_root;
use super::super::menus::load_segment_topbar_menus;
use super::super::util::{
    elapsed_ms, fill_gis_tiles_placeholders, fill_manage_wall_clock_placeholders,
    fill_perf_placeholders, is_script_target, push_manage_page_pipeline_diag,
};
use super::compiling_shell::{compile_bootstrap_enabled, render_compiling_shell};
use super::query::{
    access_canonical_location, access_sanitized_redirect_location, parse_access_scene_path,
    AppQuery,
};
use super::scene::{canonical_scene_for_target, default_file_for_scene, manage_scene_for_render};

fn html_escape_min(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn diagnostic_message_by_code(compiled: &CompiledApp, code: &str) -> Option<String> {
    compiled
        .diagnostics
        .iter()
        .find(|diag| diag.code == code)
        .map(|diag| diag.message.clone())
}

fn insert_manage_compile_observability_headers(res: &mut Response, compiled: &CompiledApp) {
    for (header, code) in [
        (
            "x-mei-compile-optimization-status",
            "compile_optimization_status",
        ),
        ("x-mei-compile-cache-stats", "compile_cache_stats"),
        ("x-mei-dependency-graph-stats", "dependency_graph_stats"),
        ("x-mei-catalog-filter-stats", "catalog_filter_stats"),
    ] {
        let Some(message) = diagnostic_message_by_code(compiled, code) else {
            continue;
        };
        if let Ok(value) = HeaderValue::from_str(&message) {
            res.headers_mut()
                .insert(HeaderName::from_static(header), value);
        }
    }
}

fn insert_manage_compile_request_headers(res: &mut Response, outcome: &CompileWithCacheOutcome) {
    for (header, value) in [
        (
            "x-mei-compile-cache-hit",
            if outcome.cache_hit { "1" } else { "0" }.to_string(),
        ),
        (
            "x-mei-compile-revision-scope",
            outcome.revision_scope.clone(),
        ),
        (
            "x-mei-compile-cache-validation",
            outcome.cache_validation.clone(),
        ),
        ("x-mei-compile-ms", outcome.compile_ms.to_string()),
        (
            "x-mei-compile-cache-lookup-ms",
            outcome.cache_lookup_ms.to_string(),
        ),
    ] {
        if let Ok(header_value) = HeaderValue::from_str(&value) {
            res.headers_mut()
                .insert(HeaderName::from_static(header), header_value);
        }
    }
}

pub async fn app_page(
    State(state): State<AppState>,
    AxumPath((mode, app_id_raw)): AxumPath<(String, String)>,
    Query(query): Query<AppQuery>,
) -> Result<Response, AppError> {
    let app_started = Instant::now();
    let route_mode = UiRouteMode::from_slug(&mode);
    let app_id_trimmed = app_id_raw.trim_start_matches('/').to_string();
    let (app_id, url_path_scene) = match parse_access_scene_path(&app_id_trimmed) {
        Ok(None) => (app_id_trimmed, None),
        Ok(Some((app, scene))) => (app, Some(scene)),
        Err(()) => {
            return Ok((
                StatusCode::NOT_FOUND,
                Html(
                    "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>场景路径无效</title></head><body><p>地址中的 <code>/scene/&lt;id&gt;</code> 无效。</p></body></html>".to_string(),
                ),
            )
                .into_response());
        }
    };
    let access_path_scene = if route_mode == UiRouteMode::Access {
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
    tracing::info!(
        app_id = %app_id,
        route_mode = route_mode.slug(),
        request_scene = %query.scene.as_deref().unwrap_or("-"),
        request_file = %query.file.as_deref().unwrap_or("-"),
        request_tab = %query.tab.as_deref().unwrap_or("-"),
        phase = "start",
        "app page request started"
    );
    if route_mode == UiRouteMode::Access
        && query
            .file
            .as_ref()
            .map(|f| !f.trim().is_empty())
            .unwrap_or(false)
    {
        return Ok(
            Redirect::temporary(&access_sanitized_redirect_location(&app_id, &query))
                .into_response(),
        );
    }
    if route_mode == UiRouteMode::Access {
        let q_scene = query
            .scene
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(ref ps) = access_path_scene {
            if let Some(qs) = q_scene {
                if qs != ps {
                    return Ok(Redirect::temporary(&access_canonical_location(
                        &app_id,
                        ps,
                        query.tab.as_deref(),
                        query.chrome.as_deref(),
                    ))
                    .into_response());
                }
            }
        } else if let Some(qs) = q_scene {
            return Ok(Redirect::temporary(&access_canonical_location(
                &app_id,
                qs,
                query.tab.as_deref(),
                query.chrome.as_deref(),
            ))
            .into_response());
        } else if let Ok(Some(default_scene)) =
            resolve_default_scene_from_root(&state.source_root.join(&app_id))
        {
            return Ok(Redirect::temporary(&access_canonical_location(
                &app_id,
                &default_scene,
                query.tab.as_deref(),
                query.chrome.as_deref(),
            ))
            .into_response());
        }
    }
    let discover_started = Instant::now();
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let discover_ms = elapsed_ms(discover_started);
    let chrome_hidden = query
        .chrome
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("none"))
        .unwrap_or(false);
    let manage_file = if route_mode == UiRouteMode::Manage {
        query
            .file
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    };
    let manage_script_file = manage_file
        .as_deref()
        .filter(|t| is_script_target(t))
        .map(ToString::to_string);
    let normalized_preview_target = if route_mode == UiRouteMode::Manage {
        manage_script_file.clone()
    } else {
        None
    };
    let compile_scene = if route_mode == UiRouteMode::Access || route_mode == UiRouteMode::Manage {
        url_path_scene.clone().or_else(|| query.scene.clone())
    } else {
        query.scene.clone()
    };
    let components_root = resolve_components_root(&state.source_root);
    let compile_options = CompileOptions {
        scene: compile_scene.clone(),
        preview_target: normalized_preview_target.clone(),
    };
    let compile_outcome = if compile_bootstrap_enabled()
        && !recent_compile_failure(&app_id, &compile_options)
    {
        let peek_started = Instant::now();
        match peek_compile_cache_hit(&state, &app_id, &compile_options, components_root.as_path()) {
            Some(hit) => CompileWithCacheOutcome {
                compiled: hit.compiled,
                cache_hit: true,
                revision_scope: hit.revision_scope,
                cache_validation: hit.cache_validation,
                cache_lookup_ms: elapsed_ms(peek_started),
                compile_cache_lock_wait_ms: 0,
                compile_ms: 0,
            },
            None => {
                start_compile_in_background_if_needed(
                    state.clone(),
                    app_id.clone(),
                    compile_options.clone(),
                    components_root.clone(),
                );
                let scene_hint = compile_scene.as_deref().or(access_path_scene.as_deref());
                let shell = render_compiling_shell(route_mode, &app_id, scene_hint);
                tracing::info!(
                    app_id = %app_id,
                    route_mode = route_mode.slug(),
                    phase = "compile_bootstrap_shell",
                    "serving compile bootstrap shell while compile runs in background"
                );
                return Ok(Html(shell).into_response());
            }
        }
    } else {
        match compile_app_with_cache(&state, &app_id, compile_options, components_root.as_path()) {
            Ok(outcome) => outcome,
            Err(failure) => {
                let CompileWithCacheFailure {
                    error,
                    revision_scope,
                    cache_validation,
                    cache_lookup_ms,
                    compile_cache_lock_wait_ms,
                    compile_ms,
                } = failure;
                tracing::warn!(
                    app_id = %app_id,
                    %error,
                    revision_scope,
                    cache_validation,
                    cache_lookup_ms,
                    compile_cache_lock_wait_ms,
                    compile_ms,
                    "failed to compile app page"
                );
                let target = if route_mode == UiRouteMode::Manage {
                    manage_file
                        .clone()
                        .unwrap_or_else(|| "main.mei".to_string())
                } else {
                    "main.mei".to_string()
                };
                let source_path = state.source_root.join(&app_id).join(&target);
                let source_started = Instant::now();
                let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
                let source_read_ms = elapsed_ms(source_started);
                let source_meta = source_panel_meta(&source_path, &source);
                let topbar_menus = load_segment_topbar_menus(&state.source_root);
                let mut compiled = compile_error_fallback_app(
                    &state.source_root,
                    &app_id,
                    target.as_str(),
                    error.to_string().as_str(),
                );
                let manage_scene_resolved =
                    manage_scene_for_render(&compiled, query.scene.as_deref());
                let render = |cc: &CompiledApp| {
                    let t = Instant::now();
                    let html = render_page(
                        &apps,
                        cc,
                        &app_id,
                        Some(&topbar_menus),
                        route_mode,
                        Some(target.as_str()),
                        Some(source.as_str()),
                        Some(&source_meta),
                        manage_scene_resolved.as_deref(),
                        normalized_preview_target.as_deref(),
                        query.tab.as_deref(),
                        query.diag_filter.as_deref(),
                        chrome_hidden,
                    );
                    (html, elapsed_ms(t))
                };
                push_manage_page_pipeline_diag(
                    &mut compiled,
                    &app_id,
                    target.as_str(),
                    discover_ms,
                    compile_ms,
                    false,
                    cache_lookup_ms,
                    source_read_ms,
                    0,
                    0,
                    0,
                    None,
                    None,
                    None,
                    elapsed_ms(app_started),
                );
                let (html, ssr_http_response_body_ms, handler_html_ready_ms) = {
                    let t = Instant::now();
                    let h = render(&compiled).0;
                    let ssr_emit_ms = elapsed_ms(t);
                    let total_wall = elapsed_ms(app_started);
                    let h = fill_perf_placeholders(h, ssr_emit_ms, total_wall);
                    let handler_ms = elapsed_ms(app_started);
                    let h = fill_manage_wall_clock_placeholders(h, ssr_emit_ms, handler_ms);
                    let h = fill_gis_tiles_placeholders(h, state.gis_tiles.as_ref());
                    (h, ssr_emit_ms, handler_ms)
                };
                let mut res = Html(html).into_response();
                if route_mode == UiRouteMode::Manage {
                    if let Ok(v) = HeaderValue::from_str(&handler_html_ready_ms.to_string()) {
                        res.headers_mut()
                            .insert(HeaderName::from_static("x-mei-handler-html-ready-ms"), v);
                    }
                    if let Ok(v) = HeaderValue::from_str(&ssr_http_response_body_ms.to_string()) {
                        res.headers_mut().insert(
                            HeaderName::from_static("x-mei-ssr-http-response-body-ms"),
                            v,
                        );
                    }
                    insert_manage_compile_observability_headers(&mut res, &compiled);
                }
                tracing::info!(
                    app_id = %app_id,
                    route_mode = route_mode.slug(),
                    target = %target,
                    compile_cache_hit = false,
                    compile_ms,
                    compile_cache_lookup_ms = cache_lookup_ms,
                    source_read_ms,
                    handler_html_ready_ms,
                    ssr_http_response_body_ms,
                    total_ms = elapsed_ms(app_started),
                    phase = "finish_compile_fallback",
                    "app page request finished with compile fallback"
                );
                return Ok(res);
            }
        }
    };
    let compile_cache_hit = compile_outcome.cache_hit;
    let compile_revision_scope = compile_outcome.revision_scope.clone();
    let compile_cache_validation = compile_outcome.cache_validation.clone();
    let compile_ms = compile_outcome.compile_ms;
    let compile_cache_lookup_ms = compile_outcome.cache_lookup_ms;
    let mut compiled = compile_outcome.compiled;
    if route_mode == UiRouteMode::Access {
        if access_path_scene.is_none() {
            let sid = compiled
                .active_scene
                .clone()
                .filter(|s| !s.trim().is_empty());
            if let Some(ref s) = sid {
                return Ok(Redirect::temporary(&access_canonical_location(
                    &app_id,
                    s,
                    query.tab.as_deref(),
                    query.chrome.as_deref(),
                ))
                .into_response());
            }
            let loc = format!("/apps/manage/{}", app_id.trim_start_matches('/'));
            return Ok(Redirect::temporary(&loc).into_response());
        }
        let requested = access_path_scene.as_ref().expect("access_path_scene");
        let rt = requested.trim();
        if let Some(route) = compiled.scene_routes.iter().find(|r| r.scene_id == rt) {
            if !route.access_export {
                let app_esc = html_escape_min(app_id.trim_start_matches('/'));
                let scene_esc = html_escape_min(rt);
                return Ok((
                    StatusCode::FORBIDDEN,
                    Html(format!(
                        "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>场景未导出</title></head><body>\
                         <p>场景 <code>{scene_esc}</code> 在应用 <code>{app_esc}</code> 中未开启 Access 导出（access_export=false）。</p>\
                         <p><a href=\"/apps/manage/{app_esc}\">返回管理态</a></p></body></html>",
                    )),
                )
                    .into_response());
            }
        }
        if compiled.active_scene.as_deref() != Some(rt) {
            let app_esc = html_escape_min(app_id.trim_start_matches('/'));
            let scene_esc = html_escape_min(rt);
            let manage_href_app = app_id.trim_start_matches('/');
            return Ok((
                StatusCode::NOT_FOUND,
                Html(format!(
                    "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>场景不存在</title></head><body>\
                     <p>应用 <code>{app_esc}</code> 中不存在场景 <code>{scene_esc}</code>（或无法绑定到当前编译结果）。</p>\
                     <p><a href=\"/apps/manage/{manage_href_app}\">返回管理态</a></p></body></html>",
                )),
            )
                .into_response());
        }
    }
    let manage_scene_resolved = if route_mode == UiRouteMode::Access {
        access_path_scene.clone()
    } else {
        canonical_scene_for_target(&compiled, manage_file.as_deref())
            .or_else(|| compiled.active_scene.clone())
            .or_else(|| {
                compiled.scene_contract.as_ref().and_then(|c| {
                    let id = c.scene.id.trim();
                    if id.is_empty() {
                        None
                    } else {
                        Some(id.to_string())
                    }
                })
            })
            .or_else(|| manage_scene_for_render(&compiled, query.scene.as_deref()))
    };
    let scene_for_default = manage_scene_resolved
        .as_deref()
        .or(compiled.active_scene.as_deref());
    let manage_default_file = default_file_for_scene(&compiled, scene_for_default);
    let target = if route_mode == UiRouteMode::Manage {
        manage_file.clone().unwrap_or(manage_default_file)
    } else {
        compiled.active_target_file.clone()
    };
    if compiled
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        let error_diagnostics = compiled
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect::<Vec<_>>();
        for diag in error_diagnostics.iter().take(20) {
            tracing::warn!(
                app_id = %app_id,
                route_mode = route_mode.slug(),
                target = %target,
                diagnostic_code = %diag.code,
                diagnostic_source = %diag.source_path.as_deref().unwrap_or("(unknown)"),
                diagnostic_message = %diag.message,
                phase = "compile_diagnostics",
                "compile completed with error diagnostic"
            );
        }
        let omitted_count = error_diagnostics.len().saturating_sub(20);
        tracing::warn!(
            app_id = %app_id,
            route_mode = route_mode.slug(),
            target = %target,
            error_diagnostic_count = error_diagnostics.len(),
            omitted_count,
            phase = "compile_diagnostics_summary",
            "compile completed with error diagnostics"
        );
    }
    let source_path = state.source_root.join(&app_id).join(&target);
    let source_started = Instant::now();
    let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
    let source_read_ms = elapsed_ms(source_started);
    let source_meta = source_panel_meta(&source_path, &source);
    let topbar_menus = load_segment_topbar_menus(&state.source_root);
    let render = |cc: &CompiledApp| {
        let t = Instant::now();
        let html = render_page(
            &apps,
            cc,
            &app_id,
            Some(&topbar_menus),
            route_mode,
            Some(target.as_str()),
            Some(source.as_str()),
            Some(&source_meta),
            manage_scene_resolved.as_deref(),
            normalized_preview_target.as_deref(),
            query.tab.as_deref(),
            query.diag_filter.as_deref(),
            chrome_hidden,
        );
        (html, elapsed_ms(t))
    };
    push_manage_page_pipeline_diag(
        &mut compiled,
        &app_id,
        target.as_str(),
        discover_ms,
        compile_ms,
        compile_cache_hit,
        compile_cache_lookup_ms,
        source_read_ms,
        0,
        0,
        0,
        None,
        None,
        None,
        elapsed_ms(app_started),
    );
    let (html, ssr_http_response_body_ms, handler_html_ready_ms) = {
        let t = Instant::now();
        let h = render(&compiled).0;
        let ssr_emit_ms = elapsed_ms(t);
        let total_wall = elapsed_ms(app_started);
        let h = fill_perf_placeholders(h, ssr_emit_ms, total_wall);
        let handler_ms = elapsed_ms(app_started);
        let h = fill_manage_wall_clock_placeholders(h, ssr_emit_ms, handler_ms);
        let h = fill_gis_tiles_placeholders(h, state.gis_tiles.as_ref());
        (h, ssr_emit_ms, handler_ms)
    };
    let mut res = Html(html).into_response();
    if let Ok(v) = HeaderValue::from_str(&handler_html_ready_ms.to_string()) {
        res.headers_mut()
            .insert(HeaderName::from_static("x-mei-handler-html-ready-ms"), v);
    }
    if let Ok(v) = HeaderValue::from_str(&ssr_http_response_body_ms.to_string()) {
        res.headers_mut().insert(
            HeaderName::from_static("x-mei-ssr-http-response-body-ms"),
            v,
        );
    }
    insert_manage_compile_observability_headers(&mut res, &compiled);
    let request_meta = CompileWithCacheOutcome {
        compiled: compiled.clone(),
        cache_hit: compile_cache_hit,
        revision_scope: compile_revision_scope.clone(),
        cache_validation: compile_cache_validation.clone(),
        cache_lookup_ms: compile_cache_lookup_ms,
        compile_cache_lock_wait_ms: 0,
        compile_ms,
    };
    insert_manage_compile_request_headers(&mut res, &request_meta);
    tracing::info!(
        app_id = %app_id,
        route_mode = route_mode.slug(),
        target = %target,
        compile_cache_hit,
        compile_ms,
        compile_cache_lookup_ms,
        source_read_ms,
        handler_html_ready_ms,
        ssr_http_response_body_ms,
        total_ms = elapsed_ms(app_started),
        phase = "finish",
        "app page request finished"
    );
    Ok(res)
}
