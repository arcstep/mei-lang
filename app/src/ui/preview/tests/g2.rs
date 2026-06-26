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
fn panel_card_layout_style_normalizes_bare_numeric_gap_to_px() {
    let mut layout = grid_layout();
    layout.gap = Some("5".to_string());
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("gap:5px;"));
}

#[test]
fn block_style_centers_content_for_centered_grid_items() {
    let mut layout = grid_layout();
    layout.justify = Some("center".to_string());
    let style = block_style(Some("doc"), Some(&layout));
    assert!(style.contains("justify-self:center;"));
    assert!(style.contains("width:auto;"));
}

#[test]
fn block_style_desc_slot_spans_full_width_when_grid_is_centered() {
    let mut layout = grid_layout();
    layout.justify = Some("center".to_string());
    let style = block_style(Some("desc"), Some(&layout));
    assert!(style.contains("width:100%;"));
    assert!(!style.contains("width:auto;"));
}

#[test]
fn block_style_label_slot_spans_full_width_when_grid_is_centered() {
    let mut layout = grid_layout();
    layout.justify = Some("center".to_string());
    let style = block_style(Some("label"), Some(&layout));
    assert!(style.contains("width:100%;"));
    assert!(!style.contains("width:auto;"));
}

#[test]
fn metric_slot_vertical_host_class_maps_metric_v_align() {
    assert_eq!(
        metric_slot_vertical_host_class(&json!({"metric_v_align": "end"})),
        "component-card--slot-v-end"
    );
    assert_eq!(
        metric_slot_vertical_host_class(&json!({"metric_v_align": "start"})),
        "component-card--slot-v-start"
    );
    assert_eq!(
        metric_slot_vertical_host_class(&json!({"metric_role": "label"})),
        "component-card--slot-v-center"
    );
}

