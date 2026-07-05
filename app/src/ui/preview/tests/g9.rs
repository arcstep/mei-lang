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
fn frame_stage_style_debug_uses_full_canvas_without_css_scale() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1920,
            "design_height": 1080,
            "aspect_ratio": "16:9",
        }
    }))
    .expect("viewport config");
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
    let style = frame_stage_style(None, &json!({}), &vp, &theme, "debug");
    assert!(style.contains("width:1920px;"));
    assert!(style.contains("min-height:1080px;"));
    assert!(style.contains("height:auto;"));
    assert!(style.contains("transform:none;"));
    let debug_style = frame_viewport_style_for_route(&vp, "debug", UiRouteMode::Layout);
    assert!(debug_style.contains("overflow-x:auto;"));
    assert!(viewport_overflow_is_debug("debug"));
    assert!(viewport_overflow_is_debug("scroll"));
}

#[test]
fn frame_viewport_style_page_flow_uses_block_layout() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1280,
            "design_height": 720,
            "fluid_height": true,
            "edit_safe_inset": { "top": 32, "right": 24, "bottom": 16, "left": 24 }
        }
    }))
    .expect("viewport config");
    let style = frame_viewport_style_for_route(&vp, "debug", UiRouteMode::Layout);
    assert!(style.contains("display:block;"));
    assert!(!style.contains("display:grid;"));
    assert!(style.contains("padding:32px 0px 16px 0px;"));
    assert!(!style.contains("padding:32px 24px"));
}

#[test]
fn frame_viewport_style_applies_alignment_and_padding() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1920,
            "design_height": 1080,
            "align_x": "left",
            "align_y": "top",
            "safe_padding": 18,
        }
    }))
    .expect("viewport config");
    let debug_style = frame_viewport_style_for_route(&vp, "debug", UiRouteMode::Layout);
    assert!(debug_style.contains("justify-items:start;"));
    assert!(debug_style.contains("align-items:start;"));
    assert!(debug_style.contains("padding:18px 18px 18px 18px;"));

    let access_style = frame_viewport_style_for_route(&vp, "clip", UiRouteMode::App);
    assert!(access_style.contains("display:flex;"));
    assert!(access_style.contains("align-items:center;"));
    assert!(access_style.contains("justify-content:center;"));
    assert!(access_style.contains("padding:18px 18px 18px 18px;"));
}

#[test]
fn effective_viewport_safe_inset_splits_access_and_edit() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1920,
            "design_height": 1080,
            "safe_inset": { "top": 0, "right": 0, "bottom": 0, "left": 0 },
            "edit_safe_inset": { "top": 32, "right": 16, "bottom": 12, "left": 8 },
        }
    }))
    .expect("viewport config");
    assert_eq!(
        effective_viewport_safe_inset(&vp, UiRouteMode::App),
        (0.0, 0.0, 0.0, 0.0)
    );
    assert_eq!(
        effective_viewport_safe_inset(&vp, UiRouteMode::Layout),
        (32.0, 16.0, 12.0, 8.0)
    );
}

#[test]
fn frame_stage_content_bounds_treats_max_width_as_cap() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1920,
            "design_height": 720,
        }
    }))
    .expect("viewport config");
    let props = json!({
        "width": "100%",
        "max_width": "520px",
    });
    let bounds = frame_stage_content_bounds(&props, vp.design_width, vp.design_height);
    assert_eq!(bounds.max_width, Some(520.0));
    assert_eq!(bounds.height, 720.0);
    assert_eq!(bounds.fallback_width, 1920.0);
    let viewport_bounds = frame_stage_content_bounds_for_viewport(&props, &vp);
    assert_eq!(viewport_bounds.max_width, Some(520.0));
}

