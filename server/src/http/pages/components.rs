use std::path::Path;

use axum::{
    body::Body,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};

use mei_lang_kernel::resolve_components_root as kernel_resolve_components_root;

use mei_lang_kernel::resolve_app_root;

use crate::{AppError, AppState};
use crate::http::scene_bundle::parse_scene_bundle_request_path;

use super::static_serve::serve_static_asset_with_cache;

const COMPONENT_REVALIDATE_CACHE_CONTROL: &str = "private, no-cache";
const VENDOR_REVALIDATE_CACHE_CONTROL: &str = "public, no-cache";
const IMMUTABLE_BUNDLE_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

pub(crate) fn resolve_components_root(source_root: &Path) -> std::path::PathBuf {
    kernel_resolve_components_root(source_root)
}

fn parse_glyph_range(range: &str) -> Option<(u32, u32)> {
    let mut parts = range.split('-');
    let start = parts.next()?.parse::<u32>().ok()?;
    let end = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || end < start {
        return None;
    }
    Some((start, end))
}

fn maplibre_glyph_fallback_asset_path(asset_path: &Path) -> Option<std::path::PathBuf> {
    let normalized = asset_path.to_string_lossy().replace('\\', "/");
    if !normalized.contains("/vendor/maplibre/fonts/") {
        return None;
    }
    let file_name = asset_path.file_name()?.to_str()?;
    let range = file_name.strip_suffix(".pbf")?;
    if range == "0-255" || parse_glyph_range(range).is_none() {
        return None;
    }
    Some(asset_path.parent()?.join("0-255.pbf"))
}

fn resolve_component_asset_path(components_root: &Path, request_path: &str) -> std::path::PathBuf {
    let requested = components_root.join(request_path);
    if requested.exists() {
        return requested;
    }
    if let Some(fallback) = maplibre_glyph_fallback_asset_path(&requested) {
        if fallback.exists() {
            return fallback;
        }
    }
    requested
}

fn is_missing_optional_source_map(request_path: &str, asset_path: &Path) -> bool {
    request_path.ends_with(".js.map") && !asset_path.exists()
}

pub async fn component_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    if let Some((app_id, scene_id, revision)) = parse_scene_bundle_request_path(path.as_str()) {
        let app_root = resolve_app_root(state.source_root.as_path(), &app_id);
        let bundle_path = crate::http::scene_bundle::resolve_scene_bundle_cache_path(
            app_root.as_path(),
            scene_id.as_str(),
            revision.as_str(),
        );
        return serve_static_asset_with_cache(
            bundle_path,
            "scene component bundle",
            &headers,
            IMMUTABLE_BUNDLE_CACHE_CONTROL,
        );
    }
    let components_root = resolve_components_root(&state.source_root);
    let asset_path = resolve_component_asset_path(&components_root, &path);
    if is_missing_optional_source_map(path.as_str(), asset_path.as_path()) {
        // vendor 包常带 sourceMappingURL 但未随仓分发 .map；浏览器探测失败不应记 404 错误。
        return Ok(Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .expect("empty source-map probe response"));
    }
    let cache_control = if path.starts_with("vendor/") {
        VENDOR_REVALIDATE_CACHE_CONTROL
    } else {
        COMPONENT_REVALIDATE_CACHE_CONTROL
    };
    serve_static_asset_with_cache(asset_path, "component asset", &headers, cache_control)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        is_missing_optional_source_map, maplibre_glyph_fallback_asset_path, parse_glyph_range,
    };

    #[test]
    fn glyph_range_parser_accepts_positive_closed_ranges() {
        assert_eq!(parse_glyph_range("0-255"), Some((0, 255)));
        assert_eq!(parse_glyph_range("8192-8447"), Some((8192, 8447)));
        assert_eq!(parse_glyph_range("10-9"), None);
        assert_eq!(parse_glyph_range("oops"), None);
    }

    #[test]
    fn missing_js_map_is_treated_as_optional_probe() {
        let path = Path::new("/tmp/_components/vendor/maplibre/maplibre-gl.js.map");
        assert!(is_missing_optional_source_map(
            "vendor/maplibre/maplibre-gl.js.map",
            path
        ));
        assert!(!is_missing_optional_source_map(
            "vendor/maplibre/maplibre-gl.js",
            path
        ));
    }

    #[test]
    fn maplibre_glyph_requests_fallback_to_base_range_file() {
        let path = Path::new(
            "/tmp/_components/vendor/maplibre/fonts/Open Sans Regular,Arial Unicode MS Regular/8192-8447.pbf",
        );
        let fallback = maplibre_glyph_fallback_asset_path(path).expect("must produce fallback");
        assert!(fallback
            .to_string_lossy()
            .ends_with("/Open Sans Regular,Arial Unicode MS Regular/0-255.pbf"));
    }
}
