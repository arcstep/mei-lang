//! Review modes smoke: preset axes + tree depth contract.

use mei_lang_app::prototype_preset::{
    match_preset, preset_tree_max_ui_role, LAYOUT_PRESET, PROTOTYPE_PRESETS,
    PROTOTYPE_SURFACE_PRESET,
};
use mei_lang_kernel::{DataMode, DataModeCeiling, ReviewProjection};

fn axes_for_build(data_mode: &str, review_projection: &str) -> (DataMode, ReviewProjection) {
    let dm = DataMode::parse(data_mode).expect("data_mode");
    let rp = ReviewProjection::parse(review_projection).expect("review_projection");
    let clamped = DataMode::clamp_to_ceiling(dm, DataModeCeiling::Eval).expect("clamp");
    (clamped, rp)
}

#[test]
fn layout_static_plane_region_section_slot_preset() {
    let (dm, rp) = axes_for_build("static", "plane_region_section_slot");
    assert_eq!(dm, DataMode::Static);
    assert_eq!(rp, ReviewProjection::PlaneRegionSectionSlot);
    let preset = match_preset("static", "plane_region_section_slot").expect("preset");
    assert_eq!(preset.slug, "layout");
    assert_eq!(preset.tree_max_ui_role, "content");
    assert_eq!(
        preset_tree_max_ui_role("static", "plane_region_section_slot"),
        "content"
    );
}

#[test]
fn prototype_static_full_preset() {
    let (dm, rp) = axes_for_build("static", "static_full");
    assert_eq!(dm, DataMode::Static);
    assert_eq!(rp, ReviewProjection::StaticFull);
    let preset = match_preset("static", "static_full").expect("preset");
    assert_eq!(preset.slug, "prototype");
    assert_eq!(preset.tree_max_ui_role, "content");
    assert_eq!(preset_tree_max_ui_role("static", "static_full"), "content");
}

#[test]
fn app_eval_live_full_has_no_workspace_preset() {
    let (dm, rp) = axes_for_build("eval", "live_full");
    assert_eq!(dm, DataMode::Eval);
    assert_eq!(rp, ReviewProjection::LiveFull);
    assert!(match_preset("eval", "live_full").is_none());
}

#[test]
fn static_ceiling_downgrades_eval_request() {
    assert_eq!(
        DataMode::clamp_to_ceiling(DataMode::Eval, DataModeCeiling::Static),
        None
    );
    assert_eq!(
        DataMode::clamp_to_ceiling(DataMode::Eval, DataModeCeiling::Static)
            .unwrap_or(DataMode::Static),
        DataMode::Static
    );
}

#[test]
fn workspace_presets_cover_layout_and_prototype() {
    assert_eq!(PROTOTYPE_PRESETS.len(), 2);
    assert_eq!(LAYOUT_PRESET.slug, "layout");
    assert_eq!(PROTOTYPE_SURFACE_PRESET.slug, "prototype");
    for preset in PROTOTYPE_PRESETS {
        assert!(
            !preset.tree_max_ui_role.is_empty(),
            "preset {} missing tree depth",
            preset.slug
        );
    }
}
