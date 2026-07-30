use std::collections::BTreeMap;

use super::resolve::{resolve_value, RuntimeSceneAnchor};
use mei_lang_kernel::{
    build_runtime_resource_index, build_runtime_resource_map, ColumnSchema, CompiledApp,
    DatasetView, LoadedResource, MetricContract, MetricShape, SceneContract, SceneDecl, SourceDecl,
};
use serde_json::{json, Value};

#[test]
fn resolve_value_supports_data_and_metric_refs() {
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
            local_nav: serde_json::json!({}),
            params: serde_json::json!({}),
            capabilities: Value::Null,
            bindings: serde_json::json!({}),
            examples: serde_json::json!([]),
            access_export: true,
            t2_pages: Vec::new(),
        },
        themes: vec![],
        shared: json!({}),
        world: None,
        flow: None,
        frame: None,
        panels: vec![],
    };
    let mut resources = BTreeMap::new();
    resources.insert(
        "sales_metrics".to_string(),
        LoadedResource {
            id: "sales_metrics".to_string(),
            kind: "dataset".to_string(),
            title: Some("Sales".to_string()),
            document: None,
            dataset: Some(DatasetView {
                id: "sales_metrics".to_string(),
                title: Some("Sales".to_string()),
                purpose: None,
                schema: vec![
                    ColumnSchema {
                        name: "label".to_string(),
                        type_name: "string".to_string(),
                        source: None,
                        optional: false,
                        unit: None,
                        normalize: None,
                    },
                    ColumnSchema {
                        name: "value".to_string(),
                        type_name: "number".to_string(),
                        source: None,
                        optional: false,
                        unit: Some("元".to_string()),
                        normalize: None,
                    },
                ],
                stage_schema: Vec::new(),
                columns: vec!["label".to_string(), "value".to_string()],
                rows: vec![json!({"label":"A","value":"100"})],
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
                metrics: BTreeMap::from([(
                    "sales_total".to_string(),
                    MetricContract {
                        id: "sales_total".to_string(),
                        label: Some("销售总额".to_string()),
                        unit: Some("元".to_string()),
                        value_format: None,
                        purpose: None,
                        shape: MetricShape::Scalar,
                        schema: vec![ColumnSchema {
                            name: "total_value".to_string(),
                            type_name: "number".to_string(),
                            source: None,
                            optional: false,
                            unit: Some("元".to_string()),
                            normalize: None,
                        }],
                        dataset: None,
                        transforms: Vec::new(),
                        value: json!({"total_value": 100}),
                    },
                )]),
                runtime_metric_defs: BTreeMap::new(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: Default::default(),
            }),
        },
    );

    let compiled = CompiledApp {
        app_id: "preview-test".to_string(),
        active_scene: Some("home".to_string()),
        stage_registry: Default::default(),
        stage_programs: Default::default(),
        scene_slot_modules: Default::default(),
        content_capabilities: Default::default(),
        narration_catalogs: Default::default(),
        active_target_file: "scenes/home.mei".to_string(),
        resources: resources.values().cloned().collect(),
        world_metrics: BTreeMap::from([(
            "sales_total".to_string(),
            mei_lang_kernel::WorldMetricLedgerEntry {
                id: "sales_total".to_string(),
                owner_resource_id: "sales_metrics".to_string(),
                order: 1,
                metric: resources
                    .get("sales_metrics")
                    .and_then(|resource| resource.dataset.as_ref())
                    .and_then(|dataset| dataset.metrics.get("sales_total"))
                    .cloned()
                    .expect("metric"),
            },
        )]),
        world_semantic_by_file: BTreeMap::new(),
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview-test".to_string(),
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
    };
    let resource_index = build_runtime_resource_index(&compiled);
    let scene_anchor = super::resolve::RuntimeSceneAnchor {
        scene_id: "home".to_string(),
        scene_path: Some("scenes/home.mei".to_string()),
    };

    let data_ref = json!({"__ref":"data","id":"sales_metrics"});
    let resolved_data = resolve_value(
        &data_ref,
        &json!({}),
        &scene_contract,
        &resources,
        &scene_anchor,
        &resource_index,
        &compiled,
        false,
        None,
    );
    assert_eq!(
        resolved_data.get("id").and_then(|value| value.as_str()),
        Some("sales_metrics")
    );
    assert_eq!(
        resolved_data
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("dataset_id"))
            .and_then(|value| value.as_str()),
        Some("sales_metrics")
    );

    let metric_ref = json!({"__ref":"metric","id":"sales_total","from_dataset":"sales_metrics"});
    let resolved_metric = resolve_value(
        &metric_ref,
        &json!({}),
        &scene_contract,
        &resources,
        &scene_anchor,
        &resource_index,
        &compiled,
        false,
        None,
    );
    assert_eq!(
        resolved_metric.get("id").and_then(|value| value.as_str()),
        Some("sales_total")
    );
    assert_eq!(
        resolved_metric
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("metric_id"))
            .and_then(|value| value.as_str()),
        Some("sales_total")
    );

    let dataset_ref = json!({"__ref": "dataset", "id": "sales_metrics"});
    let resolved_dataset = resolve_value(
        &dataset_ref,
        &json!({}),
        &scene_contract,
        &resources,
        &scene_anchor,
        &resource_index,
        &compiled,
        false,
        None,
    );
    assert_eq!(
        resolved_dataset.get("id").and_then(|value| value.as_str()),
        Some("sales_metrics")
    );
    assert!(resolved_dataset.get("rows").is_some());
    assert_eq!(
        resolved_dataset
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("kind"))
            .and_then(|value| value.as_str()),
        Some("data")
    );
    assert_eq!(
        resolved_dataset
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("dataset_id"))
            .and_then(|value| value.as_str()),
        Some("sales_metrics")
    );
}

