//! Stage Layout Profile SSOT (0104 §7 / 0105 §11 / 0332).
//!
//! Cascade: platform fallback → StageLayoutProfile → ops.themes.*.layout → author explicit.
//! Fill-down is a **cockpit** contract; slides use paged aperture; page uses document flow.

use serde::{Deserialize, Serialize};

use super::stage_registry::StageProfile;

/// Per-axis size causality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeAxisPolicy {
    /// Parent constrains child (cockpit / slides inline+block; page inline).
    Constrained,
    /// Content contributes intrinsic size (page block axis).
    Intrinsic,
    /// Profile not implemented this round.
    Deferred,
}

impl SizeAxisPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Constrained => "constrained",
            Self::Intrinsic => "intrinsic",
            Self::Deferred => "deferred",
        }
    }
}

/// Fill-down / aperture / document-flow policy for a Stage Profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillDownPolicy {
    /// Cockpit: strict `__mei_layout_fill` causal chain.
    Strict,
    /// Slides: fixed paged aperture; do not apply cockpit Fill-down axioms to deck trees.
    PagedAperture,
    /// Page: block axis intrinsic; stage viewport owns vertical scroll.
    DocumentFlow,
    /// Training / other deferred profiles.
    Deferred,
}

impl FillDownPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::PagedAperture => "paged_aperture",
            Self::DocumentFlow => "document_flow",
            Self::Deferred => "deferred",
        }
    }

    /// Whether layout-budget validation should enforce Fill-down body/content markers.
    pub fn enforces_strict_fill_down(self) -> bool {
        matches!(self, Self::Strict)
    }
}

/// Who owns overflow / scroll for the stage root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollOwnership {
    /// Cockpit viewport clips; children fill allocated cells.
    ViewportClip,
    /// Slides page aperture; no continuous document scroll.
    Paged,
    /// Page stage root scrolls vertically inside the stage viewport.
    StageViewport,
}

impl ScrollOwnership {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ViewportClip => "viewport_clip",
            Self::Paged => "paged",
            Self::StageViewport => "stage_viewport",
        }
    }
}

/// Hierarchy spacing tokens for one Stage Profile (omit-inject defaults).
/// Constructed only via `for_profile` (static table); not deserialized from JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct ProfileSpacingTokens {
    /// plane.gap → region outer margin
    pub plane_gap: &'static str,
    pub plane_padding: &'static str,
    /// region.gap → section outer margin
    pub region_gap: &'static str,
    pub region_padding: &'static str,
    /// section.gap → content outer margin
    pub section_gap: &'static str,
    pub section_padding: &'static str,
    pub content_gap: &'static str,
    pub content_padding: &'static str,
    /// Slides shell page padding (unused for cockpit/page).
    pub slide_shell_padding: &'static str,
}

impl ProfileSpacingTokens {
    pub fn for_profile(profile: StageProfile) -> Self {
        match profile {
            StageProfile::Cockpit => Self {
                plane_gap: "1px",
                plane_padding: "1px",
                region_gap: "1px",
                region_padding: "1px",
                section_gap: "1px",
                section_padding: "1px",
                content_gap: "0",
                content_padding: "0",
                slide_shell_padding: "0",
            },
            StageProfile::Slides => Self {
                plane_gap: "0",
                plane_padding: "0",
                region_gap: "0",
                region_padding: "0",
                section_gap: "0",
                section_padding: "0",
                content_gap: "0",
                content_padding: "0",
                slide_shell_padding: "48px 72px",
            },
            StageProfile::Page => Self {
                plane_gap: "16px",
                plane_padding: "16px",
                region_gap: "12px",
                region_padding: "0",
                section_gap: "8px",
                section_padding: "0",
                content_gap: "0",
                content_padding: "0",
                slide_shell_padding: "0",
            },
        }
    }

    pub fn digest_label(self) -> String {
        format!(
            "pg={}/pp={}/rg={}/rp={}/sg={}/sp={}/cg={}/cp={}/ss={}",
            self.plane_gap,
            self.plane_padding,
            self.region_gap,
            self.region_padding,
            self.section_gap,
            self.section_padding,
            self.content_gap,
            self.content_padding,
            self.slide_shell_padding.replace(' ', "_"),
        )
    }
}

