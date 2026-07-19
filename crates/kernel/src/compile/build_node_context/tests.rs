use std::collections::BTreeMap;

use super::*;
use crate::model::{BuildNodeId, CompiledApp, CompiledSceneRoute};

fn optional_external_workspace() -> Option<std::path::PathBuf> {
    let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
    let path = std::path::PathBuf::from(raw.trim());
    if path.as_os_str().is_empty() || !path.is_dir() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

fn sample_compiled() -> CompiledApp {
    CompiledApp {
        app_id: "demo".to_string(),
        title: "demo".to_string(),
        app_root: ".".to_string(),
        scene_routes: vec![CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "scenes/home.mei".to_string(),
            kind: "file_ref".to_string(),
            title: None,
            short_title: None,
            is_default: true,
            access_export: true,
        }],
        active_scene: Some("home".to_string()),
        stage_registry: Default::default(),
        stage_programs: Default::default(),
        scene_slot_modules: Default::default(),
        content_capabilities: Default::default(),
        narration_catalogs: Default::default(),
        active_target_file: "scenes/home.mei".to_string(),
        file_tree: Vec::new(),
        scene_contract: None,
        scene_local_nav_by_target: BTreeMap::new(),
        scene_bindings_by_id: BTreeMap::new(),
        scene_examples_by_id: BTreeMap::new(),
        scene_projection_assembly_by_id: BTreeMap::new(),
        resources: Vec::new(),
        world_metrics: BTreeMap::new(),
        world_semantic_by_file: BTreeMap::new(),
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
        build_experience_index: Default::default(),
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    }
}

#[test]
fn scene_node_resolves_target_file() {
    let compiled = sample_compiled();
    let ctx = resolve_build_node_context(&compiled, &BuildNodeId::scene("home"));
    assert_eq!(ctx.target_file, "scenes/home.mei");
    assert_eq!(ctx.provenance.symbol_id, "home");
}

#[test]
fn preview_target_from_world_dataset_node() {
    let node = BuildNodeId::world_dataset("scenes/01-执法要素.world.mei", "agency_objects");
    assert_eq!(
        preview_target_from_build_node(&node).as_deref(),
        Some("scenes/01-执法要素.world.mei")
    );
}

#[test]
fn component_authoring_preview_panel_scope_targets_host_panel() {
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};
    use crate::BuildPreviewKind;

    let Some(source_root) = optional_external_workspace() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let app_root = source_root.join("apps/hello");
    let example = source_root.join("stock/authoring/examples/chart-baseline.mei");
    if !app_root.is_dir() || !example.is_file() {
        eprintln!("skip: apps/hello or chart-baseline missing under MEI_TEST_WORKSPACE");
        return;
    }
    let home =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile hello home");
    let coord = crate::compile::build_experience::compile_coordinate_for_node(
        &BuildNodeId::component("chart.area"),
        &home,
    )
    .expect("coord");
    assert_eq!(coord.preview_kind, BuildPreviewKind::Script);
    let preview = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            preview_target: Some(coord.preview_target.clone()),
            ..CompileOptions::default()
        },
    )
    .expect("compile chart.area preview");
    assert!(
        preview.scene_contract.is_some(),
        "expected scene contract for chart.area preview"
    );
    let scope = build_preview_panel_scope(&preview, &BuildNodeId::component("chart.area"));
    assert_eq!(scope.as_deref(), Some("home/area_panel"));
}
