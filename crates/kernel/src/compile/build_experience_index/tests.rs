use super::*;

use crate::model::{
    BuildExperienceIndex, CompiledApp, CompiledSceneRoute, ReachabilityTreeNodeSnapshot,
    ReachabilityTreeRootSnapshot, SceneContract, SceneDecl, UiNodeDecl,
};
use serde_json::Value;
use std::collections::BTreeMap;


fn optional_external_workspace() -> Option<std::path::PathBuf> {
    let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
    let path = std::path::PathBuf::from(raw.trim());
    if path.as_os_str().is_empty() || !path.is_dir() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

fn ws_hello_root() -> Option<std::path::PathBuf> {
    let ws = optional_external_workspace()?;
    if ws.join("workspace.json").is_file() || ws.join("apps").is_dir() {
        Some(ws)
    } else {
        None
    }
}

fn sample_scene_contract(panels: Vec<UiNodeDecl>) -> SceneContract {
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
        t2_pages: Vec::new(),
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
fn experience_index_dedupes_scenes_and_omits_panels_subtree() {
    let mut contracts = BTreeMap::new();
    contracts.insert(
        "home".to_string(),
        sample_scene_contract(vec![UiNodeDecl {
            kind: "panel".to_string(),
            id: "header".to_string(),
            title: Some("Header".to_string()),
            head: None,
            area: Some("header".to_string()),
            layout: None,
            blocks: Vec::new(),
            slot: None,
            props: Value::Null,
            head_props: Value::Null,
            body_props: Value::Null,
            base: None,
            import_scope: None,
        }]),
    );
    let routes = vec![
        CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "scenes/home.mei".to_string(),
            kind: "file_ref".to_string(),
            title: Some("首页".to_string()),
            is_default: true,
            access_export: true,
        },
        CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "scenes/home-alt.mei".to_string(),
            kind: "file_ref".to_string(),
            title: Some("首页副本".to_string()),
            is_default: false,
            access_export: true,
        },
    ];
    let compiled_stub = CompiledApp {
        app_id: "demo".to_string(),
        title: "demo".to_string(),
        app_root: ".".to_string(),
        scene_routes: routes.clone(),
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
        build_experience_index: BuildExperienceIndex::default(),
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    let index = build_experience_index(&routes, &BTreeMap::new(), &contracts, &compiled_stub);
    let scenes = &index.reachability_snapshot[0];
    assert_eq!(
        scenes.children.len(),
        1,
        "duplicate scene_id routes should collapse"
    );
    let home = scenes.children.first().expect("home scene");
    assert!(
        !home.children.iter().any(|node| node.label == "Panels"),
        "experience index should not expose Panels subtree"
    );
    assert!(
        index.node_manifest.is_empty(),
        "panel manifests should not be populated without Panels subtree"
    );
}

#[test]
fn business_app_strips_legacy_templates_snapshot_on_read() {
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};

    let Some(source_root) = ws_hello_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let app_root = source_root.join("apps").join("hello");
    if !app_root.is_dir() {
        eprintln!("skip: apps/hello missing under MEI_TEST_WORKSPACE");
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile hello");
    let mut legacy = compiled.clone();
    legacy
        .build_experience_index
        .reachability_snapshot
        .push(ReachabilityTreeRootSnapshot {
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
                ui_role: String::new(),
                preview_scope: String::new(),
                plane_tier: String::new(),
                source_file: String::new(),
                source_symbol: String::new(),
                children: Vec::new(),
            }],
        });
    let roots = reachability_roots_from_compiled(&legacy);
    assert!(
        roots.iter().all(|root| root.group != "templates"),
        "legacy templates snapshot must be stripped for business apps"
    );
}

#[test]
fn stale_snapshot_rebuilds_boards_and_templates_groups() {
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};

    let Some(source_root) = ws_hello_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let app_root = source_root.join("apps").join("hello");
    if !app_root.is_dir() {
        eprintln!("skip: apps/hello missing under MEI_TEST_WORKSPACE");
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile hello");
    let mut stale = compiled;
    stale.build_experience_index = BuildExperienceIndex::default();
    stale.build_t2_page_index = Default::default();
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
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};

    let Some(source_root) = ws_hello_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
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
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};

    let Some(source_root) = ws_hello_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
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
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};
    use crate::mei_config::WORKSPACE_CONFIG_FILENAME;

    let Some(source_root) = ws_hello_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    if !source_root.join(WORKSPACE_CONFIG_FILENAME).is_file() {
        return;
    }
    let app_root = source_root.join("apps").join("hello");
    if !app_root.is_dir() {
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
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
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};
    use crate::mei_config::WORKSPACE_CONFIG_FILENAME;

    let Some(source_root) = ws_hello_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    if !source_root.join(WORKSPACE_CONFIG_FILENAME).is_file() {
        return;
    }
    let app_root = source_root.join("apps").join("_stock-catalog");
    if !app_root.is_dir() {
        return;
    }
    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
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
    use crate::compile::{compile_app_from_root_with_options, CompileOptions};

    let Some(source_root) = ws_hello_root() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
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
