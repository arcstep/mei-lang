//! Review modes smoke: preset axes + tree depth contract.

use mei_lang_app::prototype_preset::{
    match_preset, preset_tree_max_ui_role, PROTOTYPE_PRESETS,
};
use mei_lang_kernel::{DataMode, DataModeCeiling, ReviewProjection};

fn axes_for_build(data_mode: &str, review_projection: &str) -> (DataMode, ReviewProjection) {
    let dm = DataMode::parse(data_mode).expect("data_mode");
    let rp = ReviewProjection::parse(review_projection).expect("review_projection");
    let clamped = DataMode::clamp_to_ceiling(dm, DataModeCeiling::Eval).expect("clamp");
    (clamped, rp)
}

#[test]
fn build_static_plane_region_section_preset() {
    let (dm, rp) = axes_for_build("static", "plane_region_section");
    assert_eq!(dm, DataMode::Static);
    assert_eq!(rp, ReviewProjection::PlaneRegionSection);
    let preset = match_preset("static", "plane_region_section").expect("preset");
    assert_eq!(preset.slug, "section");
    assert_eq!(preset_tree_max_ui_role("static", "plane_region_section"), "plane");
}

#[test]
fn build_content_static_full_preset() {
    let (dm, rp) = axes_for_build("static", "static_full");
    assert_eq!(dm, DataMode::Static);
    assert_eq!(rp, ReviewProjection::StaticFull);
    let preset = match_preset("static", "static_full").expect("preset");
    assert_eq!(preset.slug, "content");
    assert_eq!(preset_tree_max_ui_role("static", "static_full"), "plane");
}

#[test]
fn app_eval_live_full_preset() {
    let (dm, rp) = axes_for_build("eval", "live_full");
    assert_eq!(dm, DataMode::Eval);
    assert_eq!(rp, ReviewProjection::LiveFull);
    let preset = match_preset("eval", "live_full").expect("preset");
    assert_eq!(preset.slug, "eval");
    assert_eq!(preset.tree_max_ui_role, "plane");
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
fn prototype_presets_cover_four_task_workflows() {
    assert_eq!(PROTOTYPE_PRESETS.len(), 4);
    for preset in PROTOTYPE_PRESETS {
        assert!(
            !preset.tree_max_ui_role.is_empty(),
            "preset {} missing tree depth",
            preset.slug
        );
    }
}

#[test]
fn preset_tree_roles_match_0508_contract() {
    let expectations: &[(&str, &str, &str)] = &[
        ("eval", "plane", "plane"),
        ("content", "plane", "plane"),
        ("section", "plane", "plane"),
        ("region", "plane", "plane"),
    ];
    for (slug, expected_role, _) in expectations {
        let preset = PROTOTYPE_PRESETS
            .iter()
            .find(|item| item.slug == *slug)
            .expect("preset");
        assert_eq!(preset.tree_max_ui_role, *expected_role);
        let role = preset_tree_max_ui_role(preset.data_mode, preset.review_projection);
        assert_eq!(role, *expected_role);
    }
}
