use super::manage_routing::{access_scene_route_suffix, encode_query_value};
use super::route::UiRouteMode;

pub fn view_base_href(view: UiRouteMode, app_path: &str) -> String {
    format!("/apps/{}/{}", view.slug(), app_path.trim_start_matches('/'))
}

pub fn build_href(app_path: &str, file: Option<&str>, tab: Option<&str>) -> String {
    let mut parts = Vec::new();
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

pub fn cross_app_href(view: UiRouteMode, app_path: &str) -> String {
    match view {
        UiRouteMode::App => app_scene_href(app_path, None, None, None),
        UiRouteMode::Build => build_href(app_path, None, None),
        UiRouteMode::Config => config_href(app_path),
        UiRouteMode::Upload => upload_href(app_path, None),
    }
}
