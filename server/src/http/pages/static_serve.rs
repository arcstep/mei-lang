use std::{fs, path::Path};

use anyhow::Context;
use axum::{
    http::{header::CONTENT_TYPE, HeaderName, HeaderValue, StatusCode},
    response::Response,
};

use crate::AppError;

pub(crate) fn serve_static_asset(
    asset_path: std::path::PathBuf,
    label: &str,
) -> Result<Response, AppError> {
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
    response.headers_mut().insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

pub(crate) fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("md") | Some("markdown") => "text/markdown; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("tsv") => "text/tab-separated-values; charset=utf-8",
        Some("pdf") => "application/pdf",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        _ => "text/plain; charset=utf-8",
    }
}
