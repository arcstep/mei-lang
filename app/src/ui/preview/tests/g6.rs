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
fn panel_heading_uses_theme_panel_head_and_head_props() {
    let theme_head = json!({"variant": "plain", "accent": false});
    let cfg = panel_heading_config(&theme_head, &json!({}), &json!({}));
    assert_eq!(cfg.variant, "plain");
    assert!(!cfg.show_flair);
    let cfg_screen = panel_heading_config(
        &theme_head,
        &json!({"variant": "screen", "flair": true}),
        &json!({}),
    );
    assert_eq!(cfg_screen.variant, "screen");
    assert!(cfg_screen.show_flair);
}

#[test]
fn resolve_panel_card_props_strips_heading_from_card() {
    let theme = ThemeResolved {
        id: "page".to_string(),
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
    let panel = PanelDecl {
        kind: "panel".to_string(),
        id: "p".to_string(),
        title: None,
        head: None,
        area: None,
        layout: None,
        blocks: vec![],
        props: json!({"heading": {"variant": "screen"}, "border": "1px solid red"}),
        head_props: json!({}),
        body_props: json!({}),
        base: None,
        import_scope: None,
        slot: None,
    };
    let card = resolve_panel_card_props(&theme, &panel);
    assert!(card.get("heading").is_none());
    assert_eq!(
        card.get("border").and_then(Value::as_str),
        Some("1px solid red")
    );
}

#[test]
fn panel_slot_typography_style_maps_theme_font_keys() {
    assert_eq!(
        panel_slot_typography_style(&json!({"font": "4"})),
        "font-size:var(--mei-font-4,14px);"
    );
    assert_eq!(
        panel_slot_typography_style(&json!({"font": 3})),
        "font-size:var(--mei-font-3,14px);"
    );
    assert_eq!(
        panel_slot_typography_style(&json!({"font": "18px"})),
        "font-size:18px;"
    );
    assert!(panel_slot_typography_style(&json!({})).is_empty());
}

#[test]
fn resolve_panel_head_props_merges_theme_and_panel() {
    let theme = ThemeResolved {
        id: "cockpit".to_string(),
        frame: json!({}),
        panel: json!({}),
        panel_bare: json!({}),
        panel_head: json!({"variant": "plain"}),
        panel_body: json!({}),
        heading: json!({}),
        shared: json!({}),
        components: json!({}),
        css_vars: Vec::new(),
    };
    let panel = PanelDecl {
        kind: "panel".to_string(),
        id: "p".to_string(),
        title: None,
        head: None,
        area: None,
        layout: None,
        blocks: vec![],
        props: json!({}),
        head_props: json!({"height": "54px"}),
        body_props: json!({}),
        base: None,
        import_scope: None,
        slot: None,
    };
    let head = resolve_panel_head_props(&theme, &panel);
    assert_eq!(head.get("variant").and_then(Value::as_str), Some("plain"));
    assert_eq!(head.get("height").and_then(Value::as_str), Some("54px"));
}

#[test]
fn resolve_panel_props_merges_theme_panel_defaults() {
    let theme = ThemeResolved {
        id: "page".to_string(),
        frame: json!({}),
        panel: json!({
            "padding": "12px",
            "border": "1px solid #334155",
        }),
        panel_bare: json!({
            "padding": "0",
            "border": "none",
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
            "padding": "4px",
        }),
    );
    assert_eq!(resolved.get("padding").and_then(Value::as_str), Some("4px"));
    assert_eq!(
        resolved.get("border").and_then(Value::as_str),
        Some("1px solid #334155")
    );
}

