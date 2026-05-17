use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use mei_lang_app::{render_page, UiRouteMode};
use mei_lang_kernel::{discover_apps, read_source_file, CompileOptions, CompiledApp, Severity};

use crate::{AppError, AppState};

use super::super::compile_cache::{compile_app_with_cache, CompileWithCacheFailure};
use super::app_render::{choose_default_app, compile_error_fallback_app, source_panel_meta};
use super::components::resolve_components_root;
use super::menus::load_segment_topbar_menus;
use super::util::{
    elapsed_ms, fill_manage_wall_clock_placeholders, fill_perf_placeholders, is_script_target,
    push_manage_page_pipeline_diag,
};

/// 若 URL `entry` 在应用注册表中不存在（编译已回退并带 `unknown_entry` 警告），用 `compiled.active_entry` 生成管理壳链接，避免把无效 id 写进 href。
fn manage_entry_for_render(compiled: &CompiledApp, query_entry: Option<&str>) -> Option<String> {
    let q = query_entry?.trim();
    if q.is_empty() {
        return None;
    }
    if compiled.entries.iter().any(|e| e.entry_id == q) {
        return Some(q.to_string());
    }
    compiled.active_entry.clone()
}

#[derive(Debug, serde::Deserialize)]
pub struct AppQuery {
    pub target: Option<String>,
    pub entry: Option<String>,
    pub tab: Option<String>,
    pub chrome: Option<String>,
}

pub async fn index(State(state): State<AppState>) -> Result<Redirect, AppError> {
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let first = choose_default_app(&state.source_root, &apps).or_else(|| apps.first());
    let first = first.ok_or_else(|| {
        AppError::msg(format!(
            "source root has no discoverable apps (need at least one first-level subdirectory under `{}` containing `main.mei`; root-level `main.mei` is ignored)",
            state.source_root.display()
        ))
    })?;
    Ok(Redirect::to(&format!("/apps/manage/{}", first.id)))
}

