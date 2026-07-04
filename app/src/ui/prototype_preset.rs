//! Prototype workspace task presets: product-facing mapping to `data_mode × review_projection`.

use mei_lang_kernel::{tabs_for_node_kind, BuildNodeKind, BuildViewTab};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrototypePreset {
    pub slug: &'static str,
    pub label: &'static str,
    pub data_mode: &'static str,
    pub review_projection: &'static str,
    /// Default ui_structure tree expand depth (`region` / `section` / `content`).
    pub tree_max_ui_role: &'static str,
}

pub const PROTOTYPE_PRESETS: &[PrototypePreset] = &[
    PrototypePreset {
        slug: "full_eval",
        label: "完整求值",
        data_mode: "eval",
        review_projection: "live_full",
        tree_max_ui_role: "content",
    },
    PrototypePreset {
        slug: "fixture_preview",
        label: "样例预览",
        data_mode: "fixture",
        review_projection: "static_full",
        tree_max_ui_role: "content",
    },
    PrototypePreset {
        slug: "region_layout",
        label: "Region 布局",
        data_mode: "static",
        review_projection: "plane_region",
        tree_max_ui_role: "region",
    },
    PrototypePreset {
        slug: "section_layout",
        label: "Section 布局",
        data_mode: "static",
        review_projection: "plane_region_section",
        tree_max_ui_role: "section",
    },
];

pub fn match_preset(data_mode: &str, review_projection: &str) -> Option<&'static PrototypePreset> {
    let dm = data_mode.trim();
    let rp = review_projection.trim();
    PROTOTYPE_PRESETS
        .iter()
        .find(|preset| preset.data_mode == dm && preset.review_projection == rp)
}

pub fn default_build_preset() -> &'static PrototypePreset {
    &PROTOTYPE_PRESETS[3]
}

pub fn preset_tree_max_ui_role(data_mode: &str, review_projection: &str) -> &'static str {
    match_preset(data_mode, review_projection)
        .map(|preset| preset.tree_max_ui_role)
        .unwrap_or_else(|| default_build_preset().tree_max_ui_role)
}

/// Primary workspace tabs: preview-first task surface.
pub fn prototype_workspace_primary_tabs(kind: BuildNodeKind) -> Vec<BuildViewTab> {
    let all = tabs_for_node_kind(kind);
    if all.contains(&BuildViewTab::Preview) {
        vec![BuildViewTab::Preview]
    } else {
        all.first().copied().into_iter().collect()
    }
}

/// Secondary tools demoted from the main tab strip.
pub fn prototype_workspace_tool_tabs(kind: BuildNodeKind) -> Vec<BuildViewTab> {
    let all = tabs_for_node_kind(kind);
    let primary = prototype_workspace_primary_tabs(kind);
    all.iter()
        .copied()
        .filter(|tab| !primary.contains(tab))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_map_to_review_axes() {
        let preset = match_preset("eval", "live_full").expect("full eval preset");
        assert_eq!(preset.slug, "full_eval");
        assert_eq!(default_build_preset().data_mode, "static");
        assert_eq!(default_build_preset().review_projection, "plane_region_section");
    }

    #[test]
    fn scene_node_preview_is_primary_tab() {
        let primary = prototype_workspace_primary_tabs(BuildNodeKind::Scene);
        assert_eq!(primary, vec![BuildViewTab::Preview]);
        let tools = prototype_workspace_tool_tabs(BuildNodeKind::Scene);
        assert!(tools.contains(&BuildViewTab::Overview));
        assert!(tools.contains(&BuildViewTab::Agent));
    }
}
