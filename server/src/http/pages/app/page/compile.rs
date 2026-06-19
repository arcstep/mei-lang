use std::path::PathBuf;
use std::time::Instant;

use axum::{
    http::{HeaderName, HeaderValue},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use mei_lang_app::{render_page, HostAccountView, TopbarMenuContext, UiRouteMode, UploadFileEntry};
use mei_lang_kernel::{read_source_file, resolve_app_root, CompileOptions, WorkspaceAppMeta};

use crate::AppState;

use crate::http::compile_cache::{
    access_artifact_only_mode_enabled, compile_app_with_cache, is_compile_inflight,
    load_compile_artifact_only, peek_compile_cache_hit, recent_compile_failure,
    start_compile_in_background_if_needed, CompileWithCacheFailure, CompileWithCacheOutcome,
};
use crate::http::host_error_page::{self, HostShellAction};
use crate::http::pages::app::compiling_shell::{
    compile_bootstrap_disabled_for_request, compile_bootstrap_enabled,
    compile_bootstrap_probe_requested, compile_bootstrap_route_supported,
    render_compiling_shell,
};
use crate::http::pages::app::page_render::{
    access_only_surface_enabled, insert_manage_compile_observability_headers,
};
use crate::http::pages::app::query::AppQuery;
use crate::http::pages::app::scene::manage_scene_for_render;
use crate::http::pages::app_render::{compile_error_fallback_app, source_panel_meta};
use crate::http::pages::util::{
    elapsed_ms, fill_manage_wall_clock_placeholders, fill_page_shell_placeholders,
    fill_perf_placeholders, measure_page_html_payload, push_manage_page_pipeline_diag,
};

pub(super) enum CompileResolution {
    Outcome(CompileWithCacheOutcome),
    EarlyResponse(Response),
}

pub(super) fn maybe_handle_compile_bootstrap_probe(
    state: &AppState,
    route_mode: UiRouteMode,
    app_id: &str,
    query: &AppQuery,
    compile_options: &CompileOptions,
    components_root: &std::path::Path,
    access_path_scene: Option<&str>,
) -> Option<Response> {
    if !compile_bootstrap_route_supported(route_mode) {
        return None;
    }
    if !compile_bootstrap_probe_requested(query) {
        return None;
    }
    if compile_bootstrap_disabled_for_request(query) || !compile_bootstrap_enabled() {
        return Some(compile_bootstrap_probe_response(true, "bootstrap_disabled"));
    }
    if recent_compile_failure(state, app_id, compile_options) {
        return Some(compile_bootstrap_probe_response(true, "recent_compile_failure"));
    }
    if peek_compile_cache_hit(state, app_id, compile_options, components_root).is_some() {
        return Some(compile_bootstrap_probe_response(true, "cache_hit"));
    }
    if is_compile_inflight(state, app_id, compile_options) {
        return Some(compile_bootstrap_probe_response(false, "compile_inflight"));
    }
    start_compile_in_background_if_needed(
        state.clone(),
        app_id.to_string(),
        compile_options.clone(),
        components_root.to_path_buf(),
    );
    let scene_hint = compile_options.scene.as_deref().or(access_path_scene);
    tracing::info!(
        app_id = %app_id,
        route_mode = route_mode.slug(),
        phase = "compile_bootstrap_probe_start_background",
        scene_hint = %scene_hint.unwrap_or("-"),
        "bootstrap probe started background compile"
    );
    Some(compile_bootstrap_probe_response(false, "compile_started"))
}

fn compile_bootstrap_probe_response(ready: bool, reason: &str) -> Response {
    let mut response = if ready {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::ACCEPTED.into_response()
    };
    if let Ok(value) = HeaderValue::from_str(if ready { "1" } else { "0" }) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-compile-bootstrap-ready"),
            value,
        );
    }
    if let Ok(value) = HeaderValue::from_str(reason) {
        response.headers_mut().insert(
            HeaderName::from_static("x-mei-compile-bootstrap-reason"),
            value,
        );
    }
    response
}

