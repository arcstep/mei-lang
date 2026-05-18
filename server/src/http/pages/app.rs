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

/// 若 URL `scene` 在应用路由表中不存在（编译已回退并带 `unknown_scene` 警告），用 `compiled.active_scene` 生成管理壳链接，避免把无效 id 写进 href。
fn manage_scene_for_render(compiled: &CompiledApp, query_scene: Option<&str>) -> Option<String> {
    let q = query_scene?.trim();
    if q.is_empty() {
        return None;
    }
    if compiled
        .scene_routes
        .iter()
        .any(|r| r.scene_id == q)
    {
        return Some(q.to_string());
    }
    compiled.active_scene.clone()
}

/// 给定已解析的 scene id（或 `None`），返回该场景路由对应的主文件路径；无匹配时回退 `active_target_file`。
fn default_file_for_scene(compiled: &CompiledApp, scene_id: Option<&str>) -> String {
    let sid = scene_id.unwrap_or("").trim();
    if sid.is_empty() {
        return compiled.active_target_file.clone();
    }
    compiled
        .scene_routes
        .iter()
        .find(|r| r.scene_id == sid)
        .map(|r| r.target_file.clone())
        .unwrap_or_else(|| compiled.active_target_file.clone())
}

/// 若目标文件本身就是某条 scene route 的主文件，则返回该 route 的 scene id。
fn canonical_scene_for_target(compiled: &CompiledApp, target_file: Option<&str>) -> Option<String> {
    let target_file = target_file?.trim();
    if target_file.is_empty() {
        return None;
    }
    compiled
        .scene_routes
        .iter()
        .find(|r| r.target_file == target_file)
        .map(|r| r.scene_id.clone())
}

#[derive(Debug, serde::Deserialize)]
pub struct AppQuery {
    /// 仅管理态：当前打开的源码/资源路径（相对 app 根）。兼容旧链接 `target=`。
    /// 访问态禁止携带：若出现则 307 重定向到剥离 `file`/`target` 后的 URL（发布面不得深链内部路径）。
    #[serde(default, alias = "target")]
    pub file: Option<String>,
    pub scene: Option<String>,
    pub tab: Option<String>,
    pub chrome: Option<String>,
}

fn percent_encode_query_component(value: &str) -> String {
    let mut out = String::new();
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*b));
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

/// 访问态允许的 query：`scene`、`tab`、`chrome`（不含 `file`/`target`）。
fn access_sanitized_redirect_location(app_id: &str, query: &AppQuery) -> String {
    let mut parts = Vec::new();
    if let Some(scene) = query.scene.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("scene={}", percent_encode_query_component(scene)));
    }
    if let Some(tab) = query.tab.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("tab={}", percent_encode_query_component(tab)));
    }
    if let Some(chrome) = query.chrome.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("chrome={}", percent_encode_query_component(chrome)));
    }
    if parts.is_empty() {
        format!("/apps/access/{app_id}")
    } else {
        format!("/apps/access/{app_id}?{}", parts.join("&"))
    }
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
    let route_mode = UiRouteMode::from_slug(&mode);
    if route_mode == UiRouteMode::Access
        && query
            .file
            .as_ref()
            .map(|f| !f.trim().is_empty())
            .unwrap_or(false)
    {
        return Ok(Redirect::temporary(&access_sanitized_redirect_location(&app_id, &query))
            .into_response());
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
    let compile_scene = if route_mode == UiRouteMode::Manage && manage_script_file.is_some() {
        None
    } else {
        query.scene.clone()
    };
    let components_root = resolve_components_root(&state.source_root);
    let compile_options = CompileOptions {
        scene: compile_scene,
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
    let manage_scene_resolved = canonical_scene_for_target(&compiled, manage_file.as_deref())
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
        .or_else(|| manage_scene_for_render(&compiled, query.scene.as_deref()));
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
