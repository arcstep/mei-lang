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

pub(super) fn manage_view_tab_from_query(
    active_tab: Option<&str>,
    script_target: bool,
    prefer_diagnostics: bool,
) -> ManageViewTab {
    let next = manage_tab_from_slug(active_tab).unwrap_or_else(|| {
        if prefer_diagnostics {
            ManageViewTab::Diagnostics
        } else {
            ManageViewTab::Preview
        }
    });
    if script_target {
        next
    } else {
        ManageViewTab::Preview
    }
}

pub(super) fn manage_tab_href(
    app_path: &str,
    target: &str,
    selected_entry: Option<&str>,
    _preview_target: Option<&str>,
    script_target: bool,
    tab: ManageViewTab,
) -> String {
    let mut query = vec![format!("target={target}")];
    if script_target {
        if let Some(entry) = selected_entry {
            query.push(format!("entry={entry}"));
        }
    } else if let Some(entry) = selected_entry {
        query.push(format!("entry={entry}"));
    }
    let route_tab = if script_target {
        tab
    } else {
        ManageViewTab::Preview
    };
    query.push(format!("tab={}", route_tab.slug()));
    format!("/apps/manage/{app_path}?{}", query.join("&"))
}

pub(super) fn route_query(
    selected_entry: Option<&str>,
    _preview_target: Option<&str>,
    _active_tab: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    if let Some(entry) = selected_entry {
        parts.push(format!("entry={entry}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}