pub(super) fn resolve_compile_outcome(
    state: &AppState,
    route_mode: UiRouteMode,
    app_id: &str,
    query: &AppQuery,
    compile_options: CompileOptions,
    components_root: PathBuf,
    access_path_scene: Option<&str>,
    manage_file: Option<&str>,
    apps: &[WorkspaceAppMeta],
    topbar_menus: &TopbarMenuContext,
    normalized_preview_target: Option<&str>,
    chrome_hidden: bool,
    upload_enabled: bool,
    upload_root_label: &str,
    upload_files: &[UploadFileEntry],
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    discover_ms: u64,
    app_started: Instant,
) -> CompileResolution {
    if route_mode.is_access_like() {
        if let Some(outcome) = load_compile_artifact_only(
            state,
            app_id,
            &compile_options,
            components_root.as_path(),
        ) {
            return CompileResolution::Outcome(outcome);
        }
    }
    if route_mode.is_access_like()
        && access_only_surface_enabled()
        && access_artifact_only_mode_enabled()
    {
        return match load_compile_artifact_only(
            state,
            app_id,
            &compile_options,
            components_root.as_path(),
        ) {
            Some(outcome) => CompileResolution::Outcome(outcome),
            None => CompileResolution::EarlyResponse(
                render_access_artifact_unavailable(
                    route_mode,
                    app_id,
                    compile_options.scene.as_deref().or(access_path_scene),
                    manage_file,
                ),
            ),
        };
    }
    if compile_bootstrap_enabled()
        && compile_bootstrap_route_supported(route_mode)
        && !compile_bootstrap_disabled_for_request(query)
        && !recent_compile_failure(state, app_id, &compile_options)
    {
        let peek_started = Instant::now();
        match peek_compile_cache_hit(state, app_id, &compile_options, components_root.as_path()) {
            Some(hit) => {
                return CompileResolution::Outcome(CompileWithCacheOutcome {
                    compiled: hit.compiled,
                    cache_hit: true,
                    artifact_cache_hit: false,
                    compile_revision: hit.compile_revision,
                    revision_scope: hit.revision_scope,
                    cache_validation: hit.cache_validation,
                    cache_lookup_ms: elapsed_ms(peek_started),
                    artifact_load_ms: 0,
                    compile_cache_lock_wait_ms: 0,
                    compile_ms: 0,
                });
            }
            None => {
                if is_compile_inflight(state, app_id, &compile_options) {
                    tracing::info!(
                        app_id = %app_id,
                        route_mode = route_mode.slug(),
                        phase = "compile_bootstrap_wait_inflight",
                        "compile inflight detected; waiting for singleflight result"
                    );
                    return match compile_app_with_cache(
                        state,
                        app_id,
                        compile_options.clone(),
                        components_root.as_path(),
                    ) {
                        Ok(outcome) => CompileResolution::Outcome(outcome),
                        Err(failure) => CompileResolution::EarlyResponse(render_compile_failure(
                            failure,
                            state,
                            route_mode,
                            app_id,
                            query,
                            manage_file,
                            apps,
                            topbar_menus,
                            normalized_preview_target,
                            chrome_hidden,
                            upload_enabled,
                            upload_root_label,
                            upload_files,
                            auth_enabled,
                            account_view,
                            discover_ms,
                            app_started,
                        )),
                    };
                }
                start_compile_in_background_if_needed(
                    state.clone(),
                    app_id.to_string(),
                    compile_options.clone(),
                    components_root.clone(),
                );
                let scene_hint = compile_options.scene.as_deref().or(access_path_scene);
                let shell = render_compiling_shell(route_mode, app_id, scene_hint);
                tracing::info!(
                    app_id = %app_id,
                    route_mode = route_mode.slug(),
                    phase = "compile_bootstrap_shell",
                    "serving compile bootstrap shell while compile runs in background"
                );
                return CompileResolution::EarlyResponse(Html(shell).into_response());
            }
        }
    }
    match compile_app_with_cache(state, app_id, compile_options, components_root.as_path()) {
        Ok(outcome) => CompileResolution::Outcome(outcome),
        Err(failure) => CompileResolution::EarlyResponse(render_compile_failure(
            failure,
            state,
            route_mode,
            app_id,
            query,
            manage_file,
            apps,
            topbar_menus,
            normalized_preview_target,
            chrome_hidden,
            upload_enabled,
            upload_root_label,
            upload_files,
            auth_enabled,
            account_view,
            discover_ms,
            app_started,
        )),
    }
}

fn render_access_artifact_unavailable(
    route_mode: UiRouteMode,
    app_id: &str,
    scene_hint: Option<&str>,
    manage_file: Option<&str>,
) -> Response {
    let mut actions = vec![HostShellAction {
        href: "/".to_string(),
        label: "返回首页".to_string(),
        primary: true,
    }];
    if let Some(scene_id) = scene_hint.map(str::trim).filter(|value| !value.is_empty()) {
        actions.insert(
            0,
            HostShellAction {
                href: format!("/apps/app/{app_id}/scene/{scene_id}?chrome=none"),
                label: "重试当前场景".to_string(),
                primary: false,
            },
        );
    } else if let Some(target) = manage_file.map(str::trim).filter(|value| !value.is_empty()) {
        actions.insert(
            0,
            HostShellAction {
                href: format!("/apps/build/{app_id}?file={target}"),
                label: "打开构建视图".to_string(),
                primary: false,
            },
        );
    }
    let detail = scene_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|scene_id| format!("mode={} app={app_id} scene={scene_id}", route_mode.slug()))
        .unwrap_or_else(|| format!("mode={} app={app_id}", route_mode.slug()));
    let html = host_error_page::render_error_page(
        StatusCode::SERVICE_UNAVAILABLE,
        "访问态产物尚未就绪",
        "当前 access-only 宿主已切到 artifact-first 主路径，请先等待启动预热完成，或预先构建访问产物后再提供访问流量。",
        Some(detail.as_str()),
        &actions,
    );
    let mut response = Html(html).into_response();
    *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
    response.headers_mut().insert(
        HeaderName::from_static("retry-after"),
        HeaderValue::from_static("3"),
    );
    response
}

