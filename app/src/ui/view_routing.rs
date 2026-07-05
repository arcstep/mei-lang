use super::manage_routing::{access_scene_route_suffix, encode_query_value};
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

pub fn view_base_href(view: UiRouteMode, app_path: &str) -> String {
    format!("/apps/{}/{}", view.slug(), app_path.trim_start_matches('/'))
}

#[derive(Debug, Clone, Default)]
pub struct BuildAxisHrefPreset {
    pub data_mode: Option<String>,
    pub review_projection: Option<String>,
    pub tree_max_ui_role: Option<String>,
}

impl BuildAxisHrefPreset {
    pub fn path_suffix(&self) -> String {
        let mut segments = Vec::new();
        if self.data_mode.as_deref() == Some("eval") {
            segments.push("eval");
        }
        if self.review_projection.as_deref() == Some("plane_region") {
            segments.push("region");
        }
        if self.tree_max_ui_role.as_deref() == Some("content") {
            segments.push("content");
        }
        if segments.is_empty() {
            String::new()
        } else {
            format!("/{}", segments.join("/"))
        }
    }
}

pub fn build_href_with_catalog(
    app_path: &str,
    file: Option<&str>,
    tab: Option<&str>,
    catalog: Option<&str>,
    pack: Option<&str>,
) -> String {
    build_href_with_catalog_and_axis(app_path, file, tab, catalog, pack, &BuildAxisHrefPreset::default())
}

pub fn build_href_with_catalog_and_axis(
    app_path: &str,
    file: Option<&str>,
    tab: Option<&str>,
    catalog: Option<&str>,
    pack: Option<&str>,
    axis: &BuildAxisHrefPreset,
) -> String {
    let mut parts = Vec::new();
    append_catalog_query(&mut parts, catalog, pack);
    if let Some(f) = file.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("file={}", encode_query_value(f)));
    }
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("tab={}", encode_query_value(t)));
    }
    if let Some(dm) = axis
        .data_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("data_mode={}", encode_query_value(dm)));
    }
    if let Some(rp) = axis
        .review_projection
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("review_projection={}", encode_query_value(rp)));
    }
    if let Some(tree_max) = axis
        .tree_max_ui_role
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("tree_max={}", encode_query_value(tree_max)));
    }
    let base = format!(
        "/apps/build/{}{}",
        app_path.trim_start_matches('/'),
        axis.path_suffix()
    );
    if parts.is_empty() {
        base
    } else {
        format!("{base}?{}", parts.join("&"))
    }
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

pub fn app_access_href(app_path: &str) -> String {
    format!("/apps/app/{}/access", app_path.trim_start_matches('/'))
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
    data_mode: Option<&str>,
    review_projection: Option<&str>,
) -> String {
    let suffix = access_scene_route_suffix(
        scene_id,
        tab,
        chrome,
        data_mode,
        review_projection,
    );
    if suffix.is_empty() {
        app_access_href(app_path)
    } else {
        app_href(app_path, &suffix)
    }
}

pub fn run_scene_href(
    app_path: &str,
    scene_id: Option<&str>,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
) -> String {
    format!(
        "{}{}",
        view_base_href(UiRouteMode::Run, app_path),
        access_scene_route_suffix(scene_id, None, None, data_mode, review_projection)
    )
}

/// 兼容旧链接：`/apps/presentation/...` 与 `/apps/run/...` 等价。
pub fn presentation_scene_href(
    app_path: &str,
    scene_id: Option<&str>,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
) -> String {
    run_scene_href(app_path, scene_id, data_mode, review_projection)
}

pub fn copilot_presentation_href(app_path: &str, presentation_id: &str) -> String {
    format!(
        "{}/presentation/{}",
        view_base_href(UiRouteMode::Copilot, app_path),
        encode_query_value(presentation_id.trim())
    )
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
            UiRouteMode::Build => build_href_with_catalog(app_path, None, None, catalog, pack),
            _ => build_href_with_catalog(app_path, None, None, catalog, pack),
        };
    }
    match view {
        UiRouteMode::App => app_access_href(app_path),
        UiRouteMode::Run => run_scene_href(app_path, None, None, None),
        UiRouteMode::Copilot => copilot_presentation_href(app_path, "intro"),
        UiRouteMode::Build => build_href_with_catalog(app_path, None, None, catalog, pack),
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

#[cfg(test)]
mod tests {
    use super::{app_access_href, copilot_presentation_href, speaker_tour_href, BuildAxisHrefPreset};

    #[test]
    fn host_config_href_uses_shell_route() {
        assert_eq!(super::host_config_href(None), "/config");
        assert_eq!(
            super::host_config_href(Some("pretty-panels")),
            "/config?app=pretty-panels"
        );
    }

    #[test]
    fn host_runtime_href_uses_shell_route() {
        assert_eq!(
            super::host_runtime_href(Some("mini-park"), Some("node-1"), Some("json")),
            "/runtime?app=mini-park&node=node-1&tab=json"
        );
    }

    #[test]
    fn build_axis_path_suffix_eval_content() {
        let axis = BuildAxisHrefPreset {
            data_mode: Some("eval".to_string()),
            tree_max_ui_role: Some("content".to_string()),
            ..Default::default()
        };
        assert_eq!(axis.path_suffix(), "/eval/content");
        assert!(super::build_href_with_catalog_and_axis(
            "pretty-panels",
            None,
            Some("preview"),
            None,
            None,
            &axis,
        )
        .contains("/apps/build/pretty-panels/eval/content?tab=preview"));
    }

    #[test]
    fn app_access_href_is_canonical_entry() {
        assert_eq!(app_access_href("demo"), "/apps/app/demo/access");
    }

    #[test]
    fn legacy_config_href_aliases_shell_route() {
        assert_eq!(super::config_href("data-demo"), "/config?app=data-demo");
    }

    #[test]
    fn speaker_tour_href_aliases_copilot_presentation() {
        assert_eq!(
            speaker_tour_href("mini-park", "intro"),
            copilot_presentation_href("mini-park", "intro")
        );
    }
}
