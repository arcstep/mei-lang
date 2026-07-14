//! Build SSR review_projection depth on PreviewRuntimeContext.

use std::collections::BTreeMap;

use mei_lang_kernel::CompiledApp;

use super::build_preview_runtime_context;
use crate::ui::route::UiRouteMode;

fn minimal_compiled() -> CompiledApp {
    CompiledApp {
        app_id: "demo".to_string(),
        active_scene: Some("home".to_string()),
        stage_registry: Default::default(),
        stage_programs: Default::default(),
        scene_slot_modules: Default::default(),
        content_capabilities: Default::default(),
        narration_catalogs: Default::default(),
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
        build_t2_page_index: Default::default(),
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
fn layout_route_omits_beyond_projection_and_caps_at_slot() {
    let compiled = minimal_compiled();
    let ctx = build_preview_runtime_context(
        &compiled,
        UiRouteMode::Layout,
        None,
        None,
        None,
        Some("static"),
        Some("plane_region_section_slot"),
    );
    assert!(ctx.structure_anchors_enabled);
    assert!(ctx.dev_inspect_chrome_enabled);
    assert!(ctx.omit_beyond_projection_depth);
    assert!(ctx.is_layout_slot_sandbox());
    assert_eq!(ctx.review_projection_max_ui_role(), Some("slot"));
    assert!(!ctx.ui_role_allowed_for_projection("content"));
    assert!(ctx.ui_role_allowed_for_projection("slot"));
    assert!(ctx.host_ssr_slim_payload);
}

#[test]
fn layout_runtime_context_parses_review_projection_depth() {
    let compiled = minimal_compiled();
    let ctx = build_preview_runtime_context(
        &compiled,
        UiRouteMode::Layout,
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
fn prototype_route_keeps_full_content_without_omit() {
    let compiled = minimal_compiled();
    let ctx = build_preview_runtime_context(
        &compiled,
        UiRouteMode::Prototype,
        None,
        None,
        None,
        Some("static"),
        Some("static_full"),
    );
    assert!(ctx.structure_anchors_enabled);
    assert!(!ctx.omit_beyond_projection_depth);
    assert!(ctx.is_prototype_static_full());
    assert_eq!(ctx.review_projection_max_ui_role(), None);
    assert!(ctx.ui_role_allowed_for_projection("content"));
    assert!(!ctx.host_ssr_slim_payload);
}

#[test]
fn static_full_on_layout_route_allows_all_roles_but_still_omits() {
    let compiled = minimal_compiled();
    let ctx = build_preview_runtime_context(
        &compiled,
        UiRouteMode::Layout,
        None,
        None,
        None,
        None,
        Some("static_full"),
    );
    assert_eq!(ctx.review_projection_max_ui_role(), None);
    assert!(ctx.ui_role_allowed_for_projection("content"));
    assert!(ctx.omit_beyond_projection_depth);
}
