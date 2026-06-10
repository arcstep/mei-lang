use std::sync::OnceLock;

use anyhow::Context;
use axum::{
    body::Body,
    extract::Path as AxumPath,
    http::{header, HeaderValue, StatusCode, Uri},
    response::Response,
};
use serde_json::Value;

use crate::AppError;

static GIS_PROXY_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn gis_proxy_client() -> &'static reqwest::Client {
    GIS_PROXY_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .build()
            .expect("build GIS proxy reqwest client")
    })
}

fn gis_proxy_upstream_base() -> String {
    std::env::var("MEI_GIS_PROXY_UPSTREAM")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
}

fn build_gis_proxy_target(base: &str, path: &str, query: Option<&str>) -> String {
    let normalized_base = base.trim().trim_end_matches('/');
    let normalized_path = path.trim_start_matches('/');
    let mut url = if normalized_path.is_empty() {
        normalized_base.to_string()
    } else {
        format!("{normalized_base}/{normalized_path}")
    };
    if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
        url.push('?');
        url.push_str(query);
    }
    url
}

fn build_same_origin_gis_url(path: &str, query: Option<&str>) -> String {
    let normalized_path = path.trim_start_matches('/');
    let mut url = if normalized_path.is_empty() {
        "/gis".to_string()
    } else {
        format!("/gis/{normalized_path}")
    };
    if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
        url.push('?');
        url.push_str(query);
    }
    url
}

fn rewrite_tile_entry(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let scheme_sep = trimmed.find("://")?;
        let after_scheme = &trimmed[(scheme_sep + 3)..];
        let path_start = after_scheme.find('/').map(|idx| idx + scheme_sep + 3)?;
        let raw_path = &trimmed[path_start..];
        let (path, query) = match raw_path.split_once('?') {
            Some((path, query)) => (path, Some(query)),
            None => (raw_path, None),
        };
        return Some(build_same_origin_gis_url(path, query));
    }
    if trimmed.starts_with('/') {
        return Some(build_same_origin_gis_url(trimmed, None));
    }
    None
}

fn rewrite_tilejson_body(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut json: Value = serde_json::from_slice(bytes).ok()?;
    let tiles = json.get_mut("tiles")?.as_array_mut()?;
    let mut changed = false;
    for entry in tiles.iter_mut() {
        let Some(raw) = entry.as_str() else {
            continue;
        };
        let Some(rewritten) = rewrite_tile_entry(raw) else {
            continue;
        };
        if rewritten != raw {
            *entry = Value::String(rewritten);
            changed = true;
        }
    }
    if !changed {
        return None;
    }
    serde_json::to_vec(&json).ok()
}

fn copy_proxy_header(
    source: &reqwest::header::HeaderMap,
    target: &mut axum::http::HeaderMap,
    name: reqwest::header::HeaderName,
) {
    if let Some(value) = source.get(&name) {
        target.insert(name, value.clone());
    }
}

pub async fn gis_proxy(AxumPath(path): AxumPath<String>, uri: Uri) -> Result<Response, AppError> {
    let target = build_gis_proxy_target(
        gis_proxy_upstream_base().as_str(),
        path.as_str(),
        uri.query(),
    );
    let upstream = gis_proxy_client()
        .get(target.as_str())
        .send()
        .await
        .with_context(|| format!("failed to proxy GIS request to {target}"))
        .map_err(AppError::from)?;
    let status = StatusCode::from_u16(upstream.status().as_u16())
        .map_err(|error| AppError::msg(format!("invalid upstream GIS status: {error}")))?;
    let headers = upstream.headers().clone();
    let upstream_body = upstream
        .bytes()
        .await
        .with_context(|| format!("failed to read GIS proxy response from {target}"))
        .map_err(AppError::from)?;
    let is_json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.contains("application/json"))
        .unwrap_or(false);
    let body = if is_json {
        rewrite_tilejson_body(upstream_body.as_ref()).unwrap_or_else(|| upstream_body.to_vec())
    } else {
        upstream_body.to_vec()
    };

    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    copy_proxy_header(&headers, response.headers_mut(), header::CONTENT_TYPE);
    copy_proxy_header(&headers, response.headers_mut(), header::CONTENT_ENCODING);
    copy_proxy_header(&headers, response.headers_mut(), header::CACHE_CONTROL);
    copy_proxy_header(&headers, response.headers_mut(), header::ETAG);
    copy_proxy_header(&headers, response.headers_mut(), header::LAST_MODIFIED);
    if !response.headers().contains_key(header::CACHE_CONTROL) {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        );
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::{build_gis_proxy_target, rewrite_tilejson_body};

    #[test]
    fn builds_proxy_target_without_duplicate_slashes() {
        assert_eq!(
            build_gis_proxy_target("http://127.0.0.1:8080/", "/shapingba-z10-16", None),
            "http://127.0.0.1:8080/shapingba-z10-16"
        );
    }

    #[test]
    fn preserves_query_string_for_proxy_target() {
        assert_eq!(
            build_gis_proxy_target(
                "http://127.0.0.1:8080",
                "shapingba-z10-16/10/838/412.pbf",
                Some("foo=1&bar=2"),
            ),
            "http://127.0.0.1:8080/shapingba-z10-16/10/838/412.pbf?foo=1&bar=2"
        );
    }

    #[test]
    fn rewrites_absolute_tilejson_tiles_to_same_origin_gis_proxy() {
        let raw = br#"{"tilejson":"3.0.0","tiles":["http://127.0.0.1:8080/shapingba-z10-16/{z}/{x}/{y}"]}"#;
        let body = rewrite_tilejson_body(raw).expect("must rewrite tilejson");
        let text = String::from_utf8(body).expect("utf8 json");
        assert!(text.contains(r#""tiles":["/gis/shapingba-z10-16/{z}/{x}/{y}"]"#));
    }
}
