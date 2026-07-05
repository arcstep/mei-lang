use std::collections::BTreeMap;

use super::helpers::preview_metric_with_runtime_def;
use super::nodes::component_html;
use super::resolve::{attach_host_meta, resolve_value, HostMetaOptions, RuntimeSceneAnchor};
use super::style::{
    block_style, container_visual_style, container_visual_style_without_background,
    frame_backdrop_css_vars, frame_stage_content_bounds, frame_viewport_letterbox_style,
    has_frame_backdrop, metric_slot_vertical_host_class, normalize_background_image,
    panel_card_layout_style, panel_heading_config, panel_scale_factor, panel_scaled_outer_style,
    panel_show_heading, panel_slot_typography_style, panel_style, surface_layout_style,
};
use super::theme::{
    resolve_panel_card_props, resolve_panel_head_props, resolve_panel_props, resolve_theme,
    ThemeResolved,
};
use super::viewport::{
    effective_canvas_width, effective_viewport_overflow, effective_viewport_safe_inset,
    frame_stage_content_bounds_for_viewport, frame_stage_style, frame_viewport_config,
    frame_viewport_style_for_route, viewport_overflow_is_debug,
};
use crate::ui::route::UiRouteMode;
use mei_lang_kernel::PanelDecl;
use mei_lang_kernel::{
    build_runtime_resource_index, build_runtime_resource_map, ColumnSchema, CompiledApp,
    CompiledSceneRoute, DatasetView, LayoutDecl, LoadedResource, MetricContract, MetricShape,
    SceneContract, SceneDecl, SourceDecl, ThemeDecl,
};
use serde_json::{json, Value};

fn grid_layout() -> LayoutDecl {
    LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string(), "2fr".to_string()]),
        rows: None,
        areas: Some(vec![vec!["doc".to_string(), "table".to_string()]]),
        gap: Some("16px".to_string()),
        padding: Some("20px".to_string()),
        align: None,
        justify: None,
    }
}

#[test]
fn resolve_metric_ref_allows_from_dataset_lineage_for_scene_direct_world_metrics() {
    use mei_lang_kernel::{MetricContract, MetricShape, SceneDecl};

    let scene_contract = SceneContract {
        scene: SceneDecl {
            kind: "scene".to_string(),
            id: "cockpit_embedded_carousel".to_string(),
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
        },
        themes: vec![],
        shared: json!({}),
        world: None,
        flow: None,
        frame: None,
        panels: vec![],
    };

    let dataframe_metric = MetricContract {
        id: "alerts_cockpit_table".to_string(),
        label: Some("嵌入轮播表".to_string()),
        unit: None,
        value_format: None,
        purpose: None,
        shape: MetricShape::Dataframe,
        schema: Vec::new(),
        dataset: None,
        transforms: Vec::new(),
        value: json!([
            {"level": "蓝", "org": "城南街道", "model": "扬尘预警", "alert_time": "2025-03-01"}
        ]),
    };

    let compiled = CompiledApp {
        app_id: "preview-world-metric".to_string(),
        active_scene: Some("cockpit_embedded_carousel".to_string()),
        active_target_file: "03-cockpit-embedded-carousel.mei".to_string(),
        resources: Vec::new(),
        world_metrics: BTreeMap::from([(
            "alerts_cockpit_table".to_string(),
            mei_lang_kernel::WorldMetricLedgerEntry {
                id: "alerts_cockpit_table".to_string(),
                owner_resource_id: "__world_metrics__".to_string(),
                order: 1,
                metric: dataframe_metric,
            },
        )]),
        world_semantic_by_file: BTreeMap::new(),
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview-world-metric".to_string(),
        file_tree: Vec::new(),
        scene_contract: None,
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
    };
    let resource_index = build_runtime_resource_index(&compiled);
    let scene_anchor = super::resolve::RuntimeSceneAnchor {
        scene_id: "cockpit_embedded_carousel".to_string(),
        scene_path: Some("03-cockpit-embedded-carousel.mei".to_string()),
    };

    let metric_ref = json!({
        "__ref": "metric",
        "id": "alerts_cockpit_table",
        "from_dataset": "alerts_raw"
    });
    let resolved = resolve_value(
        &metric_ref,
        &json!({}),
        &scene_contract,
        &BTreeMap::new(),
        &scene_anchor,
        &resource_index,
        &compiled,
        false,
        None,
    );
    assert_ne!(resolved, Value::Null);
    assert_eq!(
        resolved
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("dataset_id"))
            .and_then(|value| value.as_str()),
        Some("__world_metrics__")
    );
    assert_eq!(
        resolved
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("metric_id"))
            .and_then(|value| value.as_str()),
        Some("alerts_cockpit_table")
    );
}

