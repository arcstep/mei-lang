use std::{
    fs,
    path::Path,
};

use anyhow::Context;
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{Html, Redirect, Response},
};
use mei_lang_app::{render_page, UiRouteMode};
use mei_lang_kernel::{compile_app, discover_apps, read_source_file};
use serde::Deserialize;

use crate::{AppError, AppState};

#[derive(Debug, Deserialize)]
pub struct AppQuery {
    target: Option<String>,
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
    let compiled = compile_app(&state.source_root, &app_id).map_err(AppError::from)?;
    let target = query.target.unwrap_or_else(|| compiled.entry_target.clone());
    let source_path = state.source_root.join(&app_id).join(&target);
    let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
    let html = render_page(
        &apps,
        &compiled,
        UiRouteMode::from_slug(&mode),
        Some(target.as_str()),
        Some(source.as_str()),
    );
    Ok(Html(html))
}

pub async fn component_asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    let asset_path = state.source_root.join("_components").join(&path);
    if !asset_path.exists() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            format!("component asset not found: {}", asset_path.display()),
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
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    }
}
