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
fn surface_layout_style_emits_grid_template_areas() {
    let layout = grid_layout();
    let style = surface_layout_style(Some(&layout));
    assert!(style.contains("grid-template-areas:'doc table';"));
}

#[test]
fn panel_style_requires_named_grid_areas() {
    let mut layout = grid_layout();
    layout.areas = None;
    assert_eq!(panel_style(Some("doc"), Some(&layout), &json!({})), "");
}

#[test]
fn panel_card_layout_style_applies_grid_columns() {
    let layout = grid_layout();
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("display:grid;"));
    assert!(style.contains("grid-template-columns:1fr 2fr;"));
}

#[test]
fn panel_card_layout_style_emits_grid_align_items() {
    let mut layout = grid_layout();
    layout.align = Some("stretch".to_string());
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("align-items:stretch;"));
}

#[test]
fn panel_card_layout_style_emits_grid_justify_content() {
    let mut layout = grid_layout();
    layout.justify = Some("center".to_string());
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("justify-content:center;"));
}

