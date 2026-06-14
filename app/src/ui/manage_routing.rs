use super::compile_status::{asset_dual_preview_source, is_world_capsule_target};
use super::UiRouteMode;

pub const OPS_CONFIG_TARGET: &str = ".mei-config.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManageViewTab {
    Preview,
    Source,
    Diagnostics,
}

impl ManageViewTab {
    pub fn slug(self) -> &'static str {
        match self {
            ManageViewTab::Preview => "preview",
            ManageViewTab::Source => "source",
            ManageViewTab::Diagnostics => "diagnostics",
        }
    }
}

fn manage_tab_from_slug(value: Option<&str>) -> Option<ManageViewTab> {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "preview" => Some(ManageViewTab::Preview),
        "source" => Some(ManageViewTab::Source),
        "diff" => Some(ManageViewTab::Source),
        "diagnostics" => Some(ManageViewTab::Diagnostics),
        _ => None,
    }
}

pub(crate) fn is_ops_config_target(target: &str) -> bool {
    target.trim() == OPS_CONFIG_TARGET
}

pub(crate) fn encode_query_value(value: &str) -> String {
    let mut out = String::new();
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*b))
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

pub(crate) fn manage_view_tab_from_query(
    active_tab: Option<&str>,
    script_target: bool,
    prefer_diagnostics: bool,
    diagnostics_count: usize,
    selected_target: &str,
) -> ManageViewTab {
    if is_ops_config_target(selected_target) {
        return ManageViewTab::Preview;
    }
    let has_diagnostics_tab = script_target && diagnostics_count > 0;
    let asset_dual = asset_dual_preview_source(selected_target);
    let next = manage_tab_from_slug(active_tab).unwrap_or_else(|| {
        if script_target && is_world_capsule_target(selected_target) {
            ManageViewTab::Source
        } else if prefer_diagnostics && has_diagnostics_tab {
            ManageViewTab::Diagnostics
        } else {
            ManageViewTab::Preview
        }
    });
    if script_target {
        if matches!(next, ManageViewTab::Diagnostics) && !has_diagnostics_tab {
            return ManageViewTab::Preview;
        }
        next
    } else if asset_dual {
        match next {
            ManageViewTab::Source => ManageViewTab::Source,
            _ => ManageViewTab::Preview,
        }
    } else {
        ManageViewTab::Preview
    }
}

pub(crate) fn manage_tab_href(
    app_path: &str,
    file_param: Option<&str>,
    selected_target: &str,
    script_target: bool,
    tab: ManageViewTab,
    diag_filter: Option<&str>,
    selected_scene: Option<&str>,
) -> String {
    let asset_dual = asset_dual_preview_source(selected_target);
    let route_tab = if is_ops_config_target(selected_target) {
        ManageViewTab::Preview
    } else if script_target {
        tab
    } else if asset_dual {
        match tab {
            ManageViewTab::Preview | ManageViewTab::Source => tab,
            _ => ManageViewTab::Preview,
        }
    } else {
        ManageViewTab::Preview
    };
    let diag = if matches!(route_tab, ManageViewTab::Diagnostics) {
        diag_filter
    } else {
        None
    };
    build_preview_href(
        app_path,
        file_param,
        selected_scene,
        Some(route_tab.slug()),
        diag,
    )
}

/// 构建视图预览链接：`file` + 可选 `scene`（多 `scene_export` 选择）+ `tab`。
pub(crate) fn build_preview_href(
    app_path: &str,
    file: Option<&str>,
    scene: Option<&str>,
    tab: Option<&str>,
    diag_filter: Option<&str>,
) -> String {
    let query = build_preview_query_parts(file, scene, tab, diag_filter);
    if query.is_empty() {
        format!("/apps/build/{app_path}")
    } else {
        format!("/apps/build/{app_path}?{}", query.join("&"))
    }
}

pub(crate) fn build_preview_query_parts(
    file: Option<&str>,
    scene: Option<&str>,
    tab: Option<&str>,
    diag_filter: Option<&str>,
) -> Vec<String> {
    let mut query = Vec::new();
    if let Some(f) = file.map(str::trim).filter(|s| !s.is_empty()) {
        query.push(format!("file={}", encode_query_value(f)));
    }
    if let Some(sc) = scene.map(str::trim).filter(|s| !s.is_empty()) {
        query.push(format!("scene={}", encode_query_value(sc)));
    }
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        query.push(format!("tab={}", encode_query_value(t)));
    }
    if let Some(filter) = diag_filter.map(str::trim).filter(|s| !s.is_empty()) {
        if filter.eq_ignore_ascii_case("all") {
            query.push("diag_filter=all".to_string());
        }
    }
    query
}

/// 访问态 canonical 路径后缀：`/scene/<id>?tab=…&chrome=…`（`scene_id` 经 `encode_query_value` 编码）。
pub(crate) fn access_scene_route_suffix(
    selected_scene: Option<&str>,
    tab: Option<&str>,
    chrome: Option<&str>,
) -> String {
    let mut out = String::new();
    if let Some(sc) = selected_scene.map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str("/scene/");
        out.push_str(&encode_query_value(sc));
    }
    let mut q = Vec::new();
    if let Some(t) = tab.map(str::trim).filter(|s| !s.is_empty()) {
        q.push(format!("tab={}", encode_query_value(t)));
    }
    if let Some(c) = chrome.map(str::trim).filter(|s| !s.is_empty()) {
        q.push(format!("chrome={}", encode_query_value(c)));
    }
    if !q.is_empty() {
        if out.is_empty() {
            out.push('?');
        } else {
            out.push('?');
        }
        out.push_str(&q.join("&"));
    }
    out
}

/// 访问态入口使用的路径后缀；无 scene 时返回空串（由调用方决定是否禁用「访问」按钮）。
pub(crate) fn access_scene_query(selected_scene: Option<&str>) -> String {
    access_scene_route_suffix(selected_scene, None, None)
}

#[allow(dead_code)]
pub(crate) fn route_query(
    route_mode: UiRouteMode,
    selected_scene: Option<&str>,
    _preview_target: Option<&str>,
    active_tab: Option<&str>,
) -> String {
    if route_mode.uses_scene_route() {
        access_scene_route_suffix(selected_scene, active_tab, None)
    } else {
        String::new()
    }
}