#[test]
fn resolve_value_resolves_namespaced_world_metric_against_flat_ledger_key() {
    use mei_lang_kernel::WorldMetricLedgerEntry;

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
            local_nav: serde_json::json!({}),
            params: serde_json::json!({}),
            capabilities: Value::Null,
            bindings: serde_json::json!({}),
            examples: serde_json::json!([]),
            access_export: true,
            t2_pages: Vec::new(),
        },
        themes: vec![],
        shared: json!({}),
        world: None,
        flow: None,
        frame: None,
        panels: vec![],
    };
    let compiled = CompiledApp {
        app_id: "data-demo".to_string(),
        title: String::new(),
        app_root: String::new(),
        active_scene: Some("home".to_string()),
        stage_registry: Default::default(),
        stage_programs: Default::default(),
        scene_slot_modules: Default::default(),
        content_capabilities: Default::default(),
        narration_catalogs: Default::default(),
        active_target_file: "src/scenes/home.mei".to_string(),
        scene_routes: Vec::new(),
        file_tree: Vec::new(),
        scene_contract: None,
        scene_local_nav_by_target: Default::default(),
        scene_bindings_by_id: Default::default(),
        scene_examples_by_id: Default::default(),
        scene_projection_assembly_by_id: Default::default(),
        resources: Vec::new(),
        world_metrics: BTreeMap::from([(
            "supervision_items_count".to_string(),
            WorldMetricLedgerEntry {
                id: "supervision_items_count".to_string(),
                owner_resource_id: "__world_metrics__".to_string(),
                order: 1,
                metric: MetricContract {
                    id: "supervision_items_count".to_string(),
                    label: Some("监督事项".to_string()),
                    unit: Some("项".to_string()),
                    value_format: None,
                    purpose: None,
                    shape: MetricShape::Scalar,
                    schema: Vec::new(),
                    dataset: None,
                    transforms: Vec::new(),
                    value: json!({"value": 21}),
                },
            },
        )]),
        world_semantic_by_file: Default::default(),
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
        build_experience_index: Default::default(),
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    let resources = build_runtime_resource_map(&compiled);
    let resource_index = build_runtime_resource_index(&compiled);
    let scene_anchor =
        RuntimeSceneAnchor::for_preview(&compiled, Some("src/scenes/home.mei"), Some("home"));
    let metric_ref = json!({
        "__ref": "metric",
        "id": "scenes/05-监督预警.mei::supervision_items_count"
    });
    let resolved = resolve_value(
        &metric_ref,
        &json!({}),
        &scene_contract,
        &resources,
        &scene_anchor,
        &resource_index,
        &compiled,
        true,
        None,
    );
    assert_eq!(
        resolved.get("id").and_then(|value| value.as_str()),
        Some("supervision_items_count")
    );
    assert!(resolved.get("__mei_runtime_ref").is_some());
}