pub async fn app_page(
    State(state): State<AppState>,
    AxumPath((mode, app_id_raw)): AxumPath<(String, String)>,
    Query(query): Query<AppQuery>,
) -> Result<Response, AppError> {
    let app_started = Instant::now();
    let app_id = app_id_raw.trim_start_matches('/').to_string();
    if app_id.is_empty() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            "missing app id in route",
        ));
    }
    let discover_started = Instant::now();
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let discover_ms = elapsed_ms(discover_started);
    let route_mode = UiRouteMode::from_slug(&mode);
    let chrome_hidden = query
        .chrome
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("none"))
        .unwrap_or(false);
    let normalized_preview_target = query
        .target
        .as_deref()
        .filter(|target| is_script_target(target))
        .map(ToString::to_string);
    let components_root = resolve_components_root(&state.source_root);
    let compile_options = CompileOptions {
        entry: query.entry.clone(),
        preview_target: normalized_preview_target.clone(),
    };
    let compile_outcome =
        match compile_app_with_cache(&state, &app_id, compile_options, components_root.as_path()) {
            Ok(outcome) => outcome,
            Err(failure) => {
                let CompileWithCacheFailure {
                    error,
                    cache_lookup_ms,
                    compile_ms,
                } = failure;
                tracing::warn!(app_id = %app_id, %error, "failed to compile app page");
                let target = query
                    .target
                    .clone()
                    .unwrap_or_else(|| "main.mei".to_string());
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
                let manage_entry_resolved =
                    manage_entry_for_render(&compiled, query.entry.as_deref());
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
                        manage_entry_resolved.as_deref(),
                        normalized_preview_target.as_deref(),
                        query.tab.as_deref(),
                        chrome_hidden,
                    );
                    (html, elapsed_ms(t))
                };
                let (_, ssr_baseline_ms) = render(&compiled);
                push_manage_page_pipeline_diag(
                    &mut compiled,
                    &app_id,
                    target.as_str(),
                    discover_ms,
                    compile_ms,
                    false,
                    cache_lookup_ms,
                    source_read_ms,
                    ssr_baseline_ms,
                    0,
                    0,
                    None,
                    None,
                    None,
                    elapsed_ms(app_started),
                );
                let (_, ssr_publish_ms) = render(&compiled);
                compiled
                    .diagnostics
                    .retain(|d| d.code != "manage_page_pipeline");
                push_manage_page_pipeline_diag(
                    &mut compiled,
                    &app_id,
                    target.as_str(),
                    discover_ms,
                    compile_ms,
                    false,
                    cache_lookup_ms,
                    source_read_ms,
                    ssr_baseline_ms,
                    ssr_publish_ms,
                    0,
                    None,
                    None,
                    None,
                    elapsed_ms(app_started),
                );
                let (_, ssr_final_emit_ms) = render(&compiled);
                compiled
                    .diagnostics
                    .retain(|d| d.code != "manage_page_pipeline");
                let total_ms = elapsed_ms(app_started);
                push_manage_page_pipeline_diag(
                    &mut compiled,
                    &app_id,
                    target.as_str(),
                    discover_ms,
                    compile_ms,
                    false,
                    cache_lookup_ms,
                    source_read_ms,
                    ssr_baseline_ms,
                    ssr_publish_ms,
                    ssr_final_emit_ms,
                    None,
                    None,
                    None,
                    total_ms,
                );
                let (_probe_response_html, ssr_response_probe_ms) = render(&compiled);
                compiled
                    .diagnostics
                    .retain(|d| d.code != "manage_page_pipeline");
                push_manage_page_pipeline_diag(
                    &mut compiled,
                    &app_id,
                    target.as_str(),
                    discover_ms,
                    compile_ms,
                    false,
                    cache_lookup_ms,
                    source_read_ms,
                    ssr_baseline_ms,
                    ssr_publish_ms,
                    ssr_final_emit_ms,
                    Some(ssr_response_probe_ms),
                    None,
                    None,
                    elapsed_ms(app_started),
                );
                let (_html_serve_pass, ssr_serve_ms) = render(&compiled);
                let wall_after_serve = elapsed_ms(app_started);
                compiled
                    .diagnostics
                    .retain(|d| d.code != "manage_page_pipeline");
                push_manage_page_pipeline_diag(
                    &mut compiled,
                    &app_id,
                    target.as_str(),
                    discover_ms,
                    compile_ms,
                    false,
                    cache_lookup_ms,
                    source_read_ms,
                    ssr_baseline_ms,
                    ssr_publish_ms,
                    ssr_final_emit_ms,
                    Some(ssr_response_probe_ms),
                    Some(ssr_serve_ms),
                    None,
                    wall_after_serve,
                );
                let (_html, ssr_emit_ms) = render(&compiled);
                let total_ms = elapsed_ms(app_started);
                compiled
                    .diagnostics
                    .retain(|d| d.code != "manage_page_pipeline");
                push_manage_page_pipeline_diag(
                    &mut compiled,
                    &app_id,
                    target.as_str(),
                    discover_ms,
                    compile_ms,
                    false,
                    cache_lookup_ms,
                    source_read_ms,
                    ssr_baseline_ms,
                    ssr_publish_ms,
                    ssr_final_emit_ms,
                    Some(ssr_response_probe_ms),
                    Some(ssr_serve_ms),
                    Some(ssr_emit_ms),
                    total_ms,
                );
                let (html, ssr_http_response_body_ms, handler_html_ready_ms) = {
                    let t = Instant::now();
                    let h = render(&compiled).0;
                    let last_pass_ms = elapsed_ms(t);
                    let total_wall = elapsed_ms(app_started);
                    let h = fill_perf_placeholders(
                        h,
                        ssr_baseline_ms
                            .saturating_add(ssr_publish_ms)
                            .saturating_add(ssr_final_emit_ms)
                            .saturating_add(ssr_response_probe_ms)
                            .saturating_add(ssr_serve_ms)
                            .saturating_add(ssr_emit_ms)
                            .saturating_add(last_pass_ms),
                        total_wall,
                    );
                    let handler_ms = elapsed_ms(app_started);
                    let h = fill_manage_wall_clock_placeholders(h, last_pass_ms, handler_ms);
                    (h, last_pass_ms, handler_ms)
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
                }
                return Ok(res);
            }
        };
    let mut compiled = compile_outcome.compiled;
    let compile_ms = compile_outcome.compile_ms;
    let compile_cache_hit = compile_outcome.cache_hit;
    let compile_cache_lookup_ms = compile_outcome.cache_lookup_ms;
    let target = query
        .target
        .unwrap_or_else(|| compiled.entry_target.clone());
    if compiled
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        let mut lines = Vec::new();
        for d in compiled
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
        {
            let path = d.source_path.as_deref().unwrap_or("(unknown)");
            lines.push(format!("{} [{}] {}", path, d.code, d.message));
        }
        tracing::warn!(
            app_id = %app_id,
            target = %target,
            error_diagnostics = %lines.join(" | "),
            "compile completed with error diagnostics"
        );
    }
    let source_path = state.source_root.join(&app_id).join(&target);
    let source_started = Instant::now();
    let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
    let source_read_ms = elapsed_ms(source_started);
    let source_meta = source_panel_meta(&source_path, &source);
    let topbar_menus = load_segment_topbar_menus(&state.source_root);
    let manage_entry_resolved = manage_entry_for_render(&compiled, query.entry.as_deref());
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
            manage_entry_resolved.as_deref(),
            normalized_preview_target.as_deref(),
            query.tab.as_deref(),
            chrome_hidden,
        );
        (html, elapsed_ms(t))
    };
    let (_, ssr_baseline_ms) = render(&compiled);
    push_manage_page_pipeline_diag(
        &mut compiled,
        &app_id,
        target.as_str(),
        discover_ms,
        compile_ms,
        compile_cache_hit,
        compile_cache_lookup_ms,
        source_read_ms,
        ssr_baseline_ms,
        0,
        0,
        None,
        None,
        None,
        elapsed_ms(app_started),
    );
    let (_, ssr_publish_ms) = render(&compiled);
    compiled
        .diagnostics
        .retain(|d| d.code != "manage_page_pipeline");
    push_manage_page_pipeline_diag(
        &mut compiled,
        &app_id,
        target.as_str(),
        discover_ms,
        compile_ms,
        compile_cache_hit,
        compile_cache_lookup_ms,
        source_read_ms,
        ssr_baseline_ms,
        ssr_publish_ms,
        0,
        None,
        None,
        None,
        elapsed_ms(app_started),
    );
    let (_, ssr_final_emit_ms) = render(&compiled);
    compiled
        .diagnostics
        .retain(|d| d.code != "manage_page_pipeline");
    let total_ms = elapsed_ms(app_started);
    push_manage_page_pipeline_diag(
        &mut compiled,
        &app_id,
        target.as_str(),
        discover_ms,
        compile_ms,
        compile_cache_hit,
        compile_cache_lookup_ms,
        source_read_ms,
        ssr_baseline_ms,
        ssr_publish_ms,
        ssr_final_emit_ms,
        None,
        None,
        None,
        total_ms,
    );
    let (_probe_response_html, ssr_response_probe_ms) = render(&compiled);
    compiled
        .diagnostics
        .retain(|d| d.code != "manage_page_pipeline");
    push_manage_page_pipeline_diag(
        &mut compiled,
        &app_id,
        target.as_str(),
        discover_ms,
        compile_ms,
        compile_cache_hit,
        compile_cache_lookup_ms,
        source_read_ms,
        ssr_baseline_ms,
        ssr_publish_ms,
        ssr_final_emit_ms,
        Some(ssr_response_probe_ms),
        None,
        None,
        elapsed_ms(app_started),
    );
    let (_html_serve_pass, ssr_serve_ms) = render(&compiled);
    let wall_after_serve = elapsed_ms(app_started);
    compiled
        .diagnostics
        .retain(|d| d.code != "manage_page_pipeline");
    push_manage_page_pipeline_diag(
        &mut compiled,
        &app_id,
        target.as_str(),
        discover_ms,
        compile_ms,
        compile_cache_hit,
        compile_cache_lookup_ms,
        source_read_ms,
        ssr_baseline_ms,
        ssr_publish_ms,
        ssr_final_emit_ms,
        Some(ssr_response_probe_ms),
        Some(ssr_serve_ms),
        None,
        wall_after_serve,
    );
    let (_html, ssr_emit_ms) = render(&compiled);
    let total_ms = elapsed_ms(app_started);
    compiled
        .diagnostics
        .retain(|d| d.code != "manage_page_pipeline");
    push_manage_page_pipeline_diag(
        &mut compiled,
        &app_id,
        target.as_str(),
        discover_ms,
        compile_ms,
        compile_cache_hit,
        compile_cache_lookup_ms,
        source_read_ms,
        ssr_baseline_ms,
        ssr_publish_ms,
        ssr_final_emit_ms,
        Some(ssr_response_probe_ms),
        Some(ssr_serve_ms),
        Some(ssr_emit_ms),
        total_ms,
    );
    let (html, ssr_http_response_body_ms, handler_html_ready_ms) = {
        let t = Instant::now();
        let h = render(&compiled).0;
        let last_pass_ms = elapsed_ms(t);
        let total_wall = elapsed_ms(app_started);
        let h = fill_perf_placeholders(
            h,
            ssr_baseline_ms
                .saturating_add(ssr_publish_ms)
                .saturating_add(ssr_final_emit_ms)
                .saturating_add(ssr_response_probe_ms)
                .saturating_add(ssr_serve_ms)
                .saturating_add(ssr_emit_ms)
                .saturating_add(last_pass_ms),
            total_wall,
        );
        let handler_ms = elapsed_ms(app_started);
        let h = fill_manage_wall_clock_placeholders(h, last_pass_ms, handler_ms);
        (h, last_pass_ms, handler_ms)
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
    }
    Ok(res)
}
