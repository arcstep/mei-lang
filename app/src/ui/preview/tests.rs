use std::collections::BTreeMap;

use super::resolve::{attach_host_meta, resolve_value};
use super::style::{
    block_style, container_visual_style, container_visual_style_without_background,
    metric_slot_vertical_host_class,
    frame_backdrop_css_vars, frame_stage_content_bounds, frame_viewport_letterbox_style,
    has_frame_backdrop, normalize_background_image, panel_card_layout_style, panel_heading_config,
    panel_scale_factor, panel_scaled_outer_style, panel_show_heading, panel_slot_typography_style,
    panel_style, surface_layout_style,
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
    DatasetView, LayoutDecl, LoadedResource, MetricContract, MetricShape, SceneContract, SceneDecl,
    SourceDecl, ThemeDecl,
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

#[test]
fn panel_card_layout_style_preserves_gap_for_content_only_grid() {
    let mut layout = grid_layout();
    layout.gap = Some("8px".to_string());
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("gap:8px;"));
    assert!(!style.contains("gap:0;"));
}

#[test]
fn panel_card_layout_style_zeroes_gap_for_head_body_layout() {
    let layout = LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string()]),
        rows: Some(vec!["auto".to_string(), "1fr".to_string()]),
        areas: Some(vec![vec!["head".to_string()], vec!["body".to_string()]]),
        gap: Some("8px".to_string()),
        padding: Some("0".to_string()),
        align: Some("stretch".to_string()),
        justify: None,
    };
    let style = panel_card_layout_style(Some(&layout), &json!({}));
    assert!(style.contains("gap:0;"));
}

#[test]
fn panel_card_layout_style_applies_heading_height_only_for_head_slot_layouts() {
    let layout = LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string()]),
        rows: Some(vec!["auto".to_string(), "1fr".to_string()]),
        areas: Some(vec![vec!["head".to_string()], vec!["body".to_string()]]),
        gap: Some("0".to_string()),
        padding: Some("0".to_string()),
        align: Some("stretch".to_string()),
        justify: None,
    };
    let style = panel_card_layout_style(
        Some(&layout),
        &json!({
            "height": "54px"
        }),
    );
    assert!(style.contains("grid-template-rows:54px 1fr;"));
}

#[test]
fn panel_card_layout_style_preserves_non_head_grid_rows() {
    let layout = LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec![
            "1fr".to_string(),
            "1fr".to_string(),
            "1fr".to_string(),
        ]),
        rows: Some(vec!["68px".to_string(), "54px".to_string()]),
        areas: Some(vec![
            vec!["top".to_string(), "top".to_string(), "top".to_string()],
            vec!["b0".to_string(), "b1".to_string(), "b2".to_string()],
        ]),
        gap: Some("2px".to_string()),
        padding: Some("2px 4px".to_string()),
        align: Some("stretch".to_string()),
        justify: None,
    };
    let style = panel_card_layout_style(
        Some(&layout),
        &json!({
            "height": "44px"
        }),
    );
    assert!(style.contains("grid-template-rows:68px 54px;"));
    assert!(!style.contains("grid-template-rows:44px;"));
    assert!(!style.contains("grid-template-rows:44px 54px;"));
}

#[test]
fn block_style_uses_full_span_in_grid() {
    let layout = grid_layout();
    assert_eq!(
        block_style(Some("full"), Some(&layout)),
        "grid-column:1 / -1;"
    );
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
fn frame_viewport_letterbox_style_uses_background_color() {
    let style = frame_viewport_letterbox_style(&json!({
        "background": { "color": "rgb(29, 47, 65)" }
    }));
    assert!(style.contains("background:rgb(29, 47, 65);"));
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
fn frame_viewport_config_supports_fluid_height() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1000,
            "design_height": 480,
            "fluid_height": true,
        }
    }))
    .expect("viewport config");
    assert!(vp.fluid_height);
    let locked = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1000,
            "design_height": 480,
            "lock_height": false,
        }
    }))
    .expect("viewport config");
    assert!(locked.fluid_height);
}

#[test]
fn frame_viewport_config_supports_align_and_safe_inset() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1920,
            "design_height": 1080,
            "scale_mode": "contain",
            "align": "top-center",
            "safe_inset": {
                "top": 12,
                "right": 24,
                "bottom": 16,
                "left": 20,
            }
        }
    }))
    .expect("viewport config");
    assert_eq!(vp.align_x, "center");
    assert_eq!(vp.align_y, "start");
    assert_eq!(vp.safe_top, 12.0);
    assert_eq!(vp.safe_right, 24.0);
    assert_eq!(vp.safe_bottom, 16.0);
    assert_eq!(vp.safe_left, 20.0);
}

