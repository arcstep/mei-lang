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
fn frame_stage_style_uses_max_width_cap_not_fixed_canvas_width() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1920,
            "design_height": 720,
        }
    }))
    .expect("viewport config");
    let props = json!({
        "max_width": "520px",
        "width": "100%",
    });
    let theme = ThemeResolved {
        id: "cockpit".to_string(),
        frame: json!({}),
        panel: json!({}),
        panel_bare: json!({}),
        panel_head: json!({}),
        panel_body: json!({}),
        heading: json!({}),
        shared: json!({}),
        components: json!({}),
        css_vars: Vec::new(),
    };
    let style = frame_stage_style(None, &props, &vp, &theme, "clip");
    assert!(style.contains("--mei-frame-content-max-width:520px;"));
    assert!(style.contains("width:100%;"));
    assert!(style.contains("height:auto;"));
    assert!(style.contains("transform:none;"));
    assert!(!style.contains("width:1920px;"));
}

#[test]
fn resolve_theme_merges_shared_context_and_resolves_component_defaults() {
    let scene_contract = SceneContract {
        scene: SceneDecl {
            kind: "scene".to_string(),
            id: "home".to_string(),
            world: None,
            flow: None,
            frame: None,
            profile: Some("cockpit".to_string()),
            theme: Some("cockpit".to_string()),
            summary: None,
            goal: None,
            state: json!({}),
            shared: json!({
                "layout": {"rail_width": "520px"},
                "table": {"preview_chars": 18},
            }),
            local_nav: serde_json::json!({}),
            params: serde_json::json!({}),
            capabilities: Value::Null,
            bindings: serde_json::json!({}),
            examples: serde_json::json!([]),
            access_export: true,
        },
        themes: vec![ThemeDecl {
            kind: "theme".to_string(),
            id: "cockpit".to_string(),
            frame: json!({
                "max_width": {"__ref": "shared", "id": "layout.rail_width"},
            }),
            panel: json!({}),
            panel_bare: json!({}),
            panel_head: json!({}),
            panel_body: json!({}),
            heading: json!({}),
            font: json!({}),
            metric_label: json!({}),
            metric_value: json!({}),
            metric_unit: json!({}),
            metric_desc: json!({}),
            metric_sub_label: json!({}),
            metric_sub_value: json!({}),
            metric_sub_unit: json!({}),
            chart_title: json!({}),
            chart_label: json!({}),
            table_head: json!({}),
            table_body: json!({}),
            filter_panel: json!({}),
            tokens: json!({}),
            shared: json!({
                "layout": {"rail_width": "480px", "header_height": "72px"},
                "table": {"preview_chars": 30},
            }),
            components: json!({
                "dataset_table": {
                    "cell_preview_max_chars": {"__ref": "shared", "id": "table.preview_chars"},
                }
            }),
        }],
        shared: json!({
            "layout": {"rail_width": "520px", "header_height": "72px"},
            "table": {"preview_chars": 18},
        }),
        world: None,
        flow: None,
        frame: None,
        panels: vec![],
    };

    let resolved = resolve_theme(&scene_contract, None);
    assert_eq!(
        resolved
            .shared
            .get("layout")
            .and_then(|value| value.get("rail_width"))
            .and_then(Value::as_str),
        Some("520px")
    );
    assert_eq!(
        resolved.frame.get("max_width").and_then(Value::as_str),
        Some("520px")
    );
    assert_eq!(
        resolved
            .components
            .get("dataset_table")
            .and_then(|value| value.get("cell_preview_max_chars"))
            .and_then(Value::as_i64),
        Some(18)
    );
}

