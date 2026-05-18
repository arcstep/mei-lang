use mei_lang_kernel::CompiledApp;

use super::compile_status::asset_dual_preview_source;
use super::UiRouteMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManageViewTab {
    Preview,
    Source,
    Diff,
    Diagnostics,
}

impl ManageViewTab {
    pub fn slug(self) -> &'static str {
        match self {
            ManageViewTab::Preview => "preview",
            ManageViewTab::Source => "source",
            ManageViewTab::Diff => "diff",
            ManageViewTab::Diagnostics => "diagnostics",
        }
    }
}

fn manage_tab_from_slug(value: Option<&str>) -> Option<ManageViewTab> {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "preview" => Some(ManageViewTab::Preview),
        "source" => Some(ManageViewTab::Source),
        "diff" => Some(ManageViewTab::Diff),
        "diagnostics" => Some(ManageViewTab::Diagnostics),
        _ => None,
    }
}

/// 若 `target_file` 是某条 scene route 的主文件，返回其 `scene_id`。
pub(super) fn canonical_scene_for_script_target<'a>(
    compiled: &'a CompiledApp,
    target_file: Option<&'a str>,
) -> Option<&'a str> {
    let t = target_file?.trim();
    if t.is_empty() {
        return None;
    }
    compiled
        .scene_routes
        .iter()
        .find(|r| r.target_file == t)
        .map(|r| r.scene_id.as_str())
}

pub(super) fn encode_query_value(value: &str) -> String {
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

pub(super) fn manage_view_tab_from_query(
    active_tab: Option<&str>,
    script_target: bool,
    prefer_diagnostics: bool,
    diagnostics_count: usize,
    selected_target: &str,
) -> ManageViewTab {
    let has_diagnostics_tab = script_target && diagnostics_count > 0;
    let asset_dual = asset_dual_preview_source(selected_target);
    let next = manage_tab_from_slug(active_tab).unwrap_or_else(|| {
        if prefer_diagnostics && has_diagnostics_tab {
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

pub(super) fn manage_tab_href(
    app_path: &str,
    file_param: Option<&str>,
    selected_target: &str,
    script_target: bool,
    tab: ManageViewTab,
) -> String {
    let mut query = Vec::new();
    if let Some(f) = file_param {
        if !f.is_empty() {
            query.push(format!("file={}", encode_query_value(f)));
        }
    }
    let asset_dual = asset_dual_preview_source(selected_target);
    let route_tab = if script_target {
        tab
    } else if asset_dual {
        match tab {
            ManageViewTab::Preview | ManageViewTab::Source => tab,
            _ => ManageViewTab::Preview,
        }
    } else {
        ManageViewTab::Preview
    };
    query.push(format!("tab={}", route_tab.slug()));
    format!("/apps/manage/{app_path}?{}", query.join("&"))
}

/// 访问态 canonical 路径后缀：`/scene/<id>?tab=…&chrome=…`（`scene_id` 经 `encode_query_value` 编码）。
pub(super) fn access_scene_route_suffix(
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
pub(super) fn access_scene_query(selected_scene: Option<&str>) -> String {
    access_scene_route_suffix(selected_scene, None, None)
}

pub(super) fn route_query(
    route_mode: UiRouteMode,
    selected_scene: Option<&str>,
    _preview_target: Option<&str>,
    active_tab: Option<&str>,
) -> String {
    if route_mode == UiRouteMode::Access {
        access_scene_route_suffix(selected_scene, active_tab, None)
    } else {
        String::new()
    }
}
