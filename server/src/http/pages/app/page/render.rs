use std::time::Instant;

use axum::{
    http::{HeaderName, HeaderValue},
    response::{Html, IntoResponse, Response},
};
use mei_lang_app::{render_page, page_body_theme_style, HostAccountView, TopbarMenuContext, UiRouteMode, UploadFileEntry};
use mei_lang_kernel::{
    load_workspace_config, read_source_file, resolve_app_root, CompiledApp, Severity, WorkspaceAppMeta,
};

use crate::AppState;
use crate::http::scene_bundle::{
    probe_scene_component_bundle, scene_bundle_cache_marker, scene_bundle_status,
    schedule_scene_component_bundle_build, should_build_scene_bundle,
};

use crate::http::pages::app::page_cache::{page_render_cache_key, render_page_template_with_cache};
use crate::http::pages::app::page_render::{
    insert_manage_compile_observability_headers, insert_manage_compile_request_headers,
    insert_page_render_cache_hit_header,
};
use crate::http::pages::app::query::AppQuery;
use crate::http::pages::app::scene::{
    canonical_scene_for_target, default_file_for_scene, manage_scene_for_render,
};
use crate::http::pages::app_render::source_panel_meta;
use crate::http::pages::util::{
    elapsed_ms, fill_manage_wall_clock_placeholders, fill_page_load_observability_placeholders,
    fill_page_shell_placeholders, fill_perf_placeholders, measure_page_html_payload,
    push_manage_page_pipeline_diag,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn render_compiled_success(
    state: &AppState,
    route_mode: UiRouteMode,
    app_id: &str,
    query: &AppQuery,
    apps: &[WorkspaceAppMeta],
    topbar_menus: &TopbarMenuContext,
    compiled: &mut CompiledApp,
    compile_cache_hit: bool,
    compile_revision: &str,
    compile_revision_scope: &str,
    compile_cache_validation: &str,
    compile_cache_lookup_ms: u64,
    compile_ms: u64,
    access_static_file: Option<&str>,
    access_path_scene: Option<&str>,
    manage_file: Option<&str>,
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
    let manage_scene_resolved = if access_static_file.is_some() {
        None
    } else if route_mode.uses_scene_route() {
        access_path_scene.map(str::to_string)
    } else {
        manage_scene_for_render(compiled, query.scene.as_deref())
            .or_else(|| canonical_scene_for_target(compiled, manage_file))
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
    };
    let scene_for_default = manage_scene_resolved
        .as_deref()
        .or(compiled.active_scene.as_deref());
    let manage_default_file = default_file_for_scene(compiled, scene_for_default);
    let target = if route_mode == UiRouteMode::Build {
        manage_file
            .map(ToString::to_string)
            .unwrap_or(manage_default_file)
    } else if let Some(static_file) = access_static_file {
        static_file.to_string()
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
    let app_root = resolve_app_root(state.source_root.as_path(), app_id);
    let source_path = app_root.join(&target);
    let source_started = Instant::now();
    let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
    let source_read_ms = elapsed_ms(source_started);
    let source_meta = source_panel_meta(&source_path, &source);
    push_manage_page_pipeline_diag(
        compiled,
        app_id,
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
        &app_root,
        Some(state.source_root.as_path()),
        None,
    );
    let scene_id = manage_scene_resolved
        .as_deref()
        .or(compiled.active_scene.as_deref())
        .unwrap_or("home");
    let scene_bundle_enabled = should_build_scene_bundle(app_root.as_path(), route_mode, scene_id);
    let scene_bundle_marker = scene_bundle_cache_marker(app_root.as_path(), route_mode, scene_id);
    let render_cache_key = page_render_cache_key(
        app_id,
        route_mode,
        compile_revision,
        target.as_str(),
        source.as_str(),
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
        Some(topbar_menus),
        upload_files,
        &gis,
        auth_enabled,
        account_view,
        scene_bundle_marker.as_str(),
    );
    let mut scene_bundle_header_status = if scene_bundle_enabled {
        "cached".to_string()
    } else {
        "disabled".to_string()
    };
    let mut scene_bundle_revision_header: Option<String> = None;
    let mut scene_bundle_probe_ms = 0u64;
    let mut scene_bundle_build_scheduled = false;
    let (mut html, page_render_cache_hit, ssr_http_response_body_ms, handler_html_ready_ms) = {
        let t = Instant::now();
        let (h, cache_hit) = render_page_template_with_cache(render_cache_key, || {
            let scene_bundle_probe = if scene_bundle_enabled {
                let scene_bundle_probe_started = Instant::now();
                let probe = probe_scene_component_bundle(
                    state.package_root.as_path(),
                    state.source_root.as_path(),
                    app_id,
                    scene_id,
                    compile_revision,
                    &compiled.component_assets,
                );
                scene_bundle_probe_ms = elapsed_ms(scene_bundle_probe_started);
                probe
            } else {
                crate::http::scene_bundle::SceneBundleProbe {
                    bundle: None,
                    cache_marker: scene_bundle_marker.clone(),
                    build: None,
                }
            };
            scene_bundle_header_status = scene_bundle_status(&scene_bundle_probe).to_string();
            if let Some(bundle) = scene_bundle_probe.bundle.as_ref() {
                scene_bundle_revision_header = Some(bundle.revision.clone());
            }
            if let Some(build) = scene_bundle_probe.build.as_ref() {
                scene_bundle_build_scheduled = true;
                schedule_scene_component_bundle_build(state.package_root.as_path(), build);
            }
            let scene_bundle_for_render =
                scene_bundle_probe.bundle.as_ref().map(|bundle| bundle.url.as_str());
            let workspace = load_workspace_config(state.source_root.as_path());
            let body_theme_style = page_body_theme_style(&workspace, Some(compiled));
            let rendered = render_page(
                apps,
                compiled,
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
                scene_bundle_for_render,
                body_theme_style.as_str(),
            );
            fill_page_shell_placeholders(rendered, &gis, state.source_root.as_path())
        });
        let ssr_emit_ms = elapsed_ms(t);
        let total_wall = elapsed_ms(app_started);
        let h = fill_perf_placeholders(h, ssr_emit_ms, total_wall);
        let handler_ms = elapsed_ms(app_started);
        let h = fill_manage_wall_clock_placeholders(h, ssr_emit_ms, handler_ms);
        (h, cache_hit, ssr_emit_ms, handler_ms)
    };
    let payload_stats = measure_page_html_payload(&html);
    html = fill_page_load_observability_placeholders(
        html,
        compile_ms,
        compile_cache_hit,
        payload_stats.html_bytes,
        payload_stats.data_props_bytes,
        payload_stats.data_props_count,
    );
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
    if let Ok(v) = HeaderValue::from_str(scene_bundle_header_status.as_str()) {
        res.headers_mut()
            .insert(HeaderName::from_static("x-mei-scene-bundle-status"), v);
    }
    if let Ok(v) = HeaderValue::from_str(&scene_bundle_probe_ms.to_string()) {
        res.headers_mut()
            .insert(HeaderName::from_static("x-mei-scene-bundle-probe-ms"), v);
    }
    if let Ok(v) = HeaderValue::from_str(if scene_bundle_build_scheduled { "1" } else { "0" }) {
        res.headers_mut().insert(
            HeaderName::from_static("x-mei-scene-bundle-build-scheduled"),
            v,
        );
    }
    if let Some(bundle_revision) = scene_bundle_revision_header.as_deref() {
        if let Ok(v) = HeaderValue::from_str(bundle_revision) {
            res.headers_mut()
                .insert(HeaderName::from_static("x-mei-scene-bundle-revision"), v);
        }
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
    insert_page_render_cache_hit_header(&mut res, page_render_cache_hit);
    insert_manage_compile_observability_headers(&mut res, compiled);
    insert_manage_compile_request_headers(
        &mut res,
        &crate::http::compile_cache::CompileWithCacheOutcome {
            compiled: compiled.clone(),
            cache_hit: compile_cache_hit,
            artifact_cache_hit: false,
            compile_revision: compile_revision.to_string(),
            revision_scope: compile_revision_scope.to_string(),
            cache_validation: compile_cache_validation.to_string(),
            cache_lookup_ms: compile_cache_lookup_ms,
            artifact_load_ms: 0,
            compile_cache_lock_wait_ms: 0,
            compile_ms,
        },
    );
    tracing::info!(
        app_id = %app_id,
        route_mode = route_mode.slug(),
        target = %target,
        compile_cache_hit,
        compile_ms,
        compile_cache_lookup_ms,
        source_read_ms,
        page_render_cache_hit,
        scene_bundle_probe_ms,
        scene_bundle_build_scheduled,
        handler_html_ready_ms,
        ssr_http_response_body_ms,
        html_bytes = payload_stats.html_bytes,
        data_props_count = payload_stats.data_props_count,
        data_props_bytes = payload_stats.data_props_bytes,
        data_props_max_bytes = payload_stats.data_props_max_bytes,
        scene_drilldown_context_bytes = payload_stats.scene_drilldown_context_bytes,
        total_ms = elapsed_ms(app_started),
        phase = "finish",
        "app page request finished"
    );
    if payload_stats.data_props_bytes > 5 * 1024 * 1024 {
        tracing::warn!(
            app_id = %app_id,
            route_mode = route_mode.slug(),
            target = %target,
            data_props_count = payload_stats.data_props_count,
            data_props_bytes = payload_stats.data_props_bytes,
            data_props_max_bytes = payload_stats.data_props_max_bytes,
            html_bytes = payload_stats.html_bytes,
            page_render_cache_hit,
            "SSR data-props payload exceeds 5MB budget"
        );
    }
    res
}
