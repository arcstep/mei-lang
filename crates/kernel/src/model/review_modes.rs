//! Host review axes: data mode ceiling and review projection (0508).

/// Process-level maximum data capability (`eval > fixture > static`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DataModeCeiling {
    #[default]
    Eval,
    Fixture,
    Static,
}

/// Effective data mode for a page or API request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DataMode {
    #[default]
    Eval,
    Fixture,
    Static,
}

/// Depth of scene contract projection for Build / review surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ReviewProjection {
    Plane,
    PlaneRegion,
    PlaneRegionSection,
    PlaneRegionSectionSlot,
    #[default]
    StaticFull,
    LiveFull,
}

/// Host surface preview fill policy (orthogonal to `review_projection` depth).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfacePreviewPolicy {
    AppLive,
    LayoutSlotSandbox,
    PrototypeStaticFull,
}

impl SurfacePreviewPolicy {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "app_live" | "app" => Some(Self::AppLive),
            "layout_slot_sandbox" | "layout_sandbox" | "layout" => Some(Self::LayoutSlotSandbox),
            "prototype_static_full" | "prototype" => Some(Self::PrototypeStaticFull),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::AppLive => "app_live",
            Self::LayoutSlotSandbox => "layout_slot_sandbox",
            Self::PrototypeStaticFull => "prototype_static_full",
        }
    }

    pub fn omits_beyond_projection_depth(self) -> bool {
        matches!(self, Self::LayoutSlotSandbox)
    }
}

impl DataModeCeiling {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "eval" => Some(Self::Eval),
            "fixture" => Some(Self::Fixture),
            "static" => Some(Self::Static),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Eval => "eval",
            Self::Fixture => "fixture",
            Self::Static => "static",
        }
    }

    pub fn as_data_mode(self) -> DataMode {
        match self {
            Self::Eval => DataMode::Eval,
            Self::Fixture => DataMode::Fixture,
            Self::Static => DataMode::Static,
        }
    }

    /// Whether this ceiling can serve the requested mode (one-way downgrade only).
    pub fn allows(self, requested: DataMode) -> bool {
        match self {
            Self::Eval => true,
            Self::Fixture => !matches!(requested, DataMode::Eval),
            Self::Static => matches!(requested, DataMode::Static),
        }
    }

    pub fn requires_plug_ds(self) -> bool {
        matches!(self, Self::Eval)
    }

    pub fn requires_metric_warmup(self) -> bool {
        matches!(self, Self::Eval)
    }

    pub fn allows_eval_api(self) -> bool {
        matches!(self, Self::Eval)
    }
}

impl DataMode {
    pub fn parse(s: &str) -> Option<Self> {
        DataModeCeiling::parse(s).map(|c| c.as_data_mode())
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Eval => "eval",
            Self::Fixture => "fixture",
            Self::Static => "static",
        }
    }

    pub fn allows_eval_api(self) -> bool {
        matches!(self, Self::Eval)
    }

    pub fn allows_fixture_api(self) -> bool {
        matches!(self, Self::Eval | Self::Fixture)
    }

    /// Clamp requested mode to ceiling; returns None if not allowed.
    pub fn clamp_to_ceiling(requested: Self, ceiling: DataModeCeiling) -> Option<Self> {
        if ceiling.allows(requested) {
            Some(requested)
        } else {
            None
        }
    }
}

impl ReviewProjection {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "plane" => Some(Self::Plane),
            "plane_region" => Some(Self::PlaneRegion),
            "plane_region_section" => Some(Self::PlaneRegionSection),
            "plane_region_section_slot" => Some(Self::PlaneRegionSectionSlot),
            "static_full" | "static" => Some(Self::StaticFull),
            "live_full" | "live" => Some(Self::LiveFull),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Plane => "plane",
            Self::PlaneRegion => "plane_region",
            Self::PlaneRegionSection => "plane_region_section",
            Self::PlaneRegionSectionSlot => "plane_region_section_slot",
            Self::StaticFull => "static_full",
            Self::LiveFull => "live_full",
        }
    }

    pub fn max_ui_role_depth(self) -> Option<&'static str> {
        match self {
            Self::Plane => Some("plane"),
            Self::PlaneRegion => Some("region"),
            Self::PlaneRegionSection => Some("section"),
            Self::PlaneRegionSectionSlot => Some("slot"),
            Self::StaticFull | Self::LiveFull => None,
        }
    }
}

/// Ordinal depth for layout-debug projection (`plane` < `region` < `section` < `slot` < `content`).
pub fn ui_role_depth_rank(role: &str) -> Option<u8> {
    match role.trim().to_ascii_lowercase().as_str() {
        "plane" => Some(0),
        // slide sits between plane and region in the presentation tree.
        "slide" => Some(1),
        "region" => Some(1),
        "section" => Some(2),
        "slot" => Some(3),
        "content" => Some(4),
        _ => None,
    }
}

/// Whether `ui_role` is within the inclusive depth allowed by `max_role` (`None` = no limit).
pub fn ui_role_within_max_depth(ui_role: &str, max_role: Option<&str>) -> bool {
    let Some(max_role) = max_role else {
        return true;
    };
    let Some(depth) = ui_role_depth_rank(ui_role) else {
        return true;
    };
    let max_depth = ui_role_depth_rank(max_role).unwrap_or(99);
    depth <= max_depth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceiling_allows_downgrade_only() {
        assert!(DataModeCeiling::Eval.allows(DataMode::Static));
        assert!(DataModeCeiling::Fixture.allows(DataMode::Static));
        assert!(!DataModeCeiling::Fixture.allows(DataMode::Eval));
        assert!(!DataModeCeiling::Static.allows(DataMode::Fixture));
    }

    #[test]
    fn ui_role_depth_respects_review_projection_ceiling() {
        assert!(ui_role_within_max_depth("plane", Some("plane")));
        assert!(!ui_role_within_max_depth("region", Some("plane")));
        assert!(ui_role_within_max_depth("region", Some("region")));
        assert!(!ui_role_within_max_depth("section", Some("region")));
        assert!(ui_role_within_max_depth("section", Some("section")));
        assert!(!ui_role_within_max_depth("content", Some("section")));
        assert!(ui_role_within_max_depth("slot", Some("slot")));
        assert!(!ui_role_within_max_depth("content", Some("slot")));
        assert!(ui_role_within_max_depth("content", None));
    }

    #[test]
    fn plane_region_section_slot_parses_and_caps_at_slot() {
        let projection = ReviewProjection::parse("plane_region_section_slot").expect("parse");
        assert_eq!(projection.max_ui_role_depth(), Some("slot"));
        assert!(ui_role_within_max_depth(
            "slot",
            projection.max_ui_role_depth()
        ));
        assert!(!ui_role_within_max_depth(
            "content",
            projection.max_ui_role_depth()
        ));
    }
}
