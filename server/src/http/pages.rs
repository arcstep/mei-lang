use std::{fs, path::Path};

use anyhow::Context;
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{Html, Redirect, Response},
};
use mei_lang_app::{render_page, UiRouteMode};
use mei_lang_kernel::{compile_app_with_options, discover_apps, read_source_file, CompileOptions};
use serde::Deserialize;

use crate::{AppError, AppState};

#[derive(Debug, Deserialize)]
pub struct AppQuery {
    target: Option<String>,
    entry: Option<String>,
    preview_target: Option<String>,
    chrome: Option<String>,
}

pub async fn index(State(state): State<AppState>) -> Result<Redirect, AppError> {
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let first = apps
        .first()
        .ok_or_else(|| AppError::msg("examples source root does not contain any apps"))?;
    Ok(Redirect::to(&format!("/apps/manage/{}", first.id)))
}

pub async fn app_page(
    State(state): State<AppState>,
    AxumPath((mode, app_id)): AxumPath<(String, String)>,
    Query(query): Query<AppQuery>,
) -> Result<Html<String>, AppError> {
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let compile_options = CompileOptions {
        entry: query.entry.clone(),
        preview_target: query.preview_target.clone(),
    };
    let compiled = compile_app_with_options(&state.source_root, &app_id, compile_options)
        .map_err(AppError::from)?;
    let target = query
        .target
        .or_else(|| query.preview_target.clone())
        .unwrap_or_else(|| compiled.entry_target.clone());
    let source_path = state.source_root.join(&app_id).join(&target);
    let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
    let chrome_hidden = query
        .chrome
        .as_deref()
        .map(|value| value.eq_ignore_ascii_case("none"))
        .unwrap_or(false);
    let html = render_page(
        &apps,
        &compiled,
        UiRouteMode::from_slug(&mode),
        Some(target.as_str()),
        Some(source.as_str()),
        query.entry.as_deref(),
        query.preview_target.as_deref(),
        chrome_hidden,
    );
    Ok(Html(html))
}

pub async fn component_asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    serve_static_asset(
        state.source_root.join("_components").join(&path),
        "component asset",
    )
}

pub async fn app_asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    serve_static_asset(
        state.package_root.join("app").join("assets").join(&path),
        "app asset",
    )
}

pub async fn workspace_app_asset(
    State(state): State<AppState>,
    AxumPath((app_id, path)): AxumPath<(String, String)>,
) -> Result<Response, AppError> {
    serve_static_asset(
        state.source_root.join(&app_id).join(&path),
        "workspace app asset",
    )
}

fn serve_static_asset(asset_path: std::path::PathBuf, label: &str) -> Result<Response, AppError> {
    if !asset_path.exists() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            format!("{label} not found: {}", asset_path.display()),
        ));
    }
    let bytes = fs::read(&asset_path)
        .with_context(|| format!("failed to read {}", asset_path.display()))
        .map_err(AppError::from)?;
    let mut response = Response::new(bytes.into());
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_path(&asset_path)),
    );
    Ok(response)
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        _ => "text/plain; charset=utf-8",
    }
}
