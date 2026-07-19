use super::*;

use crate::model::{BlockDecl, BuildNodeId};
use std::path::PathBuf;

fn optional_external_workspace() -> Option<PathBuf> {
    let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
    let path = PathBuf::from(raw.trim());
    if path.as_os_str().is_empty() || !path.is_dir() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

#[test]
fn backing_refs_from_metric_binding() {
    let props = serde_json::json!({
        "metric": { "__ref": "metric", "id": "total", "from_dataset": "agency_objects" }
    });
    let refs = backing_refs_from_block_props(&props);
    assert!(refs.iter().any(|r| r.contains("agency_objects")));
}

#[test]
fn board_build_node_resolves_preview_target_and_scene() {
    use crate::model::BuildNodeId;

    let node =
        BuildNodeId::board_file("scenes/01-执法要素.board.mei#enforcement_units_analytics_board");
    assert_eq!(
        preview_target_from_build_node_with_app(&node, None).as_deref(),
        Some("scenes/01-执法要素.board.mei")
    );
    assert_eq!(
        compile_scene_from_build_node(&node).as_deref(),
        Some("enforcement_units_analytics_board")
    );
    let slot = BuildNodeId::board_slot(
        "scenes/01-执法要素.board.mei#enforcement_units_analytics_board",
        "hero",
    );
    assert_eq!(
        preview_target_from_build_node_with_app(&slot, None).as_deref(),
        Some("scenes/01-执法要素.board.mei")
    );
    assert_eq!(
        compile_scene_from_build_node(&slot).as_deref(),
        Some("enforcement_units_analytics_board")
    );
}

#[test]
fn compile_scene_from_panel_node() {
    let node = BuildNodeId::scene_panel("home", "kpi_row");
    assert_eq!(
        compile_scene_from_build_node(&node).as_deref(),
        Some("home")
    );
}

#[test]
fn compile_coordinate_board_exports_share_preview_target() {
    use crate::model::{BuildNodeId, CompiledApp, CompiledSceneRoute};
    use std::collections::BTreeMap;

    let compiled = CompiledApp {
        app_id: "demo".to_string(),
        title: "demo".to_string(),
        app_root: "zhifa".to_string(),
        scene_routes: vec![CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "scenes/home.mei".to_string(),
            kind: "file_ref".to_string(),
            title: Some("Home".to_string()),
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
    };
    let board_a = BuildNodeId::board_file("scenes/01.board.mei#board_a");
    let board_b = BuildNodeId::board_file("scenes/01.board.mei#board_b");
    let slot = BuildNodeId::board_slot("scenes/01.board.mei#board_a", "chart");
    let coord_a = compile_coordinate_for_node(&board_a, &compiled).expect("board a");
    let coord_b = compile_coordinate_for_node(&board_b, &compiled).expect("board b");
    let coord_slot = compile_coordinate_for_node(&slot, &compiled).expect("slot");
    assert_eq!(coord_a.preview_target, "scenes/01.board.mei");
    assert_eq!(coord_b.preview_target, coord_a.preview_target);
    assert_eq!(coord_slot.preview_target, coord_a.preview_target);
    assert_ne!(coord_a.scene_id, coord_b.scene_id);
    assert_eq!(coord_slot.scene_id, coord_a.scene_id);
}

#[test]
fn compile_coordinate_groups_scene_panels_with_scene_route() {
    use crate::model::{BuildNodeId, CompiledApp, CompiledSceneRoute};
    use std::collections::BTreeMap;

    let compiled = CompiledApp {
        app_id: "demo".to_string(),
        title: "demo".to_string(),
        app_root: "zhifa".to_string(),
        scene_routes: vec![CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "scenes/home.mei".to_string(),
            kind: "file_ref".to_string(),
            title: Some("Home".to_string()),
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
    };
    let scene = BuildNodeId::scene("home");
    let panel = BuildNodeId::scene_panel("home", "kpi_row");
    let scene_coord = compile_coordinate_for_node(&scene, &compiled).expect("scene coord");
    let panel_coord = compile_coordinate_for_node(&panel, &compiled).expect("panel coord");
    assert_eq!(scene_coord.preview_target, "scenes/home.mei");
    assert_eq!(panel_coord.preview_target, "scenes/home.mei");
    assert_eq!(scene_coord.scene_id.as_deref(), Some("home"));
    assert_eq!(panel_coord.scene_id.as_deref(), Some("home"));
}

#[test]
fn compile_coordinate_for_template_file_uses_authoring_preview() {
    use crate::model::{
        BuildNodeId, BuildTemplateIndex, CompiledApp, CompiledSceneRoute, TemplateCatalogEntry,
        TemplateConsumerAnchor,
    };
    use std::collections::BTreeMap;

    let temp = tempfile::tempdir().expect("tempdir");
    let source_root = temp.path();
    let app_root = source_root.join("apps/demo");
    std::fs::create_dir_all(source_root.join("stock/templates/cockpit"))
        .expect("create template dir");
    std::fs::create_dir_all(&app_root).expect("create app dir");
    std::fs::write(source_root.join("workspace.json"), "{}").expect("write workspace config");
    std::fs::write(
        source_root.join("stock/templates/cockpit/main.mei"),
        "scene(id = \"preview\")",
    )
    .expect("write preview template");
    let mut templates = BTreeMap::new();
    templates.insert(
        "cockpit.main".to_string(),
        TemplateCatalogEntry {
            template_key: "cockpit.main".to_string(),
            template_file: "stock/templates/cockpit/main.mei".to_string(),
            category: "component".to_string(),
            props_schema: Vec::new(),
            variants: Vec::new(),
            consumers: vec!["home/header".to_string()],
            consumer_anchors: vec![TemplateConsumerAnchor {
                scene_id: "home".to_string(),
                panel_path: "header".to_string(),
                block_id: "cockpit.main~0".to_string(),
                label: "Header".to_string(),
            }],
            agent_hint: None,
            preview_mei: None,
        },
    );
    let compiled = CompiledApp {
        app_id: "demo".to_string(),
        title: "demo".to_string(),
        app_root: app_root.to_string_lossy().to_string(),
        scene_routes: vec![CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "scenes/home.mei".to_string(),
            kind: "file_ref".to_string(),
            title: Some("Home".to_string()),
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
        build_template_index: BuildTemplateIndex { templates },
        ui_layout_index: Default::default(),
    };
    let node = BuildNodeId::template("cockpit/main.mei");
    let coord = compile_coordinate_for_node(&node, &compiled).expect("coord");
    assert_eq!(coord.scene_id, None);
    assert_eq!(
        coord.preview_target,
        "../../stock/templates/cockpit/main.mei"
    );
    assert_eq!(coord.preview_kind, BuildPreviewKind::Script);
}

#[test]
fn block_instance_id_always_includes_ordinal() {
    let block = BlockDecl {
        kind: "block".to_string(),
        use_key: "mei.text".to_string(),
        id: Some("mei.text".to_string()),
        title: None,
        area: None,
        props: serde_json::Value::Null,
        base: None,
        layout: None,
        blocks: Vec::new(),
        component: None,
        placement: None,
        interactions: Vec::new(),
        lifecycle: None,
        constraints: None,
        data: None,
    };
    assert_eq!(block_instance_id(&block, 0), "mei.text~0");
    assert_eq!(block_instance_id(&block, 1), "mei.text~1");
}

#[test]
fn ws_hello_chart_bar_resolves_authoring_example_preview() {
    use crate::compile::build_experience::preview_target_from_build_node_with_app;
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};
    use crate::mei_config::WORKSPACE_CONFIG_FILENAME;
    use crate::model::BuildNodeId;

    let Some(source_root) = optional_external_workspace() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    if !source_root.join(WORKSPACE_CONFIG_FILENAME).is_file() {
        eprintln!("skip: workspace config missing under MEI_TEST_WORKSPACE");
        return;
    }
    let app_root = source_root.join("apps").join("hello");
    if !app_root.is_dir() {
        eprintln!("skip: apps/hello missing under MEI_TEST_WORKSPACE");
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile hello");
    let node = BuildNodeId::template("chart.bar");
    let preview = preview_target_from_build_node_with_app(&node, Some(&compiled))
        .expect("chart.bar preview target");
    assert!(
        preview.contains("chart-baseline.mei"),
        "chart.bar should preview stock authoring example, got {preview}"
    );
}

#[test]
fn ws_hello_doc_markdown_resolves_scene_consumer_preview() {
    use crate::compile::build_experience::{
        compile_coordinate_for_node, preview_target_from_build_node_with_app,
    };
    use crate::compile::build_node_context::resolve_build_node_context;
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};
    use crate::mei_config::WORKSPACE_CONFIG_FILENAME;
    use crate::model::BuildNodeId;

    let Some(source_root) = optional_external_workspace() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    if !source_root.join(WORKSPACE_CONFIG_FILENAME).is_file() {
        eprintln!("skip: workspace config missing under MEI_TEST_WORKSPACE");
        return;
    }
    let app_root = source_root.join("apps").join("hello");
    if !app_root.is_dir() {
        eprintln!("skip: apps/hello missing under MEI_TEST_WORKSPACE");
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile hello");
    let node = BuildNodeId::template("doc.markdown");
    let entry = compiled.build_template_index.lookup("doc.markdown");
    assert!(
        entry.is_some(),
        "doc.markdown should be indexed from home scene compile"
    );
    assert!(
        !entry.expect("entry").consumer_anchors.is_empty(),
        "doc.markdown should have consumer anchors from home scene"
    );
    let preview =
        preview_target_from_build_node_with_app(&node, Some(&compiled)).expect("preview target");
    assert!(
        preview.contains("dataset-baseline.mei") || preview.contains("home"),
        "doc.markdown should preview authoring example or home consumer scene, got {preview}"
    );
    let ctx = resolve_build_node_context(&compiled, &node);
    assert!(
        ctx.target_file.ends_with(".mei"),
        "build context should not fall back to raw js, got {}",
        ctx.target_file
    );
    let coord = compile_coordinate_for_node(&node, &compiled).expect("coord");
    assert!(
        coord.preview_target.ends_with(".mei"),
        "coord preview should be scene mei, got {}",
        coord.preview_target
    );
}

#[test]
fn v2_template_file_preview_resolves_stock_templates_path() {
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};
    use crate::mei_config::WORKSPACE_CONFIG_FILENAME;
    use crate::model::BuildNodeId;

    let Some(source_root) = optional_external_workspace() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    if !source_root.join(WORKSPACE_CONFIG_FILENAME).is_file() {
        eprintln!("skip: workspace config missing under MEI_TEST_WORKSPACE");
        return;
    }
    let app_root = source_root.join("apps").join("hello");
    if !app_root.is_dir() {
        eprintln!("skip: apps/hello missing under MEI_TEST_WORKSPACE");
        return;
    }
    let template_key = "cockpit/metric-card.mei";
    if !source_root
        .join("stock/templates")
        .join(template_key)
        .is_file()
    {
        eprintln!("skip: stock template missing under MEI_TEST_WORKSPACE");
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile hello");
    let node = BuildNodeId::template(template_key);
    let target = preview_target_from_build_node_with_app(&node, Some(&compiled))
        .expect("template preview target");
    assert!(
        target.contains("metric-card.mei"),
        "expected stock template path, got {target}"
    );
    assert!(
        preview_target_relative_to_app(&compiled, &target)
            .is_some_and(|rel| rel.contains("metric-card.mei")),
        "preview target should compile from app-relative stock path"
    );
}
