use super::manage_routing::encode_query_value;
use super::route::UiRouteMode;

pub fn home_href() -> &'static str {
    "/home"
}

pub fn mcg_href(app_path: Option<&str>) -> String {
    match app_path.map(str::trim).filter(|s| !s.is_empty()) {
        Some(app) => format!("/mcg?app={}", encode_query_value(app)),
        None => "/mcg".to_string(),
    }
}

pub fn view_href(
    app_path: &str,
    surface: UiRouteMode,
    scene: Option<&str>,
    _file: Option<&str>,
    _tab: Option<&str>,
    _node: Option<&str>,
    chrome: Option<&str>,
    _catalog: Option<&str>,
    _pack: Option<&str>,
) -> String {
    let app = app_path.trim_start_matches('/');
    let mut parts = vec![format!("surface={}", encode_query_value(surface.slug()))];
    if let Some(scene) = scene.map(str::trim).filter(|value| !value.is_empty()) {
        parts.push(format!("scene={}", encode_query_value(scene)));
    }
    if let Some(c) = chrome.map(str::trim).filter(|value| !value.is_empty()) {
        parts.push(format!("chrome={}", encode_query_value(c)));
    }
    format!("/apps/{app}/view?{}", parts.join("&"))
}

pub fn app_surface_href(app_path: &str, surface: UiRouteMode) -> String {
    view_href(app_path, surface, None, None, None, None, None, None, None)
}

pub fn workspace_surface_href(
    app_path: &str,
    surface: UiRouteMode,
    file: Option<&str>,
    tab: Option<&str>,
    node: Option<&str>,
    catalog: Option<&str>,
    pack: Option<&str>,
) -> String {
    view_href(
        app_path, surface, None, file, tab, node, None, catalog, pack,
    )
}

pub fn app_access_href(app_path: &str) -> String {
    app_surface_href(app_path, UiRouteMode::App)
}

pub fn layout_href(app_path: &str, file: Option<&str>, tab: Option<&str>) -> String {
    workspace_surface_href(app_path, UiRouteMode::Layout, file, tab, None, None, None)
}

pub fn prototype_href(app_path: &str, file: Option<&str>, tab: Option<&str>) -> String {
    workspace_surface_href(
        app_path,
        UiRouteMode::Prototype,
        file,
        tab,
        None,
        None,
        None,
    )
}

#[allow(dead_code)]
pub fn build_href_with_catalog(
    app_path: &str,
    file: Option<&str>,
    tab: Option<&str>,
    _catalog: Option<&str>,
    _pack: Option<&str>,
) -> String {
    layout_href(app_path, file, tab)
}

#[allow(dead_code)]
pub fn build_href_with_catalog_and_axis(
    app_path: &str,
    file: Option<&str>,
    tab: Option<&str>,
    catalog: Option<&str>,
    pack: Option<&str>,
    _axis: &BuildAxisHrefPreset,
) -> String {
    workspace_surface_href(
        app_path,
        UiRouteMode::Layout,
        file,
        tab,
        None,
        catalog,
        pack,
    )
}

pub fn runtime_href(app_path: &str, node: Option<&str>, tab: Option<&str>) -> String {
    host_runtime_href(Some(app_path), node, tab)
}

pub fn config_href(app_path: &str) -> String {
    host_config_href(Some(app_path))
}

pub fn upload_href(app_path: &str, file: Option<&str>) -> String {
    host_upload_href(Some(app_path), file)
}

pub fn host_config_href(app_path: Option<&str>) -> String {
    match app_path.map(str::trim).filter(|s| !s.is_empty()) {
        Some(app) => format!("/config?app={}", encode_query_value(app)),
        None => "/config".to_string(),
    }
}

pub fn host_upload_href(app_path: Option<&str>, file: Option<&str>) -> String {
    let mut base = match app_path.map(str::trim).filter(|s| !s.is_empty()) {
        Some(app) => format!("/upload?app={}", encode_query_value(app)),
        None => "/upload".to_string(),
    };
    if let Some(f) = file.map(str::trim).filter(|s| !s.is_empty()) {
        base = format!("{base}&file={}", encode_query_value(f));
    }
    base
}

