use std::path::Path;

use axum::{
    extract::{Path as AxumPath, State},
    response::Response,
};

use crate::{AppError, AppState};

use super::static_serve::serve_static_asset;

pub(crate) fn resolve_components_root(source_root: &Path) -> std::path::PathBuf {
    let local = source_root.join("_components");
    if local.exists() {
        return local;
    }
    if let Some(parent) = source_root.parent() {
        let shared = parent.join("_components");
        if shared.exists() {
            return shared;
        }
    }
    local
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

pub async fn component_asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    let components_root = resolve_components_root(&state.source_root);
    serve_static_asset(resolve_component_asset_path(&components_root, &path), "component asset")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{maplibre_glyph_fallback_asset_path, parse_glyph_range};

    #[test]
    fn glyph_range_parser_accepts_positive_closed_ranges() {
        assert_eq!(parse_glyph_range("0-255"), Some((0, 255)));
        assert_eq!(parse_glyph_range("8192-8447"), Some((8192, 8447)));
        assert_eq!(parse_glyph_range("10-9"), None);
        assert_eq!(parse_glyph_range("oops"), None);
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
