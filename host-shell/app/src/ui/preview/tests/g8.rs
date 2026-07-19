use super::theme::ThemeResolved;
use super::viewport::{
    effective_canvas_width, effective_viewport_overflow, frame_stage_style, frame_viewport_config,
};
use crate::ui::route::UiRouteMode;
use mei_lang_kernel::LayoutDecl;
use serde_json::json;

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
        effective_viewport_overflow(&vp, UiRouteMode::Layout),
        "debug"
    );
    assert_eq!(effective_viewport_overflow(&vp, UiRouteMode::App), "clip");
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
    assert!(style.contains("min-height:0;"));
    assert!(!style.contains("min-height:480px;"));
    assert_eq!(effective_canvas_width(&props, &vp), 972.0);
}

#[test]
fn frame_stage_style_fluid_height_relaxes_fr_grid_rows() {
    let vp = frame_viewport_config(&json!({
        "viewport": {
            "design_width": 1280,
            "design_height": 720,
            "fluid_height": true,
        }
    }))
    .expect("viewport config");
    let layout = LayoutDecl {
        layout_type: "grid".to_string(),
        direction: None,
        columns: Some(vec!["1fr".to_string()]),
        rows: Some(vec!["auto".to_string(), "minmax(360px, 1fr)".to_string()]),
        areas: Some(vec![vec!["doc".to_string()], vec!["table".to_string()]]),
        gap: Some("16px".to_string()),
        padding: Some("20px".to_string()),
        align: None,
        justify: None,
    };
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
    let style = frame_stage_style(Some(&layout), &json!({}), &vp, &theme, "debug");
    assert!(style.contains("grid-template-rows:auto auto;"));
    assert!(!style.contains("minmax(360px, 1fr)"));
}