#[test]
fn resolve_value_preserves_board_link_scene_locator_in_popup() {
    let contract = SceneContract {
        scene: SceneDecl {
            kind: "scene".to_string(),
            id: "enforcement_elements".to_string(),
            world: None,
            flow: None,
            frame: None,
            profile: Some("page".to_string()),
            theme: None,
            summary: None,
            goal: None,
            state: Value::Null,
            shared: Value::Null,
            local_nav: Value::Null,
            params: serde_json::json!({}),
            capabilities: Value::Null,
            bindings: serde_json::json!({}),
            examples: serde_json::json!([]),
            access_export: true,
        },
        themes: Vec::new(),
        shared: Value::Null,
        world: None,
        flow: None,
        frame: None,
        panels: Vec::new(),
    };
    let compiled = CompiledApp {
        app_id: "t".to_string(),
        title: "t".to_string(),
        app_root: ".".to_string(),
        scene_routes: Vec::new(),
        active_scene: Some("enforcement_elements".to_string()),
        stage_registry: Default::default(),
        stage_programs: Default::default(),
        scene_slot_modules: Default::default(),
        content_capabilities: Default::default(),
        narration_catalogs: Default::default(),
        active_target_file: "scenes/1_执法要素/执法要素.mei".to_string(),
        file_tree: Vec::new(),
        scene_contract: Some(contract.clone()),
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
    let index = build_runtime_resource_index(&compiled);
    let props = json!({
        "popup": {
            "__kind": "board_link",
            "mode": "board_link",
            "scene": {
                "__ref": "scene",
                "scene_file": "templates/cockpit/drilldown/metric-explain-board.mei",
                "scene_id": "metric_explain_board",
                "entry": "detail"
            },
            "projection": "overlay"
        }
    });
    let resolved = resolve_value(
        &props,
        &json!({}),
        &contract,
        &BTreeMap::new(),
        &RuntimeSceneAnchor::from_compiled(&compiled),
        &index,
        &compiled,
        false,
        None,
    );
    let popup = resolved.get("popup").expect("popup");
    let scene = popup.get("scene").expect("scene locator");
    assert_eq!(
        scene.get("scene_id").and_then(Value::as_str),
        Some("metric_explain_board")
    );
    assert_eq!(
        scene.get("scene_file").and_then(Value::as_str),
        Some("templates/cockpit/drilldown/metric-explain-board.mei")
    );
    assert_eq!(scene.get("entry").and_then(Value::as_str), Some("detail"));
}

#[test]
fn attach_host_meta_only_includes_scene_drilldown_context_when_requested() {
    let compiled = CompiledApp {
        app_id: "preview-shared".to_string(),
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
        title: "preview-shared".to_string(),
        file_tree: Vec::new(),
        scene_contract: None,
        scene_local_nav_by_target: BTreeMap::from([(
            "templates/cockpit/drilldown/metric-explain-board.mei".to_string(),
            json!({
                "kind": "metric_explain_board",
                "default_entry": "definition",
                "order_by_kind": ["definition", "composition", "trend", "detail"],
            }),
        )]),
        scene_bindings_by_id: BTreeMap::from([(
            "inspection_total_popup".to_string(),
            json!({
                "detail": {"__ref": "metric", "id": "sales_total", "from_dataset": "sales"},
            }),
        )]),
        scene_examples_by_id: BTreeMap::from([(
            "inspection_total_popup".to_string(),
            json!([
                {
                    "id": "default",
                    "bindings": {
                        "detail": {"__ref": "metric", "id": "sales_total", "from_dataset": "sales"},
                    },
                },
            ]),
        )]),
        scene_projection_assembly_by_id: BTreeMap::from([(
            "inspection_total_popup".to_string(),
            json!({
                "scene_id": "inspection_total_popup",
                "target_file": "templates/cockpit/drilldown/metric-explain-board.mei",
                "local_nav": {
                    "kind": "metric_explain_board",
                    "default_entry": "definition",
                    "order_by_kind": ["definition", "composition", "trend", "detail"]
                },
                "bindings": {
                    "detail": {"__ref": "metric", "id": "sales_total", "from_dataset": "sales"}
                },
                "examples": [
                    {
                        "id": "default",
                        "bindings": {
                            "detail": {"__ref": "metric", "id": "sales_total", "from_dataset": "sales"}
                        }
                    }
                ]
            }),
        )]),
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
        build_experience_index: Default::default(),
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    let props = attach_host_meta(
        json!({"value": 1}),
        &compiled,
        "apps/preview-shared",
        &json!({"dataset_table": {"cell_preview_max_chars": 18}}),
        Some("scenes/home.mei"),
        HostMetaOptions {
            host_ssr_slim_payload: true,
            ..Default::default()
        },
    );
    assert!(props
        .get("_mei")
        .and_then(|value| value.get("runtime_capabilities"))
        .is_none());
    let legacy_props = attach_host_meta(
        json!({"value": 1}),
        &compiled,
        "apps/preview-shared",
        &json!({"dataset_table": {"cell_preview_max_chars": 18}}),
        Some("scenes/home.mei"),
        HostMetaOptions::default(),
    );
    assert_eq!(
        legacy_props
            .get("_mei")
            .and_then(|value| value.get("runtime_capabilities"))
            .and_then(|value| value.get("rows_query"))
            .and_then(|value| value.get("enabled"))
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        legacy_props
            .get("_mei")
            .and_then(|value| value.get("runtime_capabilities"))
            .and_then(|value| value.get("rows_query"))
            .and_then(|value| value.get("api"))
            .and_then(Value::as_str),
        Some("/api/datasets/query/apps/preview-shared")
    );
    assert_eq!(
        legacy_props
            .get("_mei")
            .and_then(|value| value.get("runtime_capabilities"))
            .and_then(|value| value.get("metric_query"))
            .and_then(|value| value.get("api"))
            .and_then(Value::as_str),
        Some("/api/datasets/metrics/apps/preview-shared")
    );
    assert!(legacy_props
        .get("_mei")
        .and_then(|value| value.get("dataset_query_api"))
        .is_none());
    assert!(legacy_props
        .get("_mei")
        .and_then(|value| value.get("metric_query_api"))
        .is_none());
    assert_eq!(
        legacy_props
            .get("_mei")
            .and_then(|value| value.get("components"))
            .and_then(|value| value.get("dataset_table"))
            .and_then(|value| value.get("cell_preview_max_chars"))
            .and_then(Value::as_i64),
        Some(18)
    );
    assert!(legacy_props
        .get("_mei")
        .and_then(|value| value.get("shared"))
        .is_none());
    assert!(legacy_props
        .get("_mei")
        .and_then(|value| value.get("scene_local_nav_by_target"))
        .is_none());
    assert!(legacy_props
        .get("_mei")
        .and_then(|value| value.get("scene_bindings_by_id"))
        .is_none());
    let drilldown_props = attach_host_meta(
        json!({"value": 1}),
        &compiled,
        "apps/preview-shared",
        &json!({"dataset_table": {"cell_preview_max_chars": 18}}),
        Some("scenes/home.mei"),
        HostMetaOptions {
            include_scene_drilldown_context: true,
            host_ssr_slim_payload: false,
            data_mode: None,
        },
    );
    assert!(drilldown_props
        .get("_mei")
        .and_then(|value| value.get("scene_local_nav_by_target"))
        .is_some());
    assert!(drilldown_props
        .get("_mei")
        .and_then(|value| value.get("scene_bindings_by_id"))
        .is_some());
    assert!(drilldown_props
        .get("_mei")
        .and_then(|value| value.get("scene_projection_assembly_by_id"))
        .is_some());
}

#[test]
fn resolve_value_supports_shared_refs() {
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
    let compiled = CompiledApp {
        app_id: "preview-shared-ref".to_string(),
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
        title: "preview-shared-ref".to_string(),
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
    let scene_anchor = super::resolve::RuntimeSceneAnchor {
        scene_id: "home".to_string(),
        scene_path: Some("scenes/home.mei".to_string()),
    };
    let resolved = resolve_value(
        &json!({
            "width": {"__ref": "shared", "id": "layout.rail_width"},
            "height": {"__ref": "shared", "id": "layout.card_height", "default": 74},
        }),
        &json!({"layout": {"rail_width": "520px"}}),
        &scene_contract,
        &BTreeMap::new(),
        &scene_anchor,
        &build_runtime_resource_index(&compiled),
        &compiled,
        false,
        None,
    );
    assert_eq!(resolved.get("width").and_then(Value::as_str), Some("520px"));
    assert_eq!(resolved.get("height").and_then(Value::as_i64), Some(74));
}