/// Layout / size policy table entry for one Stage Profile.
/// Constructed only via `for_profile`; spacing tokens are `&'static str` table values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct ProfileLayoutPolicy {
    pub profile: StageProfile,
    pub inline_axis: SizeAxisPolicy,
    pub block_axis: SizeAxisPolicy,
    pub fill_down: FillDownPolicy,
    pub scroll: ScrollOwnership,
    pub spacing: ProfileSpacingTokens,
}

impl ProfileLayoutPolicy {
    pub fn for_profile(profile: StageProfile) -> Self {
        match profile {
            StageProfile::Cockpit => Self {
                profile,
                inline_axis: SizeAxisPolicy::Constrained,
                block_axis: SizeAxisPolicy::Constrained,
                fill_down: FillDownPolicy::Strict,
                scroll: ScrollOwnership::ViewportClip,
                spacing: ProfileSpacingTokens::for_profile(profile),
            },
            StageProfile::Slides => Self {
                profile,
                inline_axis: SizeAxisPolicy::Constrained,
                block_axis: SizeAxisPolicy::Constrained,
                fill_down: FillDownPolicy::PagedAperture,
                scroll: ScrollOwnership::Paged,
                spacing: ProfileSpacingTokens::for_profile(profile),
            },
            StageProfile::Page => Self {
                profile,
                inline_axis: SizeAxisPolicy::Constrained,
                block_axis: SizeAxisPolicy::Intrinsic,
                fill_down: FillDownPolicy::DocumentFlow,
                scroll: ScrollOwnership::StageViewport,
                spacing: ProfileSpacingTokens::for_profile(profile),
            },
        }
    }

    pub fn summary_label(self) -> String {
        format!(
            "{}:inline={}/block={}/fill={}/scroll={}",
            self.profile.as_str(),
            self.inline_axis.as_str(),
            self.block_axis.as_str(),
            self.fill_down.as_str(),
            self.scroll.as_str(),
        )
    }
}

/// Digest fragment for cache / context_export (ops + profile + spacing).
pub fn profile_layout_policy_digest(
    profile: StageProfile,
    strict_t1_fill_down: bool,
    strict_t2_fill_down: bool,
) -> String {
    let policy = ProfileLayoutPolicy::for_profile(profile);
    format!(
        "v2|{}|spacing={}|t1={}|t2={}",
        policy.summary_label(),
        policy.spacing.digest_label(),
        strict_t1_fill_down as u8,
        strict_t2_fill_down as u8
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cockpit_is_strict_fill_down() {
        let p = ProfileLayoutPolicy::for_profile(StageProfile::Cockpit);
        assert_eq!(p.fill_down, FillDownPolicy::Strict);
        assert!(p.fill_down.enforces_strict_fill_down());
        assert_eq!(p.inline_axis, SizeAxisPolicy::Constrained);
        assert_eq!(p.block_axis, SizeAxisPolicy::Constrained);
        assert_eq!(p.scroll, ScrollOwnership::ViewportClip);
        assert_eq!(p.spacing.plane_gap, "1px");
    }

    #[test]
    fn slides_is_paged_aperture_not_strict_fill() {
        let p = ProfileLayoutPolicy::for_profile(StageProfile::Slides);
        assert_eq!(p.fill_down, FillDownPolicy::PagedAperture);
        assert!(!p.fill_down.enforces_strict_fill_down());
        assert_eq!(p.spacing.slide_shell_padding, "48px 72px");
    }

    #[test]
    fn page_is_document_flow() {
        let p = ProfileLayoutPolicy::for_profile(StageProfile::Page);
        assert_eq!(p.fill_down, FillDownPolicy::DocumentFlow);
        assert!(!p.fill_down.enforces_strict_fill_down());
        assert_eq!(p.block_axis, SizeAxisPolicy::Intrinsic);
        assert_eq!(p.scroll, ScrollOwnership::StageViewport);
    }
}
