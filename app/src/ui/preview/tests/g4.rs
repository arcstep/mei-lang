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
fn block_style_spans_head_row_across_columns() {
    let mut layout = grid_layout();
    layout.columns = Some(vec!["1fr".to_string(); 3]);
    layout.rows = Some(vec!["59px".to_string(), "102px".to_string()]);
    layout.areas = Some(vec![
        vec!["head".to_string(); 3],
        vec!["m0".to_string(), "m1".to_string(), "m2".to_string()],
    ]);
    let style = block_style(Some("head"), Some(&layout));
    assert!(style.contains("grid-area:head;"));
    assert!(style.contains("grid-column:1 / -1;"));
    assert!(style.contains("height:100%;"));
}

#[test]
fn panel_style_merges_grid_area_and_visual_props() {
    let layout = grid_layout();
    let style = panel_style(
        Some("doc"),
        Some(&layout),
        &json!({
            "padding": "0",
            "border": "none",
            "background": {
                "color": "#001122",
            }
        }),
    );
    assert!(style.contains("grid-area:doc;"));
    assert!(style.contains("padding:0;"));
    assert!(style.contains("border:none;"));
    assert!(style.contains("background-color:#001122;"));
}

#[test]
fn container_visual_style_supports_background_image_shorthand() {
    let style = container_visual_style(&json!({
        "background": {
            "image": "/workspace-components/demo.png",
            "size": "cover",
            "repeat": "no-repeat",
        }
    }));
    assert!(style.contains("background-image:url(\"/workspace-components/demo.png\")"));
    assert!(style.contains("background-size:cover;"));
    assert!(style.contains("background-repeat:no-repeat;"));
}

#[test]
fn panel_style_grid_area_applies_background_once() {
    let style = panel_style(
        Some("doc"),
        Some(&grid_layout()),
        &json!({
            "background": {
                "color": "#001122",
            }
        }),
    );
    assert_eq!(style.matches("background-color:#001122;").count(), 1);
}

#[test]
fn panel_scale_factor_parses_numeric_and_percent_values() {
    assert_eq!(panel_scale_factor(&json!({"scale": 0.75})), Some(0.75));
    assert_eq!(panel_scale_factor(&json!({"scale": "82%"})), Some(0.82));
    assert_eq!(panel_scale_factor(&json!({"scale": 1})), None);
    assert_eq!(panel_scale_factor(&json!({})), None);
}

