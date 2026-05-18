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

/// 访问态入口使用的 `?scene=...` 片段：与当前页面是管理态还是访问态无关。
/// 管理壳在同一次 SSR 编译中解析出的 `selected_scene` 写入此处，使「访问」仅携带 `scene=`，
/// 与访问态禁止 `file=` 深链的发布边界一致；不依赖用户事先在 main 中手工登记路由。
pub(super) fn access_scene_query(selected_scene: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(scene) = selected_scene {
        let scene = scene.trim();
        if !scene.is_empty() {
            parts.push(format!("scene={}", encode_query_value(scene)));
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

pub(super) fn route_query(
    route_mode: UiRouteMode,
    selected_scene: Option<&str>,
    _preview_target: Option<&str>,
    _active_tab: Option<&str>,
) -> String {
    if route_mode == UiRouteMode::Access {
        access_scene_query(selected_scene)
    } else {
        String::new()
    }
}
