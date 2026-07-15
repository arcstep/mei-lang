use std::collections::BTreeMap;

use super::resolve::{resolve_value, RuntimeSceneAnchor};
use super::{build_preview_runtime_context, nodes, resolve, style, theme, viewport};
use mei_lang_kernel::{
    build_runtime_analysis_contracts, build_runtime_analysis_graph, build_runtime_resource_index,
    build_runtime_resource_map, CompiledApp, DatasetView, LoadedResource, MetricContract,
    MetricShape, SceneContract, SceneDecl, SourceDecl, WorldMetricLedgerEntry,
};
use serde_json::{json, Value};

pub(super) fn preview_metric_with_runtime_def(runtime_def: Value) -> Value {
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
    let table_metric = MetricContract {
        id: "sales_total_table".to_string(),
        label: Some("销售明细".to_string()),
        unit: None,
        value_format: None,
        purpose: None,
        shape: MetricShape::Dataframe,
        schema: Vec::new(),
        dataset: None,
        transforms: Vec::new(),
        value: json!([{"id": "A", "value": 100}]),
    };
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
            metrics: BTreeMap::from([
                ("sales_total".to_string(), scalar_metric.clone()),
                ("sales_total_table".to_string(), table_metric.clone()),
            ]),
            runtime_metric_defs,
            runtime_analysis_graph,
            runtime_analysis_contracts,
        }),
    };
    let compiled = CompiledApp {
        app_id: "preview-explain".to_string(),
        active_scene: Some("home".to_string()),
        stage_registry: Default::default(),
        stage_programs: Default::default(),
        scene_slot_modules: Default::default(),
        content_capabilities: Default::default(),
        narration_catalogs: Default::default(),
        active_target_file: "scenes/home.mei".to_string(),
        resources: vec![resource.clone()],
        world_metrics: BTreeMap::from([
            (
                "sales_total".to_string(),
                WorldMetricLedgerEntry {
                    id: "sales_total".to_string(),
                    owner_resource_id: "sales_metrics".to_string(),
                    order: 1,
                    metric: scalar_metric,
                },
            ),
            (
                "sales_total_table".to_string(),
                WorldMetricLedgerEntry {
                    id: "sales_total_table".to_string(),
                    owner_resource_id: "sales_metrics".to_string(),
                    order: 2,
                    metric: table_metric,
                },
            ),
        ]),
        world_semantic_by_file: BTreeMap::new(),
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview-explain".to_string(),
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
    let scene_anchor = RuntimeSceneAnchor {
        scene_id: "home".to_string(),
        scene_path: Some("scenes/home.mei".to_string()),
    };
    resolve_value(
        &json!({"__ref":"metric","id":"sales_total","from_dataset":"sales_metrics"}),
        &json!({}),
        &scene_contract,
        &build_runtime_resource_map(&compiled),
        &scene_anchor,
        &resource_index,
        &compiled,
        false,
        None,
    )
}
