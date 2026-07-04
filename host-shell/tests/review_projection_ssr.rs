//! SSR review projection depth contract.

use mei_lang_app::{build_preview_runtime_context, UiRouteMode};
use mei_lang_kernel::{ui_role_within_max_depth, CompiledApp, ReviewProjection};
use std::collections::BTreeMap;

fn minimal_compiled() -> CompiledApp {
    CompiledApp {
        app_id: "demo".to_string(),
        active_scene: Some("home".to_string()),
        active_target_file: "scenes/home.mei".to_string(),
        resources: Vec::new(),
        world_metrics: BTreeMap::new(),
        world_semantic_by_file: BTreeMap::new(),
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview".to_string(),
        file_tree: Vec::new(),
        scene_contract: None,
        scene_local_nav_by_target: BTreeMap::new(),
        scene_bindings_by_id: BTreeMap::new(),
        scene_examples_by_id: BTreeMap::new(),
        scene_projection_assembly_by_id: BTreeMap::new(),
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
        build_experience_index: Default::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    }
}

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

#[test]
fn app_route_runtime_context_applies_plane_region() {
    let ctx = build_preview_runtime_context(
        &minimal_compiled(),
        UiRouteMode::App,
        None,
        None,
        None,
        None,
        Some("plane_region"),
    );
    assert!(ctx.structure_anchors_enabled);
    assert!(!ctx.ui_role_allowed_for_projection("content"));
    assert!(ctx.ui_role_allowed_for_projection("region"));
}

#[test]
fn run_route_runtime_context_applies_plane_region_section() {
    let ctx = build_preview_runtime_context(
        &minimal_compiled(),
        UiRouteMode::Run,
        None,
        None,
        None,
        Some("fixture"),
        Some("plane_region_section"),
    );
    assert!(ctx.structure_anchors_enabled);
    assert!(!ctx.ui_role_allowed_for_projection("content"));
    assert!(ctx.ui_role_allowed_for_projection("section"));
}
