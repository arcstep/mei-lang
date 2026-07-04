//! Review axes contract tests (kernel + host HTTP boundary).

use mei_lang_kernel::{DataMode, DataModeCeiling, ReviewProjection};

#[test]
fn static_ceiling_blocks_eval_api_flag() {
    assert!(!DataModeCeiling::Static.allows_eval_api());
    assert!(DataModeCeiling::Eval.allows_eval_api());
    assert!(!DataModeCeiling::Fixture.allows_eval_api());
}

#[test]
fn data_mode_clamp_respects_ceiling() {
    assert_eq!(
        DataMode::clamp_to_ceiling(DataMode::Eval, DataModeCeiling::Static),
        None
    );
    assert_eq!(
        DataMode::clamp_to_ceiling(DataMode::Static, DataModeCeiling::Fixture),
        Some(DataMode::Static)
    );
}

#[test]
fn review_projection_parse_and_depth() {
    assert_eq!(
        ReviewProjection::parse("plane_region_section"),
        Some(ReviewProjection::PlaneRegionSection)
    );
    assert_eq!(
        ReviewProjection::Plane.max_ui_role_depth(),
        Some("plane")
    );
    assert!(ReviewProjection::LiveFull.max_ui_role_depth().is_none());
}
