use super::*;

use crate::model::{BlockDecl, BuildNodeId, BuildNodeKind, CompiledApp, UiNodeDecl, UiTreeNode};
use serde_json::Value;
use std::collections::BTreeMap;

#[test]
fn reachability_tree_includes_routes_and_world() {
    let mut compiled = CompiledApp {
        app_id: "demo".to_string(),
        title: "demo".to_string(),
        app_root: "/__mei_test_missing_app_root__".to_string(),
        scene_routes: vec![crate::model::CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "scenes/home.mei".to_string(),
            kind: "file_ref".to_string(),
            title: Some("Home".to_string()),
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
    };
    compiled.world_semantic_by_file.insert(
        "metrics.world.mei".to_string(),
        crate::model::WorldSemanticFileIndex {
            world_id: Some("metrics".to_string()),
            datasets: vec![],
            metrics: vec![crate::model::WorldSemanticMetric {
                id: "total".to_string(),
                label: Some("Total".to_string()),
                unit: None,
                note: None,
                explain: vec![],
            }],
            resource_id: "__world_metrics__".to_string(),
        },
    );
    let roots = build_reachability_tree(&compiled);
    assert_eq!(roots.len(), 5);
    assert!(!roots[0].default_open);
    assert_eq!(roots[0].group, "scenes");
    assert_eq!(roots[1].group, "routes");
    assert_eq!(roots[1].children.len(), 1);
    assert!(!roots[2].default_open);
    assert_eq!(roots[2].label, "Backing · World");
    assert_eq!(roots[2].children.len(), 1);
}

#[test]
fn reachability_tree_expands_scene_panels_from_assembly() {
    let panel = UiNodeDecl {
        kind: "panel".to_string(),
        id: "kpi_row".to_string(),
        title: Some("KPI 行".to_string()),
        head: None,
        area: None,
        layout: None,
        blocks: vec![UiTreeNode::Block(BlockDecl {
            kind: "component".to_string(),
            use_key: "cockpit.metric-card".to_string(),
            id: Some("pending_card".to_string()),
            title: Some("待办数".to_string()),
            area: None,
            props: serde_json::json!({
                "metric": { "__ref": "metric", "id": "total", "from_dataset": "agency_objects" }
            }),
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
        import_scope: None,
    };
    let mut assembly = BTreeMap::<String, Value>::new();
    assembly.insert(
        "home".to_string(),
        serde_json::json!({
            "scene_id": "home",
            "panels": [panel],
        }),
    );
    let compiled = CompiledApp {
        app_id: "demo".to_string(),
        title: "demo".to_string(),
        app_root: "/__mei_test_missing_app_root__".to_string(),
        scene_routes: vec![crate::model::CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "scenes/home.mei".to_string(),
            kind: "file_ref".to_string(),
            title: Some("Home".to_string()),
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
        scene_projection_assembly_by_id: assembly,
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
    let roots = build_reachability_tree(&compiled);
    let structure = &roots
        .iter()
        .find(|root| root.group == "ui_structure")
        .expect("ui structure root")
        .children;
    fn find_label<'a>(
        nodes: &'a [ReachabilityTreeNode],
        label: &str,
    ) -> Option<&'a ReachabilityTreeNode> {
        nodes.iter().find_map(|node| {
            (node.label == label)
                .then_some(node)
                .or_else(|| find_label(&node.children, label))
        })
    }
    let panel_node = find_label(structure, "KPI 行").expect("KPI panel");
    let metric_node = find_label(&panel_node.children, "待办数").expect("metric content");
    assert_eq!(metric_node.kind, "ui_scope");
}

fn sample_catalog_scene_node(scene_id: &str, target_file: &str) -> ReachabilityTreeNode {
    ReachabilityTreeNode {
        id: format!("scene-{scene_id}"),
        node_id: BuildNodeId::scene(scene_id).encode(),
        kind: "scene".to_string(),
        label: scene_id.to_string(),
        badges: vec![target_file.to_string()],
        children: Vec::new(),
        ..Default::default()
    }
}

#[test]
fn stock_catalog_filter_narrows_scenes_and_flattens_facet_by_pack() {
    let roots = vec![
        ReachabilityTreeRoot {
            group: "scenes".to_string(),
            label: "Scenes".to_string(),
            default_open: false,
            children: vec![
                sample_catalog_scene_node(
                    "chart.pie",
                    "../../stock/components/chart/echarts/previews/chart.pie.mei",
                ),
                sample_catalog_scene_node(
                    "chart.line",
                    "../../stock/components/chart/line/previews/chart.line.mei",
                ),
            ],
        },
        ReachabilityTreeRoot {
            group: "templates".to_string(),
            label: "Components".to_string(),
            default_open: false,
            children: vec![ReachabilityTreeNode {
                id: "pack-chart-echarts".to_string(),
                node_id: String::new(),
                kind: "component_pack".to_string(),
                label: "chart/echarts".to_string(),
                badges: Vec::new(),
                children: vec![ReachabilityTreeNode {
                    id: "tpl-pie".to_string(),
                    node_id: BuildNodeId::new(BuildNodeKind::Template, "chart.pie.mei").encode(),
                    kind: "template".to_string(),
                    label: "chart.pie".to_string(),
                    badges: Vec::new(),
                    children: Vec::new(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        },
        ReachabilityTreeRoot {
            group: "template_files".to_string(),
            label: "Templates".to_string(),
            default_open: false,
            children: vec![ReachabilityTreeNode {
                id: "tpl-pack".to_string(),
                node_id: String::new(),
                kind: "template_group".to_string(),
                label: "layout/basic".to_string(),
                badges: Vec::new(),
                children: Vec::new(),
                ..Default::default()
            }],
        },
    ];
    let filtered = filter_reachability_roots_for_stock_catalog(
        roots,
        true,
        Some("components"),
        Some("chart/echarts"),
    );
    assert!(
        filtered.iter().all(|root| root.group != "template_files"),
        "components facet should drop template_files root"
    );
    let scenes = filtered
        .iter()
        .find(|root| root.group == "scenes")
        .expect("scenes root");
    assert_eq!(scenes.children.len(), 1);
    assert_eq!(scenes.children[0].label, "chart.pie");
    let components = filtered
        .iter()
        .find(|root| root.group == "templates")
        .expect("components root");
    assert_eq!(components.label, "chart/echarts");
    assert_eq!(components.children.len(), 1);
    assert_eq!(components.children[0].label, "chart.pie");
}
