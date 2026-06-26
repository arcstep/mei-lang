use super::manage_routing::{access_scene_route_suffix, encode_query_value};
use super::route::UiRouteMode;

pub fn view_base_href(view: UiRouteMode, app_path: &str) -> String {
    format!("/apps/{}/{}", view.slug(), app_path.trim_start_matches('/'))
}

pub fn build_href_with_catalog(
    app_path: &str,
    file: Option<&str>,
    tab: Option<&str>,
    catalog: Option<&str>,
    pack: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    append_catalog_query(&mut parts, catalog, pack);
    if let Some(f) = file.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("file={}", encode_query_value(f)));
    }
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("tab={}", encode_query_value(t)));
    }
    let base = view_base_href(UiRouteMode::Build, app_path);
    if parts.is_empty() {
        base
    } else {
        format!("{base}?{}", parts.join("&"))
    }
}

pub fn runtime_href(app_path: &str, node: Option<&str>, tab: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(n) = node.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("node={}", encode_query_value(n)));
    }
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("tab={}", encode_query_value(t)));
    }
    let base = view_base_href(UiRouteMode::Runtime, app_path);
    if parts.is_empty() {
        base
    } else {
        format!("{base}?{}", parts.join("&"))
    }
}

pub fn config_href(app_path: &str) -> String {
    view_base_href(UiRouteMode::Config, app_path)
}

pub fn upload_href(app_path: &str, file: Option<&str>) -> String {
    let base = view_base_href(UiRouteMode::Upload, app_path);
    if let Some(f) = file.map(str::trim).filter(|s| !s.is_empty()) {
        format!("{base}?file={}", encode_query_value(f))
    } else {
        base
    }
}

pub fn app_href(app_path: &str, scene_suffix: &str) -> String {
    format!(
        "{}{}",
        view_base_href(UiRouteMode::App, app_path),
        scene_suffix
    )
}

pub fn app_scene_href(
    app_path: &str,
    scene_id: Option<&str>,
    tab: Option<&str>,
    chrome: Option<&str>,
) -> String {
    app_href(app_path, &access_scene_route_suffix(scene_id, tab, chrome))
}

pub fn presentation_scene_href(app_path: &str, scene_id: Option<&str>) -> String {
    format!(
        "{}{}",
        view_base_href(UiRouteMode::Presentation, app_path),
        access_scene_route_suffix(scene_id, None, None)
    )
}

pub fn cross_app_href(
    view: UiRouteMode,
    app_path: &str,
    catalog: Option<&str>,
    pack: Option<&str>,
) -> String {
    // Stock catalog pack entries share one href shape across all shell modes.
    if catalog.is_some() || pack.is_some() {
        return match view {
            UiRouteMode::Runtime => runtime_href_with_catalog(app_path, None, None, catalog, pack),
            UiRouteMode::Build => build_href_with_catalog(app_path, None, None, catalog, pack),
            _ => build_href_with_catalog(app_path, None, None, catalog, pack),
        };
    }
    match view {
        UiRouteMode::App => app_scene_href(app_path, None, None, None),
        UiRouteMode::Presentation => presentation_scene_href(app_path, None),
        UiRouteMode::Build => build_href_with_catalog(app_path, None, None, catalog, pack),
        UiRouteMode::Config => config_href(app_path),
        UiRouteMode::Upload => upload_href(app_path, None),
        UiRouteMode::Runtime => runtime_href(app_path, None, None),
    }
}

fn append_catalog_query(parts: &mut Vec<String>, catalog: Option<&str>, pack: Option<&str>) {
    if let Some(c) = catalog.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("catalog={}", encode_query_value(c)));
    }
    if let Some(p) = pack.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("pack={}", encode_query_value(p)));
    }
}

pub fn runtime_href_with_catalog(
    app_path: &str,
    node: Option<&str>,
    tab: Option<&str>,
    catalog: Option<&str>,
    pack: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    append_catalog_query(&mut parts, catalog, pack);
    if let Some(n) = node.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("node={}", encode_query_value(n)));
    }
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("tab={}", encode_query_value(t)));
    }
    let base = view_base_href(UiRouteMode::Runtime, app_path);
    if parts.is_empty() {
        base
    } else {
        format!("{base}?{}", parts.join("&"))
    }
}
