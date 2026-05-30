use std::path::Path;

use super::{validate_imported_catalog_world_refs, validate_scene_ui_data_bindings};
use crate::model::{
    BlockDecl, LoadedResource, PanelDecl, SceneContract, SceneDecl, Severity, UiNodeDecl,
};
use serde_json::Value;

fn sample_scene() -> SceneDecl {
    serde_json::from_value(serde_json::json!({
        "kind": "scene",
        "id": "s",
        "state": {},
    }))
    .expect("scene")
}

#[test]
fn flags_analysis_expr_rows_in_block_props() {
    let contract = SceneContract {
        scene: sample_scene(),
        themes: vec![],
        shared: serde_json::json!({}),
        world: None,
        flow: None,
        frame: None,
        panels: vec![PanelDecl {
            kind: "panel".to_string(),
            id: "p1".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiNodeDecl::Block(BlockDecl {
                kind: "block".to_string(),
                use_key: "dataset.table".to_string(),
                id: Some("t1".to_string()),
                title: None,
                area: None,
                props: serde_json::json!({
                    "data": {"__kind": "analysis_expr", "type": "rows", "dataset": "x"}
                }),
                base: None,
                layout: None,
                blocks: vec![],
                component: None,
                placement: None,
                interactions: vec![],
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            props: Value::Object(serde_json::Map::new()),
            head_props: Value::Object(serde_json::Map::new()),
            body_props: Value::Object(serde_json::Map::new()),
            base: None,
            import_scope: None,
        }],
    };
    let mut diagnostics = Vec::new();
    validate_scene_ui_data_bindings(
        &contract,
        &[],
        Path::new("."),
        "entry.mei",
        &mut diagnostics,
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "forbidden_direct_ui_data_binding");
}

#[test]
fn flags_imported_catalog_resource_ref_as_warning() {
    let contract = SceneContract {
        scene: sample_scene(),
        themes: vec![],
        shared: serde_json::json!({}),
        world: None,
        flow: None,
        frame: None,
        panels: vec![PanelDecl {
            kind: "panel".to_string(),
            id: "p1".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiNodeDecl::Block(BlockDecl {
                kind: "block".to_string(),
                use_key: "dataset.table".to_string(),
                id: None,
                title: None,
                area: None,
                props: serde_json::json!({
                    "data": {"__ref": "resource", "id": "catalog_only"}
                }),
                base: None,
                layout: None,
                blocks: vec![],
                component: None,
                placement: None,
                interactions: vec![],
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            props: Value::Object(serde_json::Map::new()),
            head_props: Value::Object(serde_json::Map::new()),
            body_props: Value::Object(serde_json::Map::new()),
            base: None,
            import_scope: None,
        }],
    };
    let authorized = vec![LoadedResource {
        id: "local_only".to_string(),
        kind: "dataset".to_string(),
        title: None,
        document: None,
        dataset: None,
    }];
    let merged = vec![
        authorized[0].clone(),
        LoadedResource {
            id: "catalog_only".to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: None,
        },
    ];
    let mut diagnostics = Vec::new();
    validate_imported_catalog_world_refs(
        &contract,
        &authorized,
        &merged,
        "entry.mei",
        &mut diagnostics,
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "imported_resource_not_authorized");
    assert_eq!(diagnostics[0].severity, Severity::Warning);
}

#[test]
fn flags_misused_world_ref_in_props() {
    let contract = SceneContract {
        scene: sample_scene(),
        themes: vec![],
        shared: serde_json::json!({}),
        world: None,
        flow: None,
        frame: None,
        panels: vec![PanelDecl {
            kind: "panel".to_string(),
            id: "p1".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiNodeDecl::Block(BlockDecl {
                kind: "block".to_string(),
                use_key: "dataset.table".to_string(),
                id: None,
                title: None,
                area: None,
                props: serde_json::json!({
                    "data": {"__ref": "world", "id": "my_dataset"}
                }),
                base: None,
                layout: None,
                blocks: vec![],
                component: None,
                placement: None,
                interactions: vec![],
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            props: Value::Object(serde_json::Map::new()),
            head_props: Value::Object(serde_json::Map::new()),
            body_props: Value::Object(serde_json::Map::new()),
            base: None,
            import_scope: None,
        }],
    };
    let mut diagnostics = Vec::new();
    let resources = vec![LoadedResource {
        id: "my_dataset".to_string(),
        kind: "dataset".to_string(),
        title: None,
        document: None,
        dataset: None,
    }];
    validate_scene_ui_data_bindings(
        &contract,
        &resources,
        Path::new("."),
        "entry.mei",
        &mut diagnostics,
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "misused_world_ref_in_props");
}

#[test]
fn allows_resource_ref_in_props_when_authorized() {
    let contract = SceneContract {
        scene: sample_scene(),
        themes: vec![],
        shared: serde_json::json!({}),
        world: None,
        flow: None,
        frame: None,
        panels: vec![PanelDecl {
            kind: "panel".to_string(),
            id: "p1".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiNodeDecl::Block(BlockDecl {
                kind: "block".to_string(),
                use_key: "dataset.table".to_string(),
                id: None,
                title: None,
                area: None,
                props: serde_json::json!({
                    "data": {"__ref": "resource", "id": "my_dataset"}
                }),
                base: None,
                layout: None,
                blocks: vec![],
                component: None,
                placement: None,
                interactions: vec![],
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            props: Value::Object(serde_json::Map::new()),
            head_props: Value::Object(serde_json::Map::new()),
            body_props: Value::Object(serde_json::Map::new()),
            base: None,
            import_scope: None,
        }],
    };
    let mut diagnostics = Vec::new();
    let resources = vec![LoadedResource {
        id: "my_dataset".to_string(),
        kind: "dataset".to_string(),
        title: None,
        document: None,
        dataset: None,
    }];
    validate_scene_ui_data_bindings(
        &contract,
        &resources,
        Path::new("."),
        "entry.mei",
        &mut diagnostics,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn allows_metric_ref_in_props_when_metric_id_exists_in_world_ledger() {
    let contract = SceneContract {
        scene: sample_scene(),
        themes: vec![],
        shared: serde_json::json!({}),
        world: None,
        flow: None,
        frame: None,
        panels: vec![PanelDecl {
            kind: "panel".to_string(),
            id: "p1".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiNodeDecl::Block(BlockDecl {
                kind: "block".to_string(),
                use_key: "chart.kpi".to_string(),
                id: None,
                title: None,
                area: None,
                props: serde_json::json!({
                    "metric": {"__ref": "metric", "id": "warnings_total"}
                }),
                base: None,
                layout: None,
                blocks: vec![],
                component: None,
                placement: None,
                interactions: vec![],
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            props: Value::Object(serde_json::Map::new()),
            head_props: Value::Object(serde_json::Map::new()),
            body_props: Value::Object(serde_json::Map::new()),
            base: None,
            import_scope: None,
        }],
    };
    let resources = vec![LoadedResource {
        id: "warning_view".to_string(),
        kind: "dataset".to_string(),
        title: None,
        document: None,
        dataset: Some(crate::model::DatasetView {
            id: "warning_view".to_string(),
            title: None,
            purpose: None,
            schema: vec![],
            stage_schema: vec![],
            columns: vec![],
            rows: vec![],
            source: crate::model::SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:warning_view".to_string(),
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
            sources: vec![],
            metrics: std::collections::BTreeMap::from([(
                "warnings_total".to_string(),
                crate::model::MetricContract {
                    id: "warnings_total".to_string(),
                    label: Some("预警总量".to_string()),
                    unit: Some("条".to_string()),
                    purpose: None,
                    shape: crate::model::MetricShape::Scalar,
                    schema: vec![],
                    dataset: None,
                    transforms: vec![],
                    value: serde_json::json!({"value": 1}),
                },
            )]),
            runtime_metric_defs: std::collections::BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        }),
    }];
    let mut diagnostics = Vec::new();
    validate_scene_ui_data_bindings(
        &contract,
        &resources,
        Path::new("."),
        "entry.mei",
        &mut diagnostics,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
