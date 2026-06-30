use super::*;

use crate::model::{
    BlockDecl, ComponentAsset, PanelDecl, SceneContract, TemplateConsumerAnchor,
};
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn template_index_lists_metric_card_assets() {
    let assets = vec![ComponentAsset {
        key: "cockpit.metric-card".to_string(),
        tag: "div".to_string(),
        script: "templates/cockpit/metric-card.mei".to_string(),
        pack_path: "cockpit".to_string(),
        preview_mei: None,
    }];
    let result = build_template_index(&assets, &BTreeMap::new(), &BTreeMap::new());
    let entry = result
        .index
        .templates
        .get("cockpit.metric-card")
        .expect("template");
    assert_eq!(entry.category, "metric_card");
    assert!(entry
        .agent_hint
        .as_deref()
        .is_some_and(|hint| hint.contains("metric-card")));
    assert!(!result.tree_root.children.is_empty());
}

#[test]
fn template_index_collects_consumer_anchors() {
    use crate::model::{SceneDecl, UiNodeDecl};
    let assets = vec![ComponentAsset {
        key: "cockpit.header-brand".to_string(),
        tag: "div".to_string(),
        script: "templates/cockpit/header-brand.mei".to_string(),
        pack_path: "cockpit".to_string(),
        preview_mei: None,
    }];
    let mut contracts = BTreeMap::new();
    contracts.insert(
        "home".to_string(),
        SceneContract {
            scene: SceneDecl {
                kind: "scene".to_string(),
                id: "home".to_string(),
                profile: None,
                state: serde_json::Value::Null,
                world: None,
                flow: None,
                frame: None,
                theme: None,
                summary: None,
                goal: None,
                shared: serde_json::Value::Null,
                local_nav: serde_json::Value::Null,
                params: serde_json::Value::Null,
                capabilities: serde_json::Value::Null,
                bindings: serde_json::Value::Null,
                examples: serde_json::Value::Null,
                access_export: true,
            },
            themes: Vec::new(),
            shared: serde_json::Value::Null,
            world: None,
            flow: None,
            frame: None,
            panels: vec![PanelDecl {
                id: "header".to_string(),
                blocks: vec![UiNodeDecl::Block(BlockDecl {
                    kind: "block".to_string(),
                    use_key: "cockpit.header-brand".to_string(),
                    id: None,
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
                })],
                ..Default::default()
            }],
        },
    );
    let result = build_template_index(&assets, &contracts, &BTreeMap::new());
    let entry = result
        .index
        .templates
        .get("cockpit.header-brand")
        .expect("template");
    assert_eq!(entry.consumer_anchors.len(), 1);
    assert_eq!(entry.consumer_anchors[0].scene_id, "home");
    assert_eq!(entry.consumer_anchors[0].panel_path, "header");
}

#[test]
fn js_component_authoring_preview_targets_stock_example() {
    use std::path::Path;

    use crate::compile::{
        compile_app_from_root_with_options, compile_coordinate_for_node, BuildPreviewKind,
        CompileOptions,
    };
    use crate::model::BuildNodeId;

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
        .join("workspaces")
        .join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let preview = source_root.join("stock/components/chart/echarts/previews/chart.area.mei");
    if !app_root.is_dir() || !preview.is_file() {
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile zhifa");
    let node = BuildNodeId::component("chart.area");
    let target = authoring_preview_target_for_template(&compiled, "chart.area");
    assert!(
        target
            .as_deref()
            .is_some_and(|file| file.contains("chart.area.mei")),
        "expected pack preview, got {target:?}"
    );
    let coord = compile_coordinate_for_node(&node, &compiled).expect("coord");
    assert_eq!(coord.preview_kind, BuildPreviewKind::Script);
    assert!(
        coord.preview_target.contains("chart.area.mei"),
        "coord target should be pack preview, got {}",
        coord.preview_target
    );
    let preview_compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: coord.preview_target.clone().into(),
        },
    )
    .expect("compile chart.area authoring preview");
    let errors: Vec<_> = preview_compiled
        .diagnostics
        .iter()
        .filter(|diag| matches!(diag.severity, crate::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "chart.area authoring preview should compile cleanly: {errors:?}"
    );
    assert!(
        preview_compiled.scene_contract.is_some(),
        "chart.area authoring preview should yield scene contract"
    );
}