pub fn host_runtime_href(app_path: Option<&str>, node: Option<&str>, tab: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(app) = app_path.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("app={}", encode_query_value(app)));
    }
    if let Some(n) = node.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("node={}", encode_query_value(n)));
    }
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("tab={}", encode_query_value(t)));
    }
    if parts.is_empty() {
        "/runtime".to_string()
    } else {
        format!("/runtime?{}", parts.join("&"))
    }
}

#[allow(dead_code)]
pub fn app_href(app_path: &str, scene_suffix: &str) -> String {
    format!(
        "{}{}",
        app_surface_href(app_path, UiRouteMode::App),
        scene_suffix
    )
}

pub fn app_scene_href(
    app_path: &str,
    scene_id: Option<&str>,
    tab: Option<&str>,
    chrome: Option<&str>,
    _data_mode: Option<&str>,
    _review_projection: Option<&str>,
) -> String {
    view_href(
        app_path,
        UiRouteMode::App,
        scene_id,
        None,
        tab,
        None,
        chrome,
        None,
        None,
    )
}

pub fn copilot_presentation_href(app_path: &str, _presentation_id: &str) -> String {
    app_access_href(app_path)
}

/// 兼容旧链接：`/apps/speaker/.../tour/...`。
#[allow(dead_code)]
pub fn speaker_tour_href(app_path: &str, tour_id: &str) -> String {
    copilot_presentation_href(app_path, tour_id)
}

pub fn cross_app_href(
    view: UiRouteMode,
    app_path: &str,
    catalog: Option<&str>,
    pack: Option<&str>,
) -> String {
    if catalog.is_some() || pack.is_some() {
        return match view {
            UiRouteMode::Runtime => runtime_href_with_catalog(app_path, None, None, catalog, pack),
            UiRouteMode::Prototype => workspace_surface_href(
                app_path,
                UiRouteMode::Prototype,
                None,
                None,
                None,
                catalog,
                pack,
            ),
            UiRouteMode::Layout => workspace_surface_href(
                app_path,
                UiRouteMode::Layout,
                None,
                None,
                None,
                catalog,
                pack,
            ),
            _ => workspace_surface_href(
                app_path,
                UiRouteMode::Layout,
                None,
                None,
                None,
                catalog,
                pack,
            ),
        };
    }
    match view {
        UiRouteMode::App => app_access_href(app_path),
        UiRouteMode::Layout => layout_href(app_path, None, None),
        UiRouteMode::Prototype => prototype_href(app_path, None, None),
        UiRouteMode::Run | UiRouteMode::Copilot => app_access_href(app_path),
        UiRouteMode::Config => host_config_href(Some(app_path)),
        UiRouteMode::Upload => host_upload_href(Some(app_path), None),
        UiRouteMode::Runtime => host_runtime_href(Some(app_path), None, None),
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
    let base = host_runtime_href(Some(app_path), None, None);
    if parts.is_empty() {
        base
    } else if base.contains('?') {
        format!("{base}&{}", parts.join("&"))
    } else {
        format!("{base}?{}", parts.join("&"))
    }
}

/// Legacy build-axis href preset (compat redirects).
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct BuildAxisHrefPreset {
    pub data_mode: Option<String>,
    pub review_projection: Option<String>,
    pub tree_max_ui_role: Option<String>,
    pub compile_view: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_surface_hrefs_use_unified_view_route() {
        assert_eq!(
            app_access_href("pretty-panels"),
            "/apps/pretty-panels/view?surface=app"
        );
        assert_eq!(
            layout_href("pretty-panels", Some("main.mei"), Some("preview")),
            "/apps/pretty-panels/view?surface=layout"
        );
        assert_eq!(
            prototype_href("demo", None, None),
            "/apps/demo/view?surface=prototype"
        );
    }

    #[test]
    fn app_scene_href_omits_review_axes() {
        let href = app_scene_href(
            "demo",
            Some("home"),
            Some("preview"),
            None,
            Some("static"),
            Some("plane_region_section"),
        );
        assert!(href.starts_with("/apps/demo/view?surface=app&scene=home"));
        assert!(!href.contains("review_projection"));
        assert!(!href.contains("data_mode"));
    }
}
