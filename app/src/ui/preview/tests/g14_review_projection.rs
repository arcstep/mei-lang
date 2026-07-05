//! Build SSR review_projection depth on PreviewRuntimeContext.

use std::collections::BTreeMap;

use mei_lang_kernel::CompiledApp;

use super::build_preview_runtime_context;
use crate::ui::route::UiRouteMode;

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
fn app_route_does_not_enable_structure_anchors() {
    let compiled = minimal_compiled();
    let ctx = build_preview_runtime_context(
        &compiled,
        UiRouteMode::App,
        None,
        None,
        None,
        None,
        Some("plane_region"),
    );
    assert!(!ctx.structure_anchors_enabled);
    assert!(!ctx.dev_inspect_chrome_enabled);
    assert_eq!(ctx.review_projection_max_ui_role(), Some("region"));
}

#[test]
fn run_route_applies_review_projection_depth() {
    let compiled = minimal_compiled();
    let ctx = build_preview_runtime_context(
        &compiled,
        UiRouteMode::Run,
        None,
        None,
        None,
        Some("static"),
        Some("plane_region_section"),
    );
    assert!(!ctx.structure_anchors_enabled);
    assert!(!ctx.dev_inspect_chrome_enabled);
    assert_eq!(ctx.review_projection_max_ui_role(), Some("section"));
    assert!(!ctx.ui_role_allowed_for_projection("content"));
    assert!(ctx.ui_role_allowed_for_projection("section"));
}

#[test]
fn build_runtime_context_parses_review_projection_depth() {
    let compiled = minimal_compiled();
    let ctx = build_preview_runtime_context(
        &compiled,
        UiRouteMode::Build,
        None,
        None,
        None,
        None,
        Some("plane_region"),
    );
    assert!(ctx.structure_anchors_enabled);
    assert!(ctx.dev_inspect_chrome_enabled);
    assert_eq!(ctx.review_projection_max_ui_role(), Some("region"));
    assert!(!ctx.ui_role_allowed_for_projection("content"));
    assert!(ctx.ui_role_allowed_for_projection("region"));
}

#[test]
fn static_full_projection_allows_all_roles() {
    let compiled = minimal_compiled();
    let ctx = build_preview_runtime_context(
        &compiled,
        UiRouteMode::Build,
        None,
        None,
        None,
        None,
        Some("static_full"),
    );
    assert_eq!(ctx.review_projection_max_ui_role(), None);
    assert!(ctx.ui_role_allowed_for_projection("content"));
}
