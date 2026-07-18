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

/// Access 规范路径：`/apps/{app}/{stage}`（与顶栏舞台菜单一致）。
pub fn app_stage_href(app_path: &str, stage_id: &str, chrome: Option<&str>) -> String {
    let app = app_path.trim_start_matches('/');
    let stage = stage_id.trim();
    let stage = if stage.is_empty() { "home" } else { stage };
    let mut href = format!("/apps/{app}/{stage}");
    if let Some(c) = chrome.map(str::trim).filter(|value| !value.is_empty()) {
        if c != "full" {
            href.push_str(&format!("?chrome={}", encode_query_value(c)));
        }
    }
    href
}

/// 无舞台 id 时默认 `home`（菜单/首页应尽量写满真实 `default_stage`）。
pub fn app_access_href(app_path: &str) -> String {
    app_access_href_with_stage(app_path, None)
}

/// Access 入口；`default_stage` 缺省时回退 `home`。
pub fn app_access_href_with_stage(app_path: &str, default_stage: Option<&str>) -> String {
    let stage = default_stage
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home");
    app_stage_href(app_path, stage, None)
}

/// 布局/原型产品入口已封口：href 落到 Access 默认舞台。
pub fn layout_href(app_path: &str, _file: Option<&str>, _tab: Option<&str>) -> String {
    app_access_href(app_path)
}

pub fn prototype_href(app_path: &str, _file: Option<&str>, _tab: Option<&str>) -> String {
    app_access_href(app_path)
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
    _catalog: Option<&str>,
    _pack: Option<&str>,
    _axis: &BuildAxisHrefPreset,
) -> String {
    layout_href(app_path, file, tab)
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
    format!("{}{}", app_access_href(app_path), scene_suffix)
}

pub fn app_scene_href(
    app_path: &str,
    scene_id: Option<&str>,
    _tab: Option<&str>,
    chrome: Option<&str>,
    _data_mode: Option<&str>,
    _review_projection: Option<&str>,
) -> String {
    let stage = scene_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("home");
    app_stage_href(app_path, stage, chrome)
}

pub fn copilot_presentation_href(app_path: &str, _presentation_id: &str) -> String {
    app_access_href(app_path)
}

/// 兼容旧链接：`/apps/speaker/.../tour/...`。
#[allow(dead_code)]
pub fn speaker_tour_href(app_path: &str, tour_id: &str) -> String {
    copilot_presentation_href(app_path, tour_id)
}

/// 跨应用顶栏入口；`default_stage` 来自目标应用 `app.toml`（如 mei-tutorial → `intro`）。
/// 缺省舞台时回退 `home`。
pub fn cross_app_href(
    view: UiRouteMode,
    app_path: &str,
    catalog: Option<&str>,
    pack: Option<&str>,
    default_stage: Option<&str>,
) -> String {
    if catalog.is_some() || pack.is_some() {
        return match view {
            UiRouteMode::Runtime => runtime_href_with_catalog(app_path, None, None, catalog, pack),
            // 布局/原型已封口 → Access
            _ => app_access_href_with_stage(app_path, default_stage),
        };
    }
    match view {
        UiRouteMode::App | UiRouteMode::Layout | UiRouteMode::Prototype => {
            app_access_href_with_stage(app_path, default_stage)
        }
        UiRouteMode::Run | UiRouteMode::Copilot => {
            app_access_href_with_stage(app_path, default_stage)
        }
        // Admin / Config / Upload：App Switcher 始终回 Access Stage（0544 §4.1）。
        UiRouteMode::Admin | UiRouteMode::Config | UiRouteMode::Upload => {
            app_access_href_with_stage(app_path, default_stage)
        }
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
    fn app_hrefs_use_stage_path() {
        assert_eq!(app_access_href("pretty-panels"), "/apps/pretty-panels/home");
        assert_eq!(
            layout_href("pretty-panels", Some("main.mei"), Some("preview")),
            "/apps/pretty-panels/home"
        );
        assert_eq!(prototype_href("demo", None, None), "/apps/demo/home");
        assert_eq!(
            app_access_href_with_stage("mei-tutorial", Some("intro")),
            "/apps/mei-tutorial/intro"
        );
        assert_eq!(
            cross_app_href(UiRouteMode::App, "mei-tutorial", None, None, Some("intro")),
            "/apps/mei-tutorial/intro"
        );
        assert_eq!(
            cross_app_href(UiRouteMode::Admin, "mini-data", None, None, Some("home")),
            "/apps/mini-data/home"
        );
        assert_eq!(
            cross_app_href(UiRouteMode::Config, "mini-data", None, None, Some("home")),
            "/apps/mini-data/home"
        );
    }

    #[test]
    fn app_scene_href_uses_stage_segment() {
        let href = app_scene_href(
            "demo",
            Some("home"),
            Some("preview"),
            None,
            Some("static"),
            Some("plane_region_section"),
        );
        assert_eq!(href, "/apps/demo/home");
        let supervision = app_scene_href("mini-data", Some("supervision"), None, None, None, None);
        assert_eq!(supervision, "/apps/mini-data/supervision");
        let chrome_none = app_scene_href("demo", Some("home"), None, Some("none"), None, None);
        assert_eq!(chrome_none, "/apps/demo/home?chrome=none");
    }
}
