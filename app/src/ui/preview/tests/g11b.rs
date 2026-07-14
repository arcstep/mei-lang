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
fn runtime_scene_anchor_prefers_matching_route_scene_id() {
    let compiled = CompiledApp {
        app_id: "_stock-catalog".to_string(),
        title: "catalog".to_string(),
        app_root: ".".to_string(),
        scene_routes: vec![CompiledSceneRoute {
            scene_id: "chart.radar".to_string(),
            frame_id: None,
            target_file: "../../stock/components/chart/echarts/previews/chart.radar.mei"
                .to_string(),
            kind: "file_ref".to_string(),
            title: None,
            is_default: false,
            access_export: true,
        }],
        active_scene: Some("home".to_string()),
        stage_registry: Default::default(),
        stage_programs: Default::default(),
        scene_slot_modules: Default::default(),
        content_capabilities: Default::default(),
        narration_catalogs: Default::default(),
        active_target_file: "../../stock/components/chart/echarts/previews/chart.radar.mei"
            .to_string(),
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
    let anchor = RuntimeSceneAnchor::from_compiled(&compiled);
    assert_eq!(anchor.scene_id, "chart.radar");
    assert_eq!(
        anchor.scene_path.as_deref(),
        Some("../../stock/components/chart/echarts/previews/chart.radar.mei")
    );
    let preview_anchor = RuntimeSceneAnchor::for_preview(
        &compiled,
        Some("../../stock/components/chart/echarts/previews/chart.radar.mei"),
        Some("home"),
    );
    assert_eq!(preview_anchor.scene_id, "chart.radar");
}

#[test]
fn build_preview_runtime_context_enables_host_ssr_slim_for_build_mode() {
    use super::build_preview_runtime_context;

    let compiled = CompiledApp {
        app_id: "demo".to_string(),
        active_scene: Some("home".to_string()),
        stage_registry: Default::default(),
        stage_programs: Default::default(),
        scene_slot_modules: Default::default(),
        content_capabilities: Default::default(),
        narration_catalogs: Default::default(),
        active_target_file: "scenes/home.mei".to_string(),
        resources: Vec::new(),
        world_metrics: BTreeMap::new(),
        world_semantic_by_file: BTreeMap::new(),
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview".to_string(),
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
    assert!(
        build_preview_runtime_context(&compiled, UiRouteMode::Layout, None, None, None, None, None)
            .host_ssr_slim_payload
    );
    assert!(
        build_preview_runtime_context(&compiled, UiRouteMode::App, None, None, None, None, None)
            .host_ssr_slim_payload
    );
    assert!(
        !build_preview_runtime_context(
            &compiled,
            UiRouteMode::Config,
            None,
            None,
            None,
            None,
            None
        )
        .host_ssr_slim_payload
    );
    assert!(
        build_preview_runtime_context(
            &compiled,
            UiRouteMode::Layout,
            None,
            Some("chart.donut"),
            Some("../../stock/components/chart/echarts/previews/chart.donut.mei"),
            None,
            None,
        )
        .host_ssr_slim_payload
    );
}

#[test]
fn resolve_value_route_target_alias_matches_canonical_dataset_id() {
    use mei_lang_kernel::{CompiledSceneRoute, MetricContract, MetricShape, SceneDecl};

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
    let mut resources = BTreeMap::new();
    resources.insert(
        "home".to_string(),
        LoadedResource {
            id: "home".to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: Some(DatasetView {
                id: "home".to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: vec!["value".to_string()],
                rows: vec![json!({"value": 1})],
                source: SourceDecl {
                    kind: "derived".to_string(),
                    path: "dataset_view:home".to_string(),
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
                        label: None,
                        unit: None,
                        value_format: None,
                        purpose: None,
                        shape: MetricShape::Scalar,
                        schema: Vec::new(),
                        dataset: None,
                        transforms: Vec::new(),
                        value: json!({"value": 1}),
                    },
                )]),
                runtime_metric_defs: Default::default(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: Default::default(),
            }),
        },
    );
    let compiled = CompiledApp {
        app_id: "preview-alias".to_string(),
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
                owner_resource_id: "home".to_string(),
                order: 1,
                metric: resources
                    .get("home")
                    .and_then(|resource| resource.dataset.as_ref())
                    .and_then(|dataset| dataset.metrics.get("sales_total"))
                    .cloned()
                    .expect("metric"),
            },
        )]),
        world_semantic_by_file: BTreeMap::new(),
        scene_routes: vec![CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "scenes/home.mei".to_string(),
            kind: "file_ref".to_string(),
            title: None,
            is_default: true,
            access_export: true,
        }],
        app_root: ".".to_string(),
        title: "preview-alias".to_string(),
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
    let metric_ref = json!({
        "__ref": "metric",
        "id": "sales_total",
        "from_dataset": "scenes/home.mei"
    });
    let resolved = resolve_value(
        &metric_ref,
        &json!({}),
        &scene_contract,
        &build_runtime_resource_map(&compiled),
        &scene_anchor,
        &resource_index,
        &compiled,
        false,
        None,
    );
    assert_eq!(
        resolved
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("dataset_id"))
            .and_then(|value| value.as_str()),
        Some("home")
    );
}
