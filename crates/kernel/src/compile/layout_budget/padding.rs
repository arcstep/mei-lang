/// 层级默认边距：字面 `1px`（region / section / section 内网格）。
pub const HIERARCHY_PX_1: &str = "1px";
/// 历史名「space_1」：现与 [`HIERARCHY_PX_1`] 对齐（不再表示 4px）。
pub const HIERARCHY_SPACE_1: &str = HIERARCHY_PX_1;
/// section 外边距：落在父 region `layout.gap`。
pub const HIERARCHY_SECTION_OUTER: &str = HIERARCHY_PX_1;
/// region 外边距：落在父 plane `layout.gap`。
pub const HIERARCHY_REGION_OUTER: &str = HIERARCHY_PX_1;

pub fn padding_profile_css(profile: &str) -> Option<&'static str> {
    match profile {
        "dense_strip_100" => Some("8px 4px 2px 4px"),
        "compact_ai" => Some("8px 6px 3px 6px"),
        "compact" => Some("8px 6px 6px 6px"),
        "dense" => Some("8px 4px 4px 4px"),
        // 层级默认内边距（section_shell 默认 profile）
        "space_1" => Some(HIERARCHY_PX_1),
        "none" => Some("0"),
        _ => None,
    }
}

/// plane / region / section / content（section 内网格）省略时的默认内外边距。
/// 外边距语义落在**父级** `layout.gap`；本函数返回「当前节点应注入的 gap / padding」。
pub fn hierarchy_spacing_defaults(role: &str) -> Option<HierarchySpacingDefaults> {
    match role {
        "plane" => Some(HierarchySpacingDefaults {
            // plane.gap → region 外边距 1px
            gap: Some(HIERARCHY_REGION_OUTER),
            padding: Some(HIERARCHY_PX_1),
        }),
        "region" => Some(HierarchySpacingDefaults {
            // region.gap → section 外边距 1px；内边距 1px；边线 0（chrome 另注）
            gap: Some(HIERARCHY_SECTION_OUTER),
            padding: Some(HIERARCHY_PX_1),
        }),
        "section" => Some(HierarchySpacingDefaults {
            // section.gap → 内网格外边距 1px；内边距 1px；边线 1px（chrome 另注）
            gap: Some(HIERARCHY_PX_1),
            padding: Some(HIERARCHY_PX_1),
        }),
        "content" => Some(HierarchySpacingDefaults {
            // section 内网格：外边距由 section.gap 承担；内边距 0；边线 0
            gap: Some("0"),
            padding: Some("0"),
        }),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HierarchySpacingDefaults {
    pub gap: Option<&'static str>,
    pub padding: Option<&'static str>,
}
