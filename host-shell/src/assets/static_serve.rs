use std::{fs, path::Path, time::UNIX_EPOCH};

use anyhow::Context;
use axum::{
    body::Body,
    http::{
        header::{CONTENT_TYPE, ETAG, IF_NONE_MATCH},
        HeaderMap, HeaderName, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
};

pub fn serve_static_asset_with_cache(
    asset_path: std::path::PathBuf,
    label: &str,
    request_headers: &HeaderMap,
    cache_control: &str,
) -> anyhow::Result<Response> {
    if !asset_path.exists() {
        anyhow::bail!("{label} not found: {}", asset_path.display());
    }
    let metadata = fs::metadata(&asset_path)
        .with_context(|| format!("failed to stat {}", asset_path.display()))?;
    let etag = etag_for_metadata(&metadata);
    if request_matches_etag(request_headers, etag.as_str()) {
        let mut response = Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())?;
        response.headers_mut().insert(
            ETAG,
            HeaderValue::from_str(etag.as_str())
                .unwrap_or_else(|_| HeaderValue::from_static("\"0\"")),
        );
        response.headers_mut().insert(
            HeaderName::from_static("cache-control"),
            HeaderValue::from_str(cache_control)
                .unwrap_or_else(|_| HeaderValue::from_static("private, no-cache")),
        );
        return Ok(response);
    }
    let bytes = fs::read(&asset_path)
        .with_context(|| format!("failed to read {}", asset_path.display()))?;
    let mut response = Response::new(bytes.into());
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_path(&asset_path)),
    );
    response.headers_mut().insert(
        HeaderName::from_static("cache-control"),
        HeaderValue::from_str(cache_control)
            .unwrap_or_else(|_| HeaderValue::from_static("private, no-cache")),
    );
    response.headers_mut().insert(
        ETAG,
        HeaderValue::from_str(etag.as_str()).unwrap_or_else(|_| HeaderValue::from_static("\"0\"")),
    );
    Ok(response)
}

pub fn asset_not_found(label: &str, path: &Path) -> Response {
    (
        StatusCode::NOT_FOUND,
        format!("{label} not found: {}", path.display()),
    )
        .into_response()
}

fn etag_for_metadata(metadata: &fs::Metadata) -> String {
    let len = metadata.len();
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis())
        .unwrap_or(0);
    format!("\"{:x}-{:x}\"", len, modified_ms)
}

fn request_matches_etag(request_headers: &HeaderMap, etag: &str) -> bool {
    let Some(raw) = request_headers
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    raw.split(',')
        .map(str::trim)
        .any(|candidate| candidate == "*" || candidate == etag)
}

pub fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("md") | Some("markdown") => "text/markdown; charset=utf-8",
        Some("csv") => "text/csv; charset=utf-8",
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("pbf") => "application/x-protobuf",
        _ => "text/plain; charset=utf-8",
    }
}
