use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use mei_lang_app::{
    render_build_source_page, render_config_page, render_page, render_upload_page,
    SourcePanelMeta, TopbarMenuContext, UiRouteMode, UploadFileEntry,
};
use mei_lang_kernel::{
    discover_apps, load_mei_config_for_app, read_source_file, resolve_app_entry_main,
    resolve_default_scene_from_root, source_tree, CompileOptions, CompiledApp, Severity,
    WorkspaceAppMeta,
};
use serde::Serialize;
use serde_json::json;
use std::fs;

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
    access_canonical_location, access_sanitized_redirect_location,
    legacy_access_redirect_location, legacy_manage_redirect_location, parse_access_scene_path,
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

fn insert_page_render_cache_hit_header(res: &mut Response, cache_hit: bool) {
    let value = if cache_hit { "1" } else { "0" };
    if let Ok(header_value) = HeaderValue::from_str(value) {
        res.headers_mut().insert(
            HeaderName::from_static("x-mei-page-render-cache-hit"),
            header_value,
        );
    }
}

#[derive(Debug, Clone)]
struct CachedPageRenderTemplate {
    expires_at: Instant,
    html: String,
}

const PAGE_RENDER_CACHE_TTL_MS: u64 = 300_000;
const MAX_PAGE_RENDER_CACHE_ENTRIES: usize = 128;

fn page_render_cache() -> &'static Mutex<BTreeMap<String, CachedPageRenderTemplate>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, CachedPageRenderTemplate>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn page_render_cache_ttl() -> Duration {
    Duration::from_millis(PAGE_RENDER_CACHE_TTL_MS)
}

fn take_cached_page_render_template(key: &str) -> Option<String> {
    let Ok(mut cache) = page_render_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    cache.get(key).map(|entry| entry.html.clone())
}

fn store_cached_page_render_template(key: String, html: &str) {
    let Ok(mut cache) = page_render_cache().lock() else {
        return;
    };
    cache.retain(|_, entry| entry.expires_at > Instant::now());
    if cache.len() >= MAX_PAGE_RENDER_CACHE_ENTRIES {
        cache.clear();
    }
    cache.insert(
        key,
        CachedPageRenderTemplate {
            expires_at: Instant::now() + page_render_cache_ttl(),
            html: html.to_string(),
        },
    );
}

fn hash_signature(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn serialized_signature<T: Serialize + ?Sized>(value: &T) -> u64 {
    serde_json::to_string(value)
        .map(|raw| hash_signature(&raw))
        .unwrap_or(0)
}

fn page_render_cache_key(
    app_id: &str,
    route_mode: UiRouteMode,
    compile_revision: &str,
    target: &str,
    source: &str,
    source_meta: Option<&SourcePanelMeta>,
    selected_scene: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
    diag_filter: Option<&str>,
    chrome_hidden: bool,
    upload_enabled: bool,
    upload_root_label: Option<&str>,
    topbar_menu: Option<&TopbarMenuContext>,
    upload_files: &[UploadFileEntry],
    gis: &crate::gis_config::GisTilesConfig,
) -> Option<String> {
    let compile_revision = compile_revision.trim();
    if compile_revision.is_empty() {
        return None;
    }
    let source_sig = hash_signature(source);
    let source_meta_sig = source_meta.map(serialized_signature).unwrap_or(0);
    let topbar_sig = topbar_menu.map(serialized_signature).unwrap_or(0);
    let upload_sig = serialized_signature(upload_files);
    let extra = json!({
        "app_id": app_id,
        "route_mode": route_mode.slug(),
        "compile_revision": compile_revision,
        "target": target,
        "selected_scene": selected_scene.unwrap_or(""),
        "preview_target": preview_target.unwrap_or(""),
        "active_tab": active_tab.unwrap_or(""),
        "diag_filter": diag_filter.unwrap_or(""),
        "chrome_hidden": chrome_hidden,
        "upload_enabled": upload_enabled,
        "upload_root_label": upload_root_label.unwrap_or(""),
        "source_sig": source_sig,
        "source_meta_sig": source_meta_sig,
        "topbar_sig": topbar_sig,
        "upload_sig": upload_sig,
        "gis_base_url": gis.base_url.as_str(),
        "gis_json_path": gis.json_path.as_str(),
    });
    serde_json::to_string(&extra).ok()
}

fn render_page_template_with_cache(
    cache_key: Option<String>,
    render: impl FnOnce() -> String,
) -> (String, bool) {
    if let Some(ref key) = cache_key {
        if let Some(html) = take_cached_page_render_template(key) {
            return (html, true);
        }
    }
    let html = render();
    if let Some(key) = cache_key {
        store_cached_page_render_template(key, &html);
    }
    (html, false)
}

fn upload_rel_from_config(app_root: &Path, source_root: &Path) -> Option<String> {
    let config = load_mei_config_for_app(app_root, Some(source_root));
    config
        .paths
        .upload
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\\', "/"))
}

fn list_upload_files(upload_root: &Path, _upload_rel: &str) -> Vec<UploadFileEntry> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(upload_root) else {
        return out;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        out.push(UploadFileEntry {
            path: name.clone(),
            name,
            is_dir: file_type.is_dir(),
        });
    }
    out.sort_by(|left, right| left.name.cmp(&right.name));
    out
}

