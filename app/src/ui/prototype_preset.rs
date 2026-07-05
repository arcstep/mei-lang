//! Workspace surface presets: layout (structure) vs prototype (static draft).

use mei_lang_kernel::{tabs_for_node_kind, BuildNodeKind, BuildViewTab};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrototypePreset {
    pub slug: &'static str,
    pub label: &'static str,
    pub data_mode: &'static str,
    pub review_projection: &'static str,
    /// Default ui_structure tree expand depth.
    pub tree_max_ui_role: &'static str,
}

pub const LAYOUT_PRESET: PrototypePreset = PrototypePreset {
    slug: "layout",
    label: "布局",
    data_mode: "static",
    review_projection: "plane_region_section",
    tree_max_ui_role: "section",
};

pub const PROTOTYPE_SURFACE_PRESET: PrototypePreset = PrototypePreset {
    slug: "prototype",
    label: "原型",
    data_mode: "static",
    review_projection: "static_full",
    tree_max_ui_role: "plane",
};

pub const PROTOTYPE_PRESETS: &[PrototypePreset] = &[LAYOUT_PRESET, PROTOTYPE_SURFACE_PRESET];

pub fn preset_for_route_mode(route_mode: crate::ui::route::UiRouteMode) -> Option<&'static PrototypePreset> {
    use crate::ui::route::UiRouteMode;
    match route_mode {
        UiRouteMode::Layout => Some(&LAYOUT_PRESET),
        UiRouteMode::Prototype => Some(&PROTOTYPE_SURFACE_PRESET),
        _ => None,
    }
}

pub fn match_preset(data_mode: &str, review_projection: &str) -> Option<&'static PrototypePreset> {
    let dm = data_mode.trim();
    let rp = review_projection.trim();
    PROTOTYPE_PRESETS
        .iter()
        .find(|preset| preset.data_mode == dm && preset.review_projection == rp)
}

pub fn default_build_preset() -> &'static PrototypePreset {
    &LAYOUT_PRESET
}

pub fn preset_tree_max_ui_role(data_mode: &str, review_projection: &str) -> &'static str {
    match_preset(data_mode, review_projection)
        .map(|preset| preset.tree_max_ui_role)
        .unwrap_or_else(|| default_build_preset().tree_max_ui_role)
}

/// Tabs removed from the prototype (Build) workspace — no nav entry or panel.
pub fn prototype_workspace_retired_tab(tab: BuildViewTab) -> bool {
    matches!(
        tab,
        BuildViewTab::Overview
            | BuildViewTab::Preview
            | BuildViewTab::Provenance
            | BuildViewTab::Agent
    )
}

/// Map legacy tab query to the active prototype surface (usually Preview).
pub fn prototype_normalize_workspace_tab(_kind: BuildNodeKind, tab: BuildViewTab) -> BuildViewTab {
    if prototype_workspace_retired_tab(tab) {
        BuildViewTab::Preview
    } else {
        tab
    }
}

/// Primary workspace tabs (empty: preview is the default surface without a tab chip).
pub fn prototype_workspace_primary_tabs(_kind: BuildNodeKind) -> Vec<BuildViewTab> {
    Vec::new()
}

/// Remaining specialist tabs (e.g. 执行 / 语义图) when the node kind still needs them.
pub fn prototype_workspace_tool_tabs(kind: BuildNodeKind) -> Vec<BuildViewTab> {
    let all = tabs_for_node_kind(kind);
    let primary = prototype_workspace_primary_tabs(kind);
    all.iter()
        .copied()
        .filter(|tab| !primary.contains(tab))
        .filter(|tab| !prototype_workspace_retired_tab(*tab))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_and_prototype_presets_map_to_review_axes() {
        assert_eq!(default_build_preset().slug, "layout");
        let proto = match_preset("static", "static_full").expect("prototype preset");
        assert_eq!(proto.slug, "prototype");
    }

    #[test]
    fn scene_node_has_no_visible_workspace_tabs() {
        let primary = prototype_workspace_primary_tabs(BuildNodeKind::Scene);
        assert!(primary.is_empty());
        let tools = prototype_workspace_tool_tabs(BuildNodeKind::Scene);
        assert!(tools.is_empty());
        assert_eq!(
            prototype_normalize_workspace_tab(BuildNodeKind::Scene, BuildViewTab::Overview),
            BuildViewTab::Preview
        );
    }

    #[test]
    fn projection_shows_semantic_as_tool_tab() {
        let tools = prototype_workspace_tool_tabs(BuildNodeKind::Projection);
        assert_eq!(tools, vec![BuildViewTab::Semantic]);
    }
}
