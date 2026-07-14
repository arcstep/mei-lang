//! Phase 6: size/layout policy scoped by StageProfile (0104 §7 / 0105 §11).
//!
//! Fill-down is a **cockpit** contract, not a global UI axiom.

use serde::{Deserialize, Serialize};

use super::stage_registry::StageProfile;

/// Per-axis size causality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SizeAxisPolicy {
    /// Parent constrains child (cockpit / slides inline+block).
    Constrained,
    /// Content contributes intrinsic size (site block; deferred).
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

/// Fill-down / aperture policy for a Stage Profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillDownPolicy {
    /// Cockpit: strict `__mei_layout_fill` causal chain.
    Strict,
    /// Slides: fixed paged aperture; do not apply cockpit Fill-down axioms to deck trees.
    PagedAperture,
    /// Site / Training — not implemented this round.
    Deferred,
}

impl FillDownPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::PagedAperture => "paged_aperture",
            Self::Deferred => "deferred",
        }
    }

    /// Whether layout-budget validation should enforce Fill-down body/content markers.
    pub fn enforces_strict_fill_down(self) -> bool {
        matches!(self, Self::Strict)
    }
}

/// Layout / size policy table entry for one Stage Profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileLayoutPolicy {
    pub profile: StageProfile,
    pub inline_axis: SizeAxisPolicy,
    pub block_axis: SizeAxisPolicy,
    pub fill_down: FillDownPolicy,
}

impl ProfileLayoutPolicy {
    pub fn for_profile(profile: StageProfile) -> Self {
        match profile {
            StageProfile::Cockpit => Self {
                profile,
                inline_axis: SizeAxisPolicy::Constrained,
                block_axis: SizeAxisPolicy::Constrained,
                fill_down: FillDownPolicy::Strict,
            },
            StageProfile::Slides => Self {
                profile,
                inline_axis: SizeAxisPolicy::Constrained,
                block_axis: SizeAxisPolicy::Constrained,
                fill_down: FillDownPolicy::PagedAperture,
            },
        }
    }

    pub fn summary_label(self) -> String {
        format!(
            "{}:inline={}/block={}/fill={}",
            self.profile.as_str(),
            self.inline_axis.as_str(),
            self.block_axis.as_str(),
            self.fill_down.as_str()
        )
    }
}

/// Digest fragment for cache / context_export (ops + profile).
pub fn profile_layout_policy_digest(
    profile: StageProfile,
    strict_t1_fill_down: bool,
    strict_t2_fill_down: bool,
) -> String {
    let policy = ProfileLayoutPolicy::for_profile(profile);
    format!(
        "v1|{}|t1={}|t2={}",
        policy.summary_label(),
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
    }

    #[test]
    fn slides_is_paged_aperture_not_strict_fill() {
        let p = ProfileLayoutPolicy::for_profile(StageProfile::Slides);
        assert_eq!(p.fill_down, FillDownPolicy::PagedAperture);
        assert!(!p.fill_down.enforces_strict_fill_down());
    }
}
