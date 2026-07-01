//! Viewport stage tier (T0 / T1 / T2) and Z-band constants for panel_contract.

pub const TIER_T0: &str = "t0";
pub const TIER_T1: &str = "t1";
pub const TIER_T2: &str = "t2";

pub const DEFAULT_PANEL_TIER: &str = TIER_T1;

// Tier overall bands (see docs/mei-lang-v2/03-ui/0301-viewport-and-three-tier-layout.md)
pub const Z_T0_MIN: i64 = 0;
pub const Z_T0_MAX: i64 = 1000;
pub const Z_T1_MIN: i64 = 1001;
pub const Z_T1_MAX: i64 = 2000;
pub const Z_T2_MIN: i64 = 2001;
pub const Z_T2_MAX: i64 = 3000;
pub const Z_RESERVED_MIN: i64 = 3001;
pub const Z_RESERVED_MAX: i64 = 4999;
pub const Z_PRESENTATION_MIN: i64 = 5000;
pub const Z_PRESENTATION_MAX: i64 = 5399;
pub const Z_COPILOT_MIN: i64 = 5400;
pub const Z_COPILOT_MAX: i64 = 5799;
pub const Z_HOST_MIN: i64 = 5800;

// T0 regular content (0–99)
pub const Z_T0_DEFAULT: i64 = 1;

// T1 regular content (1001–1100)
pub const Z_T1_DEFAULT: i64 = 1001;
pub const Z_T1_CENTER_PANEL: i64 = 1101;
pub const Z_T1_RAIL: i64 = 1102;
pub const Z_T1_CENTER_FLOAT: i64 = 1105;
pub const Z_T1_HEADER: i64 = 1110;

// T1 operation / bubble sub-bands
pub const Z_T1_MAP_TOOLS: i64 = 1210;
pub const Z_T1_TOOLTIP: i64 = 1300;

// T2 regular content (2001–2100)
pub const Z_T2_DEFAULT: i64 = 2001;
pub const Z_T2_BOARD: i64 = 2010;
pub const Z_T2_CONTEXT_BANNER: i64 = 2210;
pub const Z_T2_FILTER_FLOAT: i64 = 2250;
pub const Z_T2_TOOLTIP: i64 = 2300;
pub const Z_T2_TEXT_POPOVER: i64 = 2350;

/// Normalize author `tier` to `t0` | `t1` | `t2`. Rejects legacy basemap/chrome/overlay names.
pub fn canonical_tier(raw: &str) -> Result<&'static str, String> {
    match raw.trim() {
        TIER_T0 => Ok(TIER_T0),
        TIER_T1 => Ok(TIER_T1),
        TIER_T2 => Ok(TIER_T2),
        "basemap" => Err(
            "tier \"basemap\" is deprecated; use tier \"t0\" (Tier-0 basemap stage)".to_string(),
        ),
        "chrome" => Err(
            "tier \"chrome\" is deprecated; use tier \"t1\" (Tier-1 chrome board)".to_string(),
        ),
        "overlay" => Err(
            "tier \"overlay\" is deprecated; use tier \"t2\" (Tier-2 board workspace)".to_string(),
        ),
        other if other.is_empty() => Err("tier must be t0, t1, or t2".to_string()),
        other => Err(format!(
            "unknown tier \"{other}\"; expected t0, t1, or t2 (Tier-0/1/2)"
        )),
    }
}

pub fn default_z_index_for_tier(tier: &str) -> i64 {
    match tier {
        TIER_T0 => Z_T0_DEFAULT,
        TIER_T1 => Z_T1_DEFAULT,
        TIER_T2 => Z_T2_DEFAULT,
        _ => Z_T1_DEFAULT,
    }
}

pub fn default_z_index_for_chrome_role(role: &str) -> Option<i64> {
    match role {
        "header" => Some(Z_T1_HEADER),
        "center_float" => Some(Z_T1_CENTER_FLOAT),
        "rail" => Some(Z_T1_RAIL),
        "center_panel" => Some(Z_T1_CENTER_PANEL),
        _ => None,
    }
}

/// Returns whether `z` lies in the documented band for a scene tier (`t0`/`t1`/`t2`).
pub fn z_index_in_tier_band(tier: &str, z: i64) -> bool {
    match tier {
        TIER_T0 => (Z_T0_MIN..=Z_T0_MAX).contains(&z),
        TIER_T1 => (Z_T1_MIN..=Z_T1_MAX).contains(&z),
        TIER_T2 => (Z_T2_MIN..=Z_T2_MAX).contains(&z),
        _ => false,
    }
}

