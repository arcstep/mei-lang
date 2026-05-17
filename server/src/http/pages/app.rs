use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use mei_lang_app::{render_page, UiRouteMode};
use mei_lang_kernel::{
    discover_apps, read_source_file, CompileOptions, Severity,
};

use crate::{AppError, AppState};

use super::super::compile_cache::{compile_app_with_cache, CompileWithCacheFailure};
use super::app_render::{
    choose_default_app, compile_error_fallback_app, source_panel_meta,
};
use super::components::resolve_components_root;
use super::menus::load_segment_topbar_menus;
use super::util::{append_perf_diagnostic, elapsed_ms, fill_perf_placeholders, is_script_target};

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
    let compile_outcome = match compile_app_with_cache(
        &state,
        &app_id,
        compile_options,
        components_root.as_path(),
    ) {
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
            let render_started = Instant::now();
            append_perf_diagnostic(
                &mut compiled,
                target.as_str(),
                discover_ms,
                compile_ms,
                false,
                cache_lookup_ms,
                source_read_ms,
            );
            let html = render_page(
                &apps,
                &compiled,
                &app_id,
                Some(&topbar_menus),
                route_mode,
                Some(target.as_str()),
                Some(source.as_str()),
                Some(&source_meta),
                query.entry.as_deref(),
                normalized_preview_target.as_deref(),
                query.tab.as_deref(),
                chrome_hidden,
            );
            let render_ms = elapsed_ms(render_started);
            let html = fill_perf_placeholders(html, render_ms, elapsed_ms(app_started));
            return Ok(Html(html).into_response());
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
            let path = d
                .source_path
                .as_deref()
                .unwrap_or("(unknown)");
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
    let render_started = Instant::now();
    append_perf_diagnostic(
        &mut compiled,
        target.as_str(),
        discover_ms,
        compile_ms,
        compile_cache_hit,
        compile_cache_lookup_ms,
        source_read_ms,
    );
    let html = render_page(
        &apps,
        &compiled,
        &app_id,
        Some(&topbar_menus),
        route_mode,
        Some(target.as_str()),
        Some(source.as_str()),
        Some(&source_meta),
        query.entry.as_deref(),
        normalized_preview_target.as_deref(),
        query.tab.as_deref(),
        chrome_hidden,
    );
    let render_ms = elapsed_ms(render_started);
    let html = fill_perf_placeholders(html, render_ms, elapsed_ms(app_started));
    Ok(Html(html).into_response())
}
