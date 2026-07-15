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
fn resolve_panel_props_prefers_bare_theme_when_chrome_is_bare() {
    let theme = ThemeResolved {
        id: "cockpit".to_string(),
        frame: json!({}),
        panel: json!({
            "padding": "12px",
            "border": "1px solid #334155",
        }),
        panel_bare: json!({
            "padding": "0",
            "border": "none",
            "background": "transparent",
        }),
        panel_head: json!({}),
        panel_body: json!({}),
        heading: json!({}),
        shared: json!({}),
        components: json!({}),
        css_vars: Vec::new(),
    };
    let resolved = resolve_panel_props(
        &theme,
        &json!({
            "chrome": "bare",
            "padding": "2px",
        }),
    );
    assert_eq!(resolved.get("padding").and_then(Value::as_str), Some("2px"));
    assert_eq!(resolved.get("border").and_then(Value::as_str), Some("none"));
    assert_eq!(
        resolved.get("background").and_then(Value::as_str),
        Some("transparent")
    );
}

#[test]
fn default_viewport_page_flow_is_top_left_and_fluid() {
    let vp = super::viewport::default_viewport_page_flow();
    assert!(vp.fluid_height);
    assert_eq!(vp.align_x, "start");
    assert_eq!(vp.align_y, "start");
    assert_eq!(vp.design_width, 1280.0);
    assert_eq!(vp.edit_safe_left, 0.0);
    assert_eq!(vp.edit_safe_right, 0.0);
}

#[test]
fn default_viewport_stage_lock_is_centered_and_fixed_height() {
    let vp = super::viewport::default_viewport_stage_lock();
    assert!(!vp.fluid_height);
    assert_eq!(vp.align_x, "center");
    assert_eq!(vp.align_y, "center");
    assert_eq!(vp.design_width, 1920.0);
    assert_eq!(vp.design_height, 1080.0);
    assert_eq!(vp.aspect_ratio.as_deref(), Some("16:9"));
}

#[test]
fn resolve_frame_viewport_uses_profile_default_without_explicit_props() {
    let props = serde_json::json!({});
    assert!(!super::viewport::frame_viewport_is_explicit(&props));
    let vp = super::viewport::resolve_frame_viewport(&props, Some("page")).expect("page default");
    assert!(vp.fluid_height);
    let cockpit =
        super::viewport::resolve_frame_viewport(&props, Some("cockpit")).expect("cockpit default");
    assert!(!cockpit.fluid_height);
}

#[test]
fn frame_viewport_is_explicit_when_props_declares_viewport() {
    let props = serde_json::json!({
        "viewport": {
            "design_width": 1280,
            "design_height": 720,
        }
    });
    assert!(super::viewport::frame_viewport_is_explicit(&props));
}