/// Returns whether `z` lies in a non-tier plane band (reserved / presentation / copilot / host).
pub fn z_index_in_named_plane(plane: &str, z: i64) -> bool {
    match plane {
        "reserved" => (Z_RESERVED_MIN..=Z_RESERVED_MAX).contains(&z),
        "presentation" => (Z_PRESENTATION_MIN..=Z_PRESENTATION_MAX).contains(&z),
        "copilot" => (Z_COPILOT_MIN..=Z_COPILOT_MAX).contains(&z),
        "host" => z >= Z_HOST_MIN,
        _ => false,
    }
}

/// Named runtime overlay tokens mirrored in CSS (`--mei-z-*`) and `COCKPIT_Z_INDEX`.
pub fn runtime_overlay_z_index(token: &str) -> Option<i64> {
    match token {
        "map_tools" => Some(Z_T1_MAP_TOOLS),
        "tooltip" => Some(Z_T1_TOOLTIP),
        "drilldown" => Some(Z_T2_DEFAULT),
        "drilldown_board" => Some(Z_T2_BOARD),
        "drilldown_context" => Some(Z_T2_CONTEXT_BANNER),
        "filter_float" => Some(Z_T2_FILTER_FLOAT),
        "tooltip_in_board" => Some(Z_T2_TOOLTIP),
        "text_popover" => Some(Z_T2_TEXT_POPOVER),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_tier_accepts_t0_t1_t2() {
        assert_eq!(canonical_tier("t0").unwrap(), TIER_T0);
        assert_eq!(canonical_tier("t1").unwrap(), TIER_T1);
        assert_eq!(canonical_tier("t2").unwrap(), TIER_T2);
    }

    #[test]
    fn canonical_tier_rejects_legacy_names() {
        assert!(canonical_tier("basemap").is_err());
        assert!(canonical_tier("chrome").is_err());
        assert!(canonical_tier("overlay").is_err());
    }

    #[test]
    fn default_z_index_uses_thousand_band() {
        assert_eq!(default_z_index_for_tier(TIER_T0), Z_T0_DEFAULT);
        assert_eq!(default_z_index_for_tier(TIER_T1), Z_T1_DEFAULT);
        assert_eq!(default_z_index_for_tier(TIER_T2), Z_T2_DEFAULT);
        assert_eq!(default_z_index_for_chrome_role("header"), Some(Z_T1_HEADER));
    }

    #[test]
    fn runtime_overlay_tokens_and_planes() {
        assert!(z_index_in_tier_band(TIER_T1, Z_T1_HEADER));
        assert!(!z_index_in_tier_band(TIER_T0, Z_T1_HEADER));
        assert_eq!(runtime_overlay_z_index("tooltip"), Some(Z_T1_TOOLTIP));
        assert!(z_index_in_named_plane("presentation", 5100));
        assert!(z_index_in_named_plane("copilot", 5500));
        assert!(z_index_in_named_plane("host", 5800));
    }

    #[test]
    fn z_band_constants_match_0301_contract() {
        assert!(Z_T0_DEFAULT >= Z_T0_MIN && Z_T0_DEFAULT <= Z_T0_MAX);
        assert!(Z_T1_DEFAULT >= Z_T1_MIN && Z_T1_DEFAULT <= Z_T1_MAX);
        assert!(Z_T2_DEFAULT >= Z_T2_MIN && Z_T2_DEFAULT <= Z_T2_MAX);
        assert!(Z_RESERVED_MIN > Z_T2_MAX && Z_RESERVED_MAX < Z_PRESENTATION_MIN);
        assert!(Z_PRESENTATION_MIN < Z_COPILOT_MIN);
        assert!(Z_COPILOT_MAX < Z_HOST_MIN);
        assert_eq!(Z_T1_MAP_TOOLS, 1210);
        assert_eq!(Z_T1_TOOLTIP, 1300);
        assert_eq!(Z_T2_BOARD, 2010);
        assert_eq!(Z_T2_CONTEXT_BANNER, 2210);
        assert_eq!(Z_T2_FILTER_FLOAT, 2250);
        assert_eq!(Z_T2_TOOLTIP, 2300);
        assert_eq!(Z_T2_TEXT_POPOVER, 2350);
    }
}
