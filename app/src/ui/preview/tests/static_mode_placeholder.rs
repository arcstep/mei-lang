//! Static `data_mode` SSR metric placeholders.

use std::collections::BTreeMap;

use super::resolve::{resolve_value, RuntimeSceneAnchor};
use mei_lang_kernel::{
    build_runtime_analysis_contracts, build_runtime_analysis_graph, build_runtime_resource_index,
    build_runtime_resource_map, CompiledApp, LoadedResource, MetricContract, MetricShape,
    SceneContract, SceneDecl, SourceDecl, WorldMetricLedgerEntry,
};
use serde_json::{json, Value};

#[test]
fn static_mode_metric_ref_returns_skeleton_and_ignores_patch() {
    let compiled = static_metric_fixture_compiled();
    let resource_map = build_runtime_resource_map(&compiled);
    let resource_index = build_runtime_resource_index(&compiled);
    let scene_contract = compiled
        .scene_contract
        .clone()
        .expect("scene contract");
    let scene_anchor = RuntimeSceneAnchor {
        scene_id: "home".to_string(),
        scene_path: Some("scenes/home.mei".to_string()),
    };
    let resolved = resolve_value(
        &json!({
            "__ref": "metric",
            "id": "sales_total",
            "from_dataset": "sales_metrics"
        }),
        &json!({}),
        &scene_contract,
        &resource_map,
        &scene_anchor,
        &resource_index,
        &compiled,
        true,
        Some("static"),
    );
    assert_eq!(
        resolved.get("value").and_then(Value::as_str),
        Some("xxxx"),
        "static mode must not leak eval metric value"
    );
    assert_eq!(
        resolved.get("label").and_then(Value::as_str),
        Some("销售总额")
    );
    assert_eq!(
        resolved.get("__mei_data_origin").and_then(Value::as_str),
        Some("static_skeleton")
    );

    let with_patch = resolve_value(
        &json!({
            "metric_ref": {
                "__ref": "metric",
                "id": "sales_total",
                "from_dataset": "sales_metrics"
            },
            "patch": { "value": "23" }
        }),
        &json!({}),
        &scene_contract,
        &resource_map,
        &scene_anchor,
        &resource_index,
        &compiled,
        true,
        Some("static"),
    );
    assert_eq!(
        with_patch
            .get("metric_ref")
            .and_then(|value| value.get("value"))
            .and_then(Value::as_str),
        Some("xxxx"),
        "static mode metric_ref child must use skeleton value"
    );
    assert!(
        with_patch.get("patch").and_then(|value| value.get("value")).is_none(),
        "static mode must strip eval patch.value"
    );
}

fn static_metric_fixture_compiled() -> CompiledApp {
    use mei_lang_kernel::DatasetView;
    let scene_contract = SceneContract {
        scene: SceneDecl {
            kind: "scene".to_string(),
            id: "home".to_string(),
            world: None,
            flow: None,
            frame: None,
            profile: None,
            theme: None,
            summary: None,
            goal: None,
            state: json!({}),
            shared: json!({}),
            local_nav: json!({}),
            params: json!({}),
            capabilities: Value::Null,
            bindings: json!({}),
            examples: json!([]),
            access_export: true,
        },
        themes: vec![],
        shared: json!({}),
        world: None,
        flow: None,
        frame: None,
        panels: vec![],
    };
    let scalar_metric = MetricContract {
        id: "sales_total".to_string(),
        label: Some("销售总额".to_string()),
        unit: Some("元".to_string()),
        value_format: None,
        purpose: None,
        shape: MetricShape::Scalar,
        schema: Vec::new(),
        dataset: None,
        transforms: Vec::new(),
        value: json!({"value": 100}),
    };
    let runtime_def = json!({"id": "sales_total", "shape": "scalar", "value": 999});
    let runtime_metric_defs = BTreeMap::from([("sales_total".to_string(), runtime_def)]);
    let runtime_analysis_graph =
        build_runtime_analysis_graph(&runtime_metric_defs, "sales_metrics");
    let runtime_analysis_contracts =
        build_runtime_analysis_contracts(&runtime_metric_defs, "sales_metrics");
    let resource = LoadedResource {
        id: "sales_metrics".to_string(),
        kind: "dataset".to_string(),
        title: None,
        document: None,
        dataset: Some(DatasetView {
            id: "sales_metrics".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["value".to_string()],
            rows: vec![json!({"value": 100})],
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:sales_metrics".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::from([("sales_total".to_string(), scalar_metric.clone())]),
            runtime_metric_defs,
            runtime_analysis_graph,
            runtime_analysis_contracts,
        }),
    };
    CompiledApp {
        app_id: "preview-explain".to_string(),
        active_scene: Some("home".to_string()),
        active_target_file: "scenes/home.mei".to_string(),
        resources: vec![resource],
        world_metrics: BTreeMap::from([(
            "sales_total".to_string(),
            WorldMetricLedgerEntry {
                id: "sales_total".to_string(),
                owner_resource_id: "sales_metrics".to_string(),
                order: 1,
                metric: scalar_metric,
            },
        )]),
        world_semantic_by_file: BTreeMap::new(),
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview-explain".to_string(),
        file_tree: Vec::new(),
        scene_contract: Some(scene_contract),
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
