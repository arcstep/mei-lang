//! SSR review projection depth contract.

use mei_lang_kernel::{ui_role_within_max_depth, ReviewProjection};

#[test]
fn plane_region_section_blocks_content_role() {
    assert!(!ui_role_within_max_depth(
        "content",
        ReviewProjection::PlaneRegionSection.max_ui_role_depth()
    ));
    assert!(ui_role_within_max_depth(
        "section",
        ReviewProjection::PlaneRegionSection.max_ui_role_depth()
    ));
}

#[test]
fn plane_region_blocks_section_and_content() {
    assert!(!ui_role_within_max_depth(
        "section",
        ReviewProjection::PlaneRegion.max_ui_role_depth()
    ));
    assert!(ui_role_within_max_depth(
        "region",
        ReviewProjection::PlaneRegion.max_ui_role_depth()
    ));
}

#[test]
fn static_full_allows_content() {
    assert!(ui_role_within_max_depth(
        "content",
        ReviewProjection::StaticFull.max_ui_role_depth()
    ));
}