#[test]
fn effective_viewport_overflow_is_fixed_by_route_not_frame_props() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1920,
            "design_height": 1080,
            "overflow": "scroll",
            "edit_overflow": "clip",
        }
    }))
    .expect("viewport config");
    assert_eq!(
        effective_viewport_overflow(&vp, UiRouteMode::Manage),
        "debug"
    );
    assert_eq!(
        effective_viewport_overflow(&vp, UiRouteMode::Access),
        "clip"
    );
}

#[test]
fn frame_stage_style_debug_caps_canvas_width_to_frame_max_width() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1000,
            "design_height": 480,
            "fluid_height": true,
        }
    }))
    .expect("viewport config");
    let props = json!({
        "max_width": "972px",
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
    let style = frame_stage_style(None, &props, &vp, &theme, "debug");
    assert!(style.contains("width:972px;"));
    assert!(!style.contains("width:1000px;"));
    assert_eq!(effective_canvas_width(&props, &vp), 972.0);
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
    let debug_style = frame_viewport_style_for_route(&vp, "debug", UiRouteMode::Manage);
    assert!(debug_style.contains("overflow-x:auto;"));
    assert!(viewport_overflow_is_debug("debug"));
    assert!(viewport_overflow_is_debug("scroll"));
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
    let debug_style = frame_viewport_style_for_route(&vp, "debug", UiRouteMode::Manage);
    assert!(debug_style.contains("justify-items:start;"));
    assert!(debug_style.contains("align-items:start;"));
    assert!(debug_style.contains("padding:18px 18px 18px 18px;"));

    let access_style = frame_viewport_style_for_route(&vp, "clip", UiRouteMode::Access);
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
        effective_viewport_safe_inset(&vp, UiRouteMode::Access),
        (0.0, 0.0, 0.0, 0.0)
    );
    assert_eq!(
        effective_viewport_safe_inset(&vp, UiRouteMode::Manage),
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

    let resolved = resolve_theme(&scene_contract);
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
fn attach_host_meta_exposes_shared_context_to_components() {
    let compiled = CompiledApp {
        app_id: "preview-shared".to_string(),
        active_scene: Some("home".to_string()),
        active_target_file: "scenes/home.mei".to_string(),
        resources: Vec::new(),
        world_metrics: BTreeMap::new(),
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview-shared".to_string(),
        file_tree: Vec::new(),
        scene_contract: None,
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
    };
    let props = attach_host_meta(
        json!({"value": 1}),
        &compiled,
        "apps/preview-shared",
        &json!({"dataset_table": {"cell_preview_max_chars": 18}}),
        &json!({"layout": {"rail_width": "520px"}}),
        Some("scenes/home.mei"),
    );
    assert_eq!(
        props.get("_mei")
            .and_then(|value| value.get("shared"))
            .and_then(|value| value.get("layout"))
            .and_then(|value| value.get("rail_width"))
            .and_then(Value::as_str),
        Some("520px")
    );
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
        active_target_file: "scenes/home.mei".to_string(),
        resources: Vec::new(),
        world_metrics: BTreeMap::new(),
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview-shared-ref".to_string(),
        file_tree: Vec::new(),
        scene_contract: None,
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
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
    );
    assert_eq!(resolved.get("width").and_then(Value::as_str), Some("520px"));
    assert_eq!(resolved.get("height").and_then(Value::as_i64), Some(74));
}

#[test]
fn resolve_value_supports_data_and_metric_refs() {
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
        "sales_metrics".to_string(),
        LoadedResource {
            id: "sales_metrics".to_string(),
            kind: "dataset".to_string(),
            title: Some("Sales".to_string()),
            document: None,
            dataset: Some(DatasetView {
                id: "sales_metrics".to_string(),
                title: Some("Sales".to_string()),
                purpose: None,
                schema: vec![
                    ColumnSchema {
                        name: "label".to_string(),
                        type_name: "string".to_string(),
                        source: None,
                        optional: false,
                        unit: None,
                    },
                    ColumnSchema {
                        name: "value".to_string(),
                        type_name: "number".to_string(),
                        source: None,
                        optional: false,
                        unit: Some("元".to_string()),
                    },
                ],
                stage_schema: Vec::new(),
                columns: vec!["label".to_string(), "value".to_string()],
                rows: vec![json!({"label":"A","value":"100"})],
                source: SourceDecl {
                    kind: "derived".to_string(),
                    path: "dataset_view:sales_metrics".to_string(),
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
                        label: Some("销售总额".to_string()),
                        unit: Some("元".to_string()),
                        purpose: None,
                        shape: MetricShape::Scalar,
                        schema: vec![ColumnSchema {
                            name: "total_value".to_string(),
                            type_name: "number".to_string(),
                            source: None,
                            optional: false,
                            unit: Some("元".to_string()),
                        }],
                        dataset: None,
                        transforms: Vec::new(),
                        value: json!({"total_value": 100}),
                    },
                )]),
                runtime_metric_defs: BTreeMap::new(),
            }),
        },
    );

    let compiled = CompiledApp {
        app_id: "preview-test".to_string(),
        active_scene: Some("home".to_string()),
        active_target_file: "scenes/home.mei".to_string(),
        resources: resources.values().cloned().collect(),
        world_metrics: BTreeMap::from([(
            "sales_total".to_string(),
            mei_lang_kernel::WorldMetricLedgerEntry {
                id: "sales_total".to_string(),
                owner_resource_id: "sales_metrics".to_string(),
                order: 1,
                metric: resources
                    .get("sales_metrics")
                    .and_then(|resource| resource.dataset.as_ref())
                    .and_then(|dataset| dataset.metrics.get("sales_total"))
                    .cloned()
                    .expect("metric"),
            },
        )]),
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview-test".to_string(),
        file_tree: Vec::new(),
        scene_contract: None,
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
    };
    let resource_index = build_runtime_resource_index(&compiled);
    let scene_anchor = super::resolve::RuntimeSceneAnchor {
        scene_id: "home".to_string(),
        scene_path: Some("scenes/home.mei".to_string()),
    };

    let data_ref = json!({"__ref":"data","id":"sales_metrics"});
    let resolved_data = resolve_value(
        &data_ref,
        &json!({}),
        &scene_contract,
        &resources,
        &scene_anchor,
        &resource_index,
        &compiled,
    );
    assert_eq!(
        resolved_data.get("id").and_then(|value| value.as_str()),
        Some("sales_metrics")
    );
    assert_eq!(
        resolved_data
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("dataset_id"))
            .and_then(|value| value.as_str()),
        Some("sales_metrics")
    );

    let metric_ref = json!({"__ref":"metric","id":"sales_total","from_dataset":"sales_metrics"});
    let resolved_metric = resolve_value(
        &metric_ref,
        &json!({}),
        &scene_contract,
        &resources,
        &scene_anchor,
        &resource_index,
        &compiled,
    );
    assert_eq!(
        resolved_metric.get("id").and_then(|value| value.as_str()),
        Some("sales_total")
    );
    assert_eq!(
        resolved_metric
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("metric_id"))
            .and_then(|value| value.as_str()),
        Some("sales_total")
    );

    let dataset_ref = json!({"__ref": "dataset", "id": "sales_metrics"});
    let resolved_dataset = resolve_value(
        &dataset_ref,
        &json!({}),
        &scene_contract,
        &resources,
        &scene_anchor,
        &resource_index,
        &compiled,
    );
    assert_eq!(
        resolved_dataset.get("id").and_then(|value| value.as_str()),
        Some("sales_metrics")
    );
    assert!(resolved_dataset.get("rows").is_some());
    assert_eq!(
        resolved_dataset
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("kind"))
            .and_then(|value| value.as_str()),
        Some("data")
    );
    assert_eq!(
        resolved_dataset
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("dataset_id"))
            .and_then(|value| value.as_str()),
        Some("sales_metrics")
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
                        purpose: None,
                        shape: MetricShape::Scalar,
                        schema: Vec::new(),
                        dataset: None,
                        transforms: Vec::new(),
                        value: json!({"value": 1}),
                    },
                )]),
                runtime_metric_defs: Default::default(),
            }),
        },
    );
    let compiled = CompiledApp {
        app_id: "preview-alias".to_string(),
        active_scene: Some("home".to_string()),
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
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
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
    );
    assert_eq!(
        resolved
            .get("__mei_runtime_ref")
            .and_then(|value| value.get("dataset_id"))
            .and_then(|value| value.as_str()),
        Some("home")
    );
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
        scene_routes: Vec::new(),
        app_root: ".".to_string(),
        title: "preview-ledger".to_string(),
        file_tree: Vec::new(),
        scene_contract: None,
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
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
