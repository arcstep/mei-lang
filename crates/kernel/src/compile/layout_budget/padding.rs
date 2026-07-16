//! Hierarchy spacing defaults keyed by Stage Profile (0332 §6.0).
//!
//! Cascade: omit-inject only. Author-explicit gap/padding/`"0"` always wins.
//! Cockpit values remain the 1px hierarchy unit; slides/page use profile tables.

use crate::model::{ProfileLayoutPolicy, ProfileSpacingTokens, StageProfile};

/// 层级默认边距：字面 `1px`（cockpit region / section / section 内网格）。
pub const HIERARCHY_PX_1: &str = "1px";
/// 历史名「space_1」：现与 [`HIERARCHY_PX_1`] 对齐（不再表示 4px）。
pub const HIERARCHY_SPACE_1: &str = HIERARCHY_PX_1;
/// section 外边距：落在父 region `layout.gap`（cockpit）。
pub const HIERARCHY_SECTION_OUTER: &str = HIERARCHY_PX_1;
/// region 外边距：落在父 plane `layout.gap`（cockpit）。
pub const HIERARCHY_REGION_OUTER: &str = HIERARCHY_PX_1;

pub fn padding_profile_css(profile: &str) -> Option<&'static str> {
    match profile {
        "dense_strip_100" => Some("8px 4px 2px 4px"),
        "compact_ai" => Some("8px 6px 3px 6px"),
        "compact" => Some("8px 6px 6px 6px"),
        "dense" => Some("8px 4px 4px 4px"),
        // 层级默认内边距（section_shell 默认 profile）；cockpit 字面 1px
        "space_1" => Some(HIERARCHY_PX_1),
        "none" => Some("0"),
        _ => None,
    }
}

/// plane / region / section / content 省略时的默认内外边距（cockpit 表）。
///
/// Prefer [`hierarchy_spacing_defaults_for`] when the owning Stage Profile is known.
pub fn hierarchy_spacing_defaults(role: &str) -> Option<HierarchySpacingDefaults> {
    hierarchy_spacing_defaults_for(role, StageProfile::Cockpit)
}

/// Profile-aware hierarchy spacing (omit-inject).
pub fn hierarchy_spacing_defaults_for(
    role: &str,
    profile: StageProfile,
) -> Option<HierarchySpacingDefaults> {
    let tokens = ProfileLayoutPolicy::for_profile(profile).spacing;
    hierarchy_spacing_from_tokens(role, tokens)
}

fn hierarchy_spacing_from_tokens(
    role: &str,
    tokens: ProfileSpacingTokens,
) -> Option<HierarchySpacingDefaults> {
    match role {
        "plane" => Some(HierarchySpacingDefaults {
            gap: Some(tokens.plane_gap),
            padding: Some(tokens.plane_padding),
        }),
        "region" => Some(HierarchySpacingDefaults {
            gap: Some(tokens.region_gap),
            padding: Some(tokens.region_padding),
        }),
        "section" => Some(HierarchySpacingDefaults {
            gap: Some(tokens.section_gap),
            padding: Some(tokens.section_padding),
        }),
        "content" => Some(HierarchySpacingDefaults {
            gap: Some(tokens.content_gap),
            padding: Some(tokens.content_padding),
        }),
        "slide" => Some(HierarchySpacingDefaults {
            gap: Some("0"),
            padding: Some(tokens.slide_shell_padding),
        }),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HierarchySpacingDefaults {
    pub gap: Option<&'static str>,
    pub padding: Option<&'static str>,
}