fn render_compile_failure(
    failure: CompileWithCacheFailure,
    state: &AppState,
    route_mode: UiRouteMode,
    app_id: &str,
    query: &AppQuery,
    manage_file: Option<&str>,
    apps: &[WorkspaceAppMeta],
    topbar_menus: &TopbarMenuContext,
    normalized_preview_target: Option<&str>,
    chrome_hidden: bool,
    upload_enabled: bool,
    upload_root_label: &str,
    upload_files: &[UploadFileEntry],
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    discover_ms: u64,
    app_started: Instant,
) -> Response {
    let CompileWithCacheFailure {
        error,
        revision_scope,
        cache_validation,
        cache_lookup_ms,
        compile_cache_lock_wait_ms: _,
        compile_ms,
    } = failure;
    tracing::warn!(
        app_id = %app_id,
        %error,
        revision_scope,
        cache_validation,
        cache_lookup_ms,
        compile_ms,
        "failed to compile app page"
    );
    let target = if route_mode == UiRouteMode::Build {
        manage_file
            .map(ToString::to_string)
            .unwrap_or_else(|| "main.mei".to_string())
    } else {
        "main.mei".to_string()
    };
    let source_path = resolve_app_root(state.source_root.as_path(), app_id).join(&target);
    let source_started = Instant::now();
    let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
    let source_read_ms = elapsed_ms(source_started);
    let source_meta = source_panel_meta(&source_path, &source);
    let mut compiled = compile_error_fallback_app(
        &state.source_root,
        app_id,
        target.as_str(),
        error.to_string().as_str(),
    );
    let manage_scene_resolved = manage_scene_for_render(&compiled, query.scene.as_deref());
    push_manage_page_pipeline_diag(
        &mut compiled,
        app_id,
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
        let h = render_page(
            apps,
            &compiled,
            app_id,
            Some(topbar_menus),
            route_mode,
            Some(target.as_str()),
            Some(source.as_str()),
            Some(&source_meta),
            manage_scene_resolved.as_deref(),
            normalized_preview_target,
            query.tab.as_deref(),
            query.diag_filter.as_deref(),
            query.world_metric.as_deref(),
            query.world_dataset.as_deref(),
            query.explain.as_deref(),
            chrome_hidden,
            upload_enabled,
            Some(upload_root_label),
            upload_files,
            auth_enabled,
            account_view,
            None,
        );
        let ssr_emit_ms = elapsed_ms(t);
        let total_wall = elapsed_ms(app_started);
        let h = fill_perf_placeholders(h, ssr_emit_ms, total_wall);
        let handler_ms = elapsed_ms(app_started);
        let h = fill_manage_wall_clock_placeholders(h, ssr_emit_ms, handler_ms);
        let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
            &resolve_app_root(state.source_root.as_path(), app_id),
            Some(state.source_root.as_path()),
            None,
        );
        let h = fill_page_shell_placeholders(h, &gis, state.source_root.as_path());
        (h, ssr_emit_ms, handler_ms)
    };
    let payload_stats = measure_page_html_payload(&html);
    let mut res = Html(html).into_response();
    if matches!(route_mode, UiRouteMode::Build) {
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
        if let Ok(v) = HeaderValue::from_str(&payload_stats.html_bytes.to_string()) {
            res.headers_mut()
                .insert(HeaderName::from_static("x-mei-html-bytes"), v);
        }
        if let Ok(v) = HeaderValue::from_str(&payload_stats.data_props_bytes.to_string()) {
            res.headers_mut()
                .insert(HeaderName::from_static("x-mei-data-props-bytes"), v);
        }
        if let Ok(v) = HeaderValue::from_str(&payload_stats.data_props_count.to_string()) {
            res.headers_mut()
                .insert(HeaderName::from_static("x-mei-data-props-count"), v);
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
        html_bytes = payload_stats.html_bytes,
        data_props_count = payload_stats.data_props_count,
        data_props_bytes = payload_stats.data_props_bytes,
        data_props_max_bytes = payload_stats.data_props_max_bytes,
        scene_drilldown_context_bytes = payload_stats.scene_drilldown_context_bytes,
        total_ms = elapsed_ms(app_started),
        phase = "finish_compile_fallback",
        "app page request finished with compile fallback"
    );
    res
}
