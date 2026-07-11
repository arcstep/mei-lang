use std::collections::BTreeMap;

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
use mei_lang_kernel::UiNodeDecl;
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
fn resolve_metric_ref_prefers_world_metric_ledger_over_first_dataset_match() {
    use mei_lang_kernel::{MetricContract, MetricShape, SceneDecl};

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
        },
        themes: vec![],
        shared: json!({}),
        world: None,
        flow: None,
        frame: None,
        panels: vec![],
    };

    let metric_a = MetricContract {
        id: "same_metric".to_string(),
        label: Some("A".to_string()),
        unit: None,
        value_format: None,
        purpose: None,
        shape: MetricShape::Scalar,
        schema: Vec::new(),
        dataset: None,
        transforms: Vec::new(),
        value: json!({"value": 1}),
    };
    let metric_b = MetricContract {
        id: "same_metric".to_string(),
        label: Some("B".to_string()),
        unit: None,
        value_format: None,
        purpose: None,
        shape: MetricShape::Scalar,
        schema: Vec::new(),
        dataset: None,
        transforms: Vec::new(),
        value: json!({"value": 2}),
    };

    let resource_a = LoadedResource {
        id: "a".to_string(),
        kind: "dataset".to_string(),
        title: None,
        document: None,
        dataset: Some(DatasetView {
            id: "a".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["value".to_string()],
            rows: vec![json!({"value": 1})],
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:a".to_string(),
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
            metrics: BTreeMap::from([(metric_a.id.clone(), metric_a.clone())]),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        }),
    };
    let resource_b = LoadedResource {
        id: "b".to_string(),
        kind: "dataset".to_string(),
        title: None,
        document: None,
        dataset: Some(DatasetView {
            id: "b".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: vec!["value".to_string()],
            rows: vec![json!({"value": 2})],
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:b".to_string(),
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
            metrics: BTreeMap::from([(metric_b.id.clone(), metric_b.clone())]),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        }),
    };
    let resources = BTreeMap::from([
        ("a".to_string(), resource_a.clone()),
        ("b".to_string(), resource_b.clone()),
    ]);
    let compiled = CompiledApp {
        app_id: "preview-ledger".to_string(),
        active_scene: Some("home".to_string()),
        active_target_file: "scenes/home.mei".to_string(),
        resources: vec![resource_a, resource_b],
        world_metrics: BTreeMap::from([(
            "same_metric".to_string(),
            mei_lang_kernel::WorldMetricLedgerEntry {
                id: "same_metric".to_string(),
                owner_resource_id: "b".to_string(),
                order: 2,
                metric: metric_b,
            },
        )]),
        world_semantic_by_file: BTreeMap::new(),
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview-ledger".to_string(),
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

    let resolved = resolve_value(
        &json!({"__ref":"metric","id":"same_metric"}),
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
        resolved
            .get("value")
            .and_then(|value| value.get("value"))
            .and_then(|value| value.as_i64()),
        Some(2)
    );
    assert_eq!(
        resolved
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("dataset_id"))
            .and_then(|value| value.as_str()),
        Some("b")
    );
}
