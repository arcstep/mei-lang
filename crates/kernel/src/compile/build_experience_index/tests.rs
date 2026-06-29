use super::*;

use crate::model::{
    BlockDecl, BuildExperienceIndex, BuildNodeId, CompiledApp, CompiledSceneRoute,
    ReachabilityTreeNodeSnapshot, ReachabilityTreeRootSnapshot, SceneContract, SceneDecl, UiNodeDecl,
    PanelDecl,
};
use serde_json::Value;
use std::collections::BTreeMap;

fn sample_scene_contract(panels: Vec<PanelDecl>) -> SceneContract {
    SceneContract {
        scene: SceneDecl {
            kind: "scene".to_string(),
            id: "home".to_string(),
            profile: Some("cockpit".to_string()),
            state: Value::Null,
            world: None,
            flow: None,
            frame: None,
            theme: None,
            summary: None,
            goal: None,
            shared: Value::Null,
            local_nav: Value::Null,
            params: Value::Null,
            capabilities: Value::Null,
            bindings: Value::Null,
            examples: Value::Null,
            access_export: true,
        },
        themes: Vec::new(),
        shared: Value::Null,
        world: None,
        flow: None,
        frame: None,
        panels,
    }
}

#[test]
fn experience_index_expands_nested_panels() {
    let inner = PanelDecl {
        kind: "panel".to_string(),
        id: "supervision_warning_stats".to_string(),
        title: Some("监督预警".to_string()),
        head: None,
        area: Some("warning".to_string()),
        layout: None,
        blocks: vec![UiNodeDecl::Block(BlockDecl {
            kind: "block".to_string(),
            use_key: "cockpit.metric-card".to_string(),
            id: Some("card_one".to_string()),
            title: Some("预警数".to_string()),
            area: None,
            props: Value::Null,
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
        slot: None,
        props: Value::Null,
        head_props: Value::Null,
        body_props: Value::Null,
        base: None,
        import_scope: Some("scenes/05-监督预警.mei".to_string()),
    };
    let shell = PanelDecl {
        kind: "panel".to_string(),
        id: "right_rail_float".to_string(),
        title: None,
        head: None,
        area: Some("body".to_string()),
        layout: None,
        blocks: vec![UiNodeDecl::Panel(inner)],
        slot: None,
        props: serde_json::json!({
            "position": "absolute",
            "top": "84px",
            "right": "0",
        }),
        head_props: Value::Null,
        body_props: Value::Null,
        base: None,
        import_scope: Some("scenes/layout-右栏.mei".to_string()),
    };
    let mut contracts = BTreeMap::new();
    contracts.insert("home".to_string(), sample_scene_contract(vec![shell]));
    let routes = vec![CompiledSceneRoute {
        scene_id: "home".to_string(),
        frame_id: None,
        target_file: "scenes/home.mei".to_string(),
        kind: "file_ref".to_string(),
        title: Some("首页".to_string()),
        is_default: true,
        access_export: true,
    }];
    let compiled_stub = CompiledApp {
        app_id: "demo".to_string(),
        title: "demo".to_string(),
        app_root: ".".to_string(),
        scene_routes: routes.clone(),
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
        build_experience_index: BuildExperienceIndex::default(),
        build_board_index: Default::default(),
        build_template_index: Default::default(),
    };
    let index = build_experience_index(&routes, &BTreeMap::new(), &contracts, &compiled_stub);
    let nested_id =
        BuildNodeId::scene_panel("home", "right_rail_float/supervision_warning_stats").encode();
    let nested = index
        .node_manifest
        .get(&nested_id)
        .expect("nested panel manifest");
    assert_eq!(nested.label, "监督预警");
    assert!(nested
        .mount_chain
        .iter()
        .any(|entry| entry.file.contains("05-监督预警")));
    let shell_id = BuildNodeId::scene_panel("home", "right_rail_float").encode();
    let shell = index
        .node_manifest
        .get(&shell_id)
        .expect("shell panel manifest");
    assert!(shell
        .mount_chain
        .iter()
        .any(|entry| entry.file.contains("layout")));
    let scenes = &index.reachability_snapshot[0];
    let home = scenes.children.first().expect("home scene");
    let panels = home
        .children
        .iter()
        .find(|node| node.label == "Panels")
        .expect("panels group");
    assert_eq!(panels.children.len(), 1);
    assert_eq!(panels.children[0].children.len(), 1);
    assert_eq!(panels.children[0].children[0].label, "监督预警");
}

#[test]
fn business_app_strips_legacy_templates_snapshot_on_read() {
    use std::path::Path;

    use crate::compile::{compile_app_from_root_with_options, CompileOptions};

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
        .join("workspaces")
        .join("ws-hello");
    let app_root = source_root.join("apps").join("hello");
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile hello");
    let mut legacy = compiled.clone();
    legacy.build_experience_index.reachability_snapshot.push(
        ReachabilityTreeRootSnapshot {
            group: "templates".to_string(),
            label: "Components".to_string(),
            default_open: false,
            children: vec![ReachabilityTreeNodeSnapshot {
                id: "legacy-component".to_string(),
                node_id: "component:chart.line".to_string(),
                kind: "component".to_string(),
                label: "chart.line".to_string(),
                badges: Vec::new(),
                compile_scene: String::new(),
                compile_target: String::new(),
                board_layout_zone: String::new(),
                children: Vec::new(),
            }],
        },
    );
    let roots = reachability_roots_from_compiled(&legacy);
    assert!(
        roots.iter().all(|root| root.group != "templates"),
        "legacy templates snapshot must be stripped for business apps"
    );
}

#[test]
fn stale_snapshot_rebuilds_boards_and_templates_groups() {
    use std::path::Path;

    use crate::compile::{compile_app_from_root_with_options, CompileOptions};

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
        .join("workspaces")
        .join("ws-hello");
    let app_root = source_root.join("apps").join("hello");
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile hello");
    let mut stale = compiled;
    stale.build_experience_index = BuildExperienceIndex::default();
    stale.build_board_index = Default::default();
    stale.build_template_index = Default::default();

    let roots = reachability_roots_from_compiled(&stale);
    let groups: Vec<_> = roots.iter().map(|root| root.group.as_str()).collect();
    assert!(
        groups.contains(&"scenes"),
        "expected scenes group after stale rebuild, got {groups:?}"
    );
    assert!(
        !groups.contains(&"templates"),
        "business app stale rebuild should not inject stock templates, got {groups:?}"
    );
    assert!(
        !groups.contains(&"template_files"),
        "business app stale rebuild should not inject stock template files, got {groups:?}"
    );
    assert!(
        !groups.contains(&"components"),
        "stale rebuild should not fall back to legacy runtime-only components group"
    );
}

#[test]
fn partial_snapshot_restores_templates_from_component_assets() {
    use std::path::Path;

    use crate::compile::{compile_app_from_root_with_options, CompileOptions};

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
        .join("workspaces")
        .join("ws-hello");
    let app_root = source_root.join("apps").join("_stock-catalog");
    if !app_root.is_dir() {
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile stock catalog");
    assert!(
        !compiled.component_assets.is_empty(),
        "fixture should expose component assets"
    );
    let mut partial = compiled.clone();
    partial.build_template_index = Default::default();
    partial
        .build_experience_index
        .reachability_snapshot
        .retain(|root| root.group != "templates");

    let roots = reachability_roots_from_compiled(&partial);
    assert!(
        roots.iter().any(|root| root.group == "templates"),
        "templates group should be restored from component_assets, groups: {:?}",
        roots
            .iter()
            .map(|root| root.group.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn empty_templates_snapshot_is_treated_as_stale() {
    use std::path::Path;

    use crate::compile::{compile_app_from_root_with_options, CompileOptions};

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
        .join("workspaces")
        .join("ws-hello");
    let app_root = source_root.join("apps").join("_stock-catalog");
    if !app_root.is_dir() {
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile stock catalog");
    let mut partial = compiled.clone();
    partial.build_experience_index.reachability_snapshot = partial
        .build_experience_index
        .reachability_snapshot
        .into_iter()
        .map(|mut root| {
            if root.group == "templates" {
                root.children.clear();
            }
            root
        })
        .collect();

    let roots = reachability_roots_from_compiled(&partial);
    let templates = roots
        .iter()
        .find(|root| root.group == "templates")
        .expect("templates group");
    assert!(
        !templates.children.is_empty(),
        "empty templates snapshot should be rebuilt with component catalog entries"
    );
}

#[test]
fn v2_app_root_hydrates_stock_components_and_templates_in_build_tree() {
    use std::path::Path;

    use crate::compile::{compile_app_from_root_with_options, CompileOptions};
    use crate::mei_config::WORKSPACE_CONFIG_FILENAME;

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
        .join("workspaces")
        .join("ws-hello");
    if !source_root.join(WORKSPACE_CONFIG_FILENAME).is_file() {
        return;
    }
    let app_root = source_root.join("apps").join("hello");
    if !app_root.is_dir() {
        return;
    }
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions::default(),
    )
    .expect("compile hello");
    let roots = reachability_roots_from_compiled(&compiled);
    assert!(
        roots.iter().all(|root| root.group != "templates"),
        "business app build tree should not include stock component catalog"
    );
    assert!(
        roots.iter().all(|root| root.group != "template_files"),
        "business app build tree should not include stock template files"
    );
}

#[test]
fn stock_catalog_app_hydrates_components_and_templates_in_build_tree() {
    use std::path::Path;

    use crate::compile::{compile_app_from_root_with_options, CompileOptions};
    use crate::mei_config::WORKSPACE_CONFIG_FILENAME;

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
        .join("workspaces")
        .join("ws-hello");
    if !source_root.join(WORKSPACE_CONFIG_FILENAME).is_file() {
        return;
    }
    let app_root = source_root.join("apps").join("_stock-catalog");
    if !app_root.is_dir() {
        return;
    }
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions::default(),
    )
    .expect("compile stock catalog");
    let roots = reachability_roots_from_compiled(&compiled);
    let templates = roots
        .iter()
        .find(|root| root.group == "templates")
        .expect("templates/components group");
    assert!(
        !templates.children.is_empty(),
        "stock catalog app should hydrate stock components into build tree"
    );
    let template_files = roots
        .iter()
        .find(|root| root.group == "template_files")
        .expect("template_files group");
    assert!(
        !template_files.children.is_empty(),
        "stock catalog app should list stock template files in build tree"
    );
}

#[test]
fn templates_group_renders_as_components_label() {
    use std::path::Path;

    use crate::compile::{compile_app_from_root_with_options, CompileOptions};

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
        .join("workspaces")
        .join("ws-hello");
    let app_root = source_root.join("apps").join("_stock-catalog");
    if !app_root.is_dir() {
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile stock catalog");
    let roots = reachability_roots_from_compiled(&compiled);
    let components = roots
        .iter()
        .find(|root| root.group == "templates")
        .expect("templates/components group");
    assert_eq!(components.label, "Components");
}
