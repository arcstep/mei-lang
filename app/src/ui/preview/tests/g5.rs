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
fn panel_scaled_outer_style_scales_fixed_dimensions() {
    let style = panel_scaled_outer_style(
        Some("doc"),
        Some(&grid_layout()),
        &json!({
            "width": "234px",
            "height": "128px",
            "min_height": "128px",
        }),
        0.75,
    );
    assert!(style.contains("grid-area:doc;"));
    assert!(style.contains("justify-self:center;"));
    assert!(style.contains("align-self:center;"));
    assert!(style.contains("width:175.5px;"));
    assert!(style.contains("height:96px;"));
    assert!(style.contains("min-height:96px;"));
}

#[test]
fn normalize_background_image_none_is_not_wrapped_as_url() {
    assert_eq!(normalize_background_image("none"), "none".to_string());
}

#[test]
fn frame_viewport_letterbox_style_sets_letterbox_var_only() {
    let style = frame_viewport_letterbox_style(&json!({
        "letterbox": { "color": "rgb(29, 47, 65)" }
    }));
    assert!(style.contains("--mei-frame-letterbox:rgb(29, 47, 65);"));
    assert!(!style.contains("background:rgb(29, 47, 65);"));
}

#[test]
fn frame_letterbox_defaults_when_frame_has_only_background() {
    let style = frame_viewport_letterbox_style(&json!({
        "background": { "color": "rgb(10, 36, 72)" }
    }));
    assert!(style.contains("--mei-frame-letterbox:#070d14;"));
}

#[test]
fn frame_backdrop_css_vars_exports_layer_tokens_without_inline_background() {
    let props = json!({
        "background": {
            "color": "#182f42",
            "image": "linear-gradient(180deg, #1a3348, #0a1824)",
            "size": "100% 100%",
        }
    });
    let vars = frame_backdrop_css_vars(&props);
    assert!(vars.contains("--mei-frame-bg-color:#182f42;"));
    assert!(vars.contains("--mei-frame-bg-image:linear-gradient(180deg, #1a3348, #0a1824);"));
    assert!(vars.contains("--mei-frame-bg-size:100% 100%;"));
    assert!(vars.contains("--mei-frame-letterbox:#070d14;"));
    assert!(has_frame_backdrop(&props));
    let stage = container_visual_style_without_background(&props);
    assert!(!stage.contains("background-color"));
    assert!(!stage.contains("background-image"));
}

#[test]
fn panel_show_heading_uses_normalized_head_flag() {
    assert!(!panel_show_heading(&json!({"show_heading": false})));
    assert!(!panel_show_heading(&json!({"chrome": "bare"})));
    assert!(!panel_show_heading(&json!({})));
    assert!(panel_show_heading(&json!({"__mei_has_head": true})));
}