#[test]
fn ws_hello_chart_area_authoring_preview_coordinate() {
    use std::path::Path;

    use crate::compile::{
        compile_app_from_root_with_options, compile_coordinate_for_node, BuildPreviewKind,
        CompileOptions,
    };
    use crate::model::BuildNodeId;

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root")
        .join("workspaces")
        .join("ws-hello");
    let app_root = source_root.join("apps").join("hello");
    let preview = source_root.join("stock/components/chart/echarts/previews/chart.area.mei");
    if !app_root.is_dir() || !preview.is_file() {
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile ws-hello hello");
    let node = BuildNodeId::component("chart.area");
    let coord = compile_coordinate_for_node(&node, &compiled).expect("coord");
    assert_eq!(coord.preview_kind, BuildPreviewKind::Script);
    assert!(
        coord.preview_target.contains("chart.area.mei"),
        "expected pack preview path, got {}",
        coord.preview_target
    );
    assert!(
        coord.preview_target.contains("stock/components/chart/echarts/previews"),
        "expected pack-local preview, got {}",
        coord.preview_target
    );
}

#[test]
fn components_tree_groups_by_pack_path() {
    let assets = vec![
        ComponentAsset {
            key: "chart.line".to_string(),
            tag: "t".to_string(),
            script: "chart/echarts/line.js".to_string(),
            pack_path: "chart/echarts".to_string(),
            preview_mei: Some(
                "stock/components/chart/echarts/previews/chart.line.mei".to_string(),
            ),
        },
        ComponentAsset {
            key: "chart.area".to_string(),
            tag: "t".to_string(),
            script: "chart/echarts/area.js".to_string(),
            pack_path: "chart/echarts".to_string(),
            preview_mei: Some(
                "stock/components/chart/echarts/previews/chart.area.mei".to_string(),
            ),
        },
    ];
    let result = build_template_index(&assets, &BTreeMap::new(), &BTreeMap::new());
    assert_eq!(result.tree_root.children.len(), 1);
    assert_eq!(result.tree_root.children[0].kind, "component_pack");
    assert_eq!(result.tree_root.children[0].label, "chart/echarts");
    assert_eq!(result.tree_root.children[0].children.len(), 2);
}

#[test]
fn template_preview_targets_primary_consumer_scene() {
    use std::path::Path;

    use crate::compile::{
        compile_app_from_root_with_options, compile_coordinate_for_node, BuildPreviewKind,
        CompileOptions,
    };
    use crate::model::BuildNodeId;

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
        .join("workspaces")
        .join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    if !app_root.is_dir() {
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile zhifa");
    let node = BuildNodeId::template("cockpit.header-brand");
    let target = preview_target_for_template_consumer(&compiled, "cockpit.header-brand");
    assert!(
        target.as_deref().is_some_and(|file| file.contains("home")),
        "expected home scene file, got {target:?}"
    );
    let scene = preview_scene_id_for_template_consumer(&compiled, "cockpit.header-brand");
    assert_eq!(scene.as_deref(), Some("home"));
    let coord = compile_coordinate_for_node(&node, &compiled).expect("coord");
    let cockpit_example = source_root.join("stock/authoring/examples/cockpit-panel.mei");
    if cockpit_example.is_file() {
        assert_eq!(coord.preview_kind, BuildPreviewKind::Script);
        assert!(coord.preview_target.contains("cockpit-panel.mei"));
    } else {
        assert_eq!(coord.preview_kind, BuildPreviewKind::SceneCapsule);
    }
}

#[test]
fn template_file_authoring_preview_targets_template_mei() {
    use std::collections::BTreeMap;

    use crate::model::{
        BuildTemplateIndex, CompiledApp, CompiledSceneRoute, TemplateCatalogEntry,
    };

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("workspaces")
        .join("ws-spbjw");
    let app_root = source_root.join("zhifa");
    let template_mei = source_root.join("stock/templates/cockpit/main.mei");
    if !app_root.is_dir() || !template_mei.is_file() {
        return;
    }

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
        app_root: app_root.display().to_string(),
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
        build_board_index: Default::default(),
        build_template_index: BuildTemplateIndex { templates },
    };
    assert_eq!(
        authoring_preview_target_for_template(&compiled, "cockpit/main.mei").as_deref(),
        Some("../stock/templates/cockpit/main.mei")
    );
    assert_eq!(
        preview_scene_id_for_template_file_consumer(&compiled, "cockpit/main.mei").as_deref(),
        Some("home")
    );
    assert_eq!(
        preview_target_for_template_file_consumer(&compiled, "cockpit/main.mei").as_deref(),
        Some("scenes/home.mei")
    );
}