fn app_title_for(apps: &[WorkspaceAppMeta], app_id: &str) -> String {
    apps.iter()
        .find(|app| app.id == app_id)
        .map(|app| app.title.clone())
        .unwrap_or_else(|| app_id.to_string())
}

fn lightweight_access_scene(
    app_root: &Path,
    query_scene: Option<&str>,
) -> Option<String> {
    query_scene
        .map(str::trim)
        .filter(|scene| !scene.is_empty())
        .map(str::to_string)
        .or_else(|| resolve_default_scene_from_root(app_root).ok().flatten())
}

pub async fn app_page(
    State(state): State<AppState>,
    AxumPath((mode, app_id_raw)): AxumPath<(String, String)>,
    Query(query): Query<AppQuery>,
) -> Result<Response, AppError> {
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
                Html(
                    "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>场景路径无效</title></head><body><p>地址中的 <code>/scene/&lt;id&gt;</code> 无效。</p></body></html>".to_string(),
                ),
            )
                .into_response());
        }
    };
    let access_path_scene = if route_mode == UiRouteMode::App {
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
    let app_root = state.source_root.join(&app_id);
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
    if route_mode == UiRouteMode::Build {
        if request_file
            .as_deref()
            .map(str::trim)
            .is_some_and(|file| file == ".mei-config.json")
        {
            return Ok(Redirect::temporary(&format!("/apps/config/{app_id}")).into_response());
        }
    }
    if route_mode == UiRouteMode::App {
        if let Some(ref file) = request_file {
            if is_script_target(file) {
                return Ok(Redirect::temporary(&access_sanitized_redirect_location(
                    &app_id, &query,
                ))
                .into_response());
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
    if route_mode == UiRouteMode::App && access_static_file.is_none() {
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
    let app_title = app_title_for(&apps, &app_id);
    let chrome_hidden = query
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
    let upload_root_label = upload_rel.as_deref().unwrap_or("upload");
    let lightweight_scene = lightweight_access_scene(&app_root, query.scene.as_deref());
    let manage_file = if route_mode == UiRouteMode::Build {
        request_file.clone()
    } else {
        None
    };
    if route_mode == UiRouteMode::Config {
        let target = ".mei-config.json".to_string();
        let source_path = app_root.join(&target);
        let source_started = Instant::now();
        let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
        let source_read_ms = elapsed_ms(source_started);
        let source_meta = source_panel_meta(&source_path, &source);
        let mut html = render_config_page(
            &apps,
            app_title.as_str(),
            &app_id,
            Some(&topbar_menus),
            Some(source.as_str()),
            Some(&source_meta),
            lightweight_scene.as_deref(),
            upload_enabled,
        );
        html = fill_perf_placeholders(html, 0, elapsed_ms(app_started));
        html = fill_manage_wall_clock_placeholders(html, 0, elapsed_ms(app_started));
        let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
            &app_root,
            Some(state.source_root.as_path()),
            None,
        );
        html = fill_gis_tiles_placeholders(html, &gis);
        tracing::info!(
            app_id = %app_id,
            route_mode = route_mode.slug(),
            target = %target,
            source_read_ms,
            total_ms = elapsed_ms(app_started),
            phase = "finish_light_config",
            "app page request finished without compile"
        );
        return Ok(Html(html).into_response());
    }
    if route_mode == UiRouteMode::Upload {
        let rel = upload_rel.clone().ok_or_else(|| {
            AppError::status(
                axum::http::StatusCode::NOT_FOUND,
                "app has no paths.upload configured",
            )
        })?;
        let target = if let Some(file) = request_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            format!("{rel}/{file}")
        } else {
            rel
        };
        let source_path = app_root.join(&target);
        let source_started = Instant::now();
        let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
        let source_read_ms = elapsed_ms(source_started);
        let source_meta = source_panel_meta(&source_path, &source);
        let mut html = render_upload_page(
            &apps,
            app_title.as_str(),
            &app_id,
            Some(&topbar_menus),
            request_file.as_deref(),
            Some(source.as_str()),
            Some(&source_meta),
            lightweight_scene.as_deref(),
            upload_enabled,
            Some(upload_root_label),
            &upload_files,
        );
        html = fill_perf_placeholders(html, 0, elapsed_ms(app_started));
        html = fill_manage_wall_clock_placeholders(html, 0, elapsed_ms(app_started));
        let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
            &app_root,
            Some(state.source_root.as_path()),
            None,
        );
        html = fill_gis_tiles_placeholders(html, &gis);
        tracing::info!(
            app_id = %app_id,
            route_mode = route_mode.slug(),
            target = %target,
            source_read_ms,
            total_ms = elapsed_ms(app_started),
            phase = "finish_light_upload",
            "app page request finished without compile"
        );
        return Ok(Html(html).into_response());
    }
    if route_mode == UiRouteMode::Build
        && query
            .tab
            .as_deref()
            .map(str::trim)
            .is_some_and(|tab| tab.eq_ignore_ascii_case("source"))
    {
        let target = manage_file
            .clone()
            .unwrap_or_else(|| resolve_app_entry_main(&app_root));
        let source_path = app_root.join(&target);
        let source_started = Instant::now();
        let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
        let source_read_ms = elapsed_ms(source_started);
        let source_meta = source_panel_meta(&source_path, &source);
        let file_tree = source_tree(&app_root).unwrap_or_default();
        let mut html = render_build_source_page(
            &apps,
            app_title.as_str(),
            &app_id,
            Some(&topbar_menus),
            &file_tree,
            target.as_str(),
            source.as_str(),
            Some(&source_meta),
            lightweight_scene.as_deref(),
            query.tab.as_deref(),
            upload_enabled,
        );
        html = fill_perf_placeholders(html, 0, elapsed_ms(app_started));
        html = fill_manage_wall_clock_placeholders(html, 0, elapsed_ms(app_started));
        let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
            &app_root,
            Some(state.source_root.as_path()),
            None,
        );
        html = fill_gis_tiles_placeholders(html, &gis);
        tracing::info!(
            app_id = %app_id,
            route_mode = route_mode.slug(),
            target = %target,
            source_read_ms,
            total_ms = elapsed_ms(app_started),
            phase = "finish_light_build_source",
            "app page request finished without compile"
        );
        return Ok(Html(html).into_response());
    }
    let manage_script_file = manage_file
        .as_deref()
        .filter(|t| is_script_target(t))
        .map(ToString::to_string);
    let normalized_preview_target = if route_mode == UiRouteMode::Build {
        manage_script_file.clone()
    } else {
        None
    };
    let compile_scene = if route_mode == UiRouteMode::App || route_mode == UiRouteMode::Build {
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
        && !recent_compile_failure(&state, &app_id, &compile_options)
    {
        let peek_started = Instant::now();
        match peek_compile_cache_hit(&state, &app_id, &compile_options, components_root.as_path()) {
            Some(hit) => CompileWithCacheOutcome {
                compiled: hit.compiled,
                cache_hit: true,
                compile_revision: hit.compile_revision,
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
                let target = if route_mode == UiRouteMode::Build {
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
                        upload_enabled,
                        Some(upload_root_label),
                        &upload_files,
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
                    let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
                        &state.source_root.join(&app_id),
                        Some(state.source_root.as_path()),
                        None,
                    );
                    let h = fill_gis_tiles_placeholders(h, &gis);
                    (h, ssr_emit_ms, handler_ms)
                };
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
    let compile_revision = compile_outcome.compile_revision.clone();
    let compile_revision_scope = compile_outcome.revision_scope.clone();
    let compile_cache_validation = compile_outcome.cache_validation.clone();
    let compile_ms = compile_outcome.compile_ms;
    let compile_cache_lookup_ms = compile_outcome.cache_lookup_ms;
    let mut compiled = compile_outcome.compiled;
    if route_mode == UiRouteMode::App && access_static_file.is_none() {
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
            let loc = format!("/apps/build/{}", app_id.trim_start_matches('/'));
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
                         <p><a href=\"/apps/build/{app_esc}\">返回构建视图</a></p></body></html>",
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
                     <p><a href=\"/apps/build/{manage_href_app}\">返回构建视图</a></p></body></html>",
                )),
            )
                .into_response());
        }
    }
    let manage_scene_resolved = if access_static_file.is_some() {
        None
    } else if route_mode == UiRouteMode::App {
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
    let target = if route_mode == UiRouteMode::Build {
        manage_file.clone().unwrap_or(manage_default_file)
    } else if let Some(static_file) = access_static_file.clone() {
        static_file
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
    let gis = crate::gis_config::GisTilesConfig::resolve_for_app(
        &state.source_root.join(&app_id),
        Some(state.source_root.as_path()),
        None,
    );
    let render_cache_key = page_render_cache_key(
        &app_id,
        route_mode,
        &compile_revision,
        target.as_str(),
        source.as_str(),
        Some(&source_meta),
        manage_scene_resolved.as_deref(),
        normalized_preview_target.as_deref(),
        query.tab.as_deref(),
        query.diag_filter.as_deref(),
        chrome_hidden,
        upload_enabled,
        Some(upload_root_label),
        Some(&topbar_menus),
        &upload_files,
        &gis,
    );
    let (html, page_render_cache_hit, ssr_http_response_body_ms, handler_html_ready_ms) = {
        let t = Instant::now();
        let (h, cache_hit) = render_page_template_with_cache(render_cache_key, || {
            let rendered = render_page(
                &apps,
                &compiled,
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
                upload_enabled,
                Some(upload_root_label),
                &upload_files,
            );
            fill_gis_tiles_placeholders(rendered, &gis)
        });
        let ssr_emit_ms = elapsed_ms(t);
        let total_wall = elapsed_ms(app_started);
        let h = fill_perf_placeholders(h, ssr_emit_ms, total_wall);
        let handler_ms = elapsed_ms(app_started);
        let h = fill_manage_wall_clock_placeholders(h, ssr_emit_ms, handler_ms);
        (h, cache_hit, ssr_emit_ms, handler_ms)
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
    insert_page_render_cache_hit_header(&mut res, page_render_cache_hit);
    insert_manage_compile_observability_headers(&mut res, &compiled);
    let request_meta = CompileWithCacheOutcome {
        compiled: compiled.clone(),
        cache_hit: compile_cache_hit,
        compile_revision: compile_revision.clone(),
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
        page_render_cache_hit,
        handler_html_ready_ms,
        ssr_http_response_body_ms,
        total_ms = elapsed_ms(app_started),
        phase = "finish",
        "app page request finished"
    );
    Ok(res)
}