#[test]
fn resolve_value_builds_analysis_contract_from_explain_list() {
    let resolved = preview_metric_with_runtime_def(json!({
        "explain": [
            {"__kind": "explain_item", "id": "note", "kind": "note", "note": "按销售单去重统计。", "content": "按销售单去重统计。", "format": "text"},
            {"__kind": "explain_item", "id": "definition", "kind": "definition", "label": "口径", "basis_refs": ["sales.xlsx", "销售单ID"]},
            {"__kind": "explain_item", "id": "numerator_denominator", "kind": "numerator_denominator", "label": "分子分母", "numerator": "有效销售额", "denominator": "销售总额", "formula": "有效销售额 / 销售总额"},
            {"__kind": "explain_item", "id": "detail", "kind": "detail", "label": "销售明细", "fields": ["销售单ID", "客户", "金额"], "source": {"__ref": "metric", "id": "sales_total_table"}}
        ]
    }));
    let contract = resolved
        .get("__mei_runtime_ref")
        .and_then(|value| value.get("analysis_contract"))
        .and_then(Value::as_object)
        .expect("analysis contract from explain list");
    assert_eq!(
        contract
            .get("tabs")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(3)
    );
    assert_eq!(
        contract
            .get("tab_metrics")
            .and_then(|value| value.get("detail"))
            .and_then(|value| value.get("metric_id"))
            .and_then(Value::as_str),
        Some("sales_total_table")
    );
    assert_eq!(
        contract
            .get("blocks")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(4)
    );
    assert_eq!(
        contract.get("note").and_then(Value::as_str),
        Some("按销售单去重统计。")
    );
}

#[test]
fn resolve_value_builds_analysis_contract_nodes_for_explain_scope_metrics() {
    let resolved = preview_metric_with_runtime_def(json!({
        "explain": [
            {
                "__kind": "data_product",
                "key": "sales_total_table",
                "id": "sales_total_table",
                "shape": "dataframe",
                "label": "销售明细表",
                "analysis_local_id": "sales_total_table",
                "analysis_scoped_id": "sales_total::sales_total_table",
                "analysis_parent_metric_id": "sales_total",
                "value": [{"id": "A", "value": 100}]
            },
            {
                "__kind": "explain_item",
                "id": "detail",
                "kind": "detail",
                "label": "销售明细",
                "fields": ["销售单ID", "客户", "金额"],
                "source": {"__ref": "metric", "id": "sales_total::sales_total_table"}
            }
        ]
    }));
    let contract = resolved
        .get("__mei_runtime_ref")
        .and_then(|value| value.get("analysis_contract"))
        .and_then(Value::as_object)
        .expect("analysis contract from explain scope metric");
    assert_eq!(
        contract.get("table_metric_id").and_then(Value::as_str),
        None
    );
    assert_eq!(
        contract
            .get("tab_metrics")
            .and_then(Value::as_object)
            .and_then(|items| items.get("detail"))
            .and_then(Value::as_object)
            .and_then(|value| value.get("metric_id"))
            .and_then(Value::as_str),
        Some("sales_total::sales_total_table")
    );
    assert_eq!(
        contract
            .get("nodes")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|value| value.get("metric_id"))
            .and_then(Value::as_str),
        Some("sales_total::sales_total_table")
    );
}

