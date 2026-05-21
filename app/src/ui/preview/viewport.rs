use mei_lang_kernel::LayoutDecl;
use serde_json::Value;

use super::style::{
    container_visual_style, container_visual_style_without_background, frame_backdrop_css_vars,
    surface_layout_style,
};
use super::theme::{theme_css_vars_style, ThemeResolved};

#[derive(Debug, Clone)]
pub(super) struct FrameViewportConfig {
    pub(super) design_width: f64,
    pub(super) design_height: f64,
    pub(super) scale_mode: String,
    pub(super) align_x: String,
    pub(super) align_y: String,
    pub(super) safe_top: f64,
    pub(super) safe_right: f64,
    pub(super) safe_bottom: f64,
    pub(super) safe_left: f64,
}

fn viewport_align(viewport: &serde_json::Map<String, Value>) -> (String, String) {
    let align_x = viewport
        .get("align_x")
        .and_then(Value::as_str)
        .map(normalize_align_x);
    let align_y = viewport
        .get("align_y")
        .and_then(Value::as_str)
        .map(normalize_align_y);
    if align_x.is_some() || align_y.is_some() {
        return (
            align_x.unwrap_or_else(|| "center".to_string()),
            align_y.unwrap_or_else(|| "center".to_string()),
        );
    }
    let align = viewport
        .get("align")
        .and_then(Value::as_str)
        .unwrap_or("center");
    match align.trim().to_ascii_lowercase().as_str() {
        "top" | "top-center" => ("center".to_string(), "start".to_string()),
        "top-left" => ("start".to_string(), "start".to_string()),
        "top-right" => ("end".to_string(), "start".to_string()),
        "bottom" | "bottom-center" => ("center".to_string(), "end".to_string()),
        "bottom-left" => ("start".to_string(), "end".to_string()),
        "bottom-right" => ("end".to_string(), "end".to_string()),
        "left" | "center-left" => ("start".to_string(), "center".to_string()),
        "right" | "center-right" => ("end".to_string(), "center".to_string()),
        _ => ("center".to_string(), "center".to_string()),
    }
}

fn normalize_align_x(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "left" | "start" => "start".to_string(),
        "right" | "end" => "end".to_string(),
        _ => "center".to_string(),
    }
}

fn normalize_align_y(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "top" | "start" => "start".to_string(),
        "bottom" | "end" => "end".to_string(),
        _ => "center".to_string(),
    }
}

fn viewport_safe_inset(viewport: &serde_json::Map<String, Value>) -> (f64, f64, f64, f64) {
    let all = viewport
        .get("safe_padding")
        .or_else(|| viewport.get("safe_inset"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .max(0.0);
    let Some(inset) = viewport.get("safe_inset").and_then(Value::as_object) else {
        return (all, all, all, all);
    };
    let top = inset
        .get("top")
        .and_then(Value::as_f64)
        .unwrap_or(all)
        .max(0.0);
    let right = inset
        .get("right")
        .and_then(Value::as_f64)
        .unwrap_or(all)
        .max(0.0);
    let bottom = inset
        .get("bottom")
        .and_then(Value::as_f64)
        .unwrap_or(all)
        .max(0.0);
    let left = inset
        .get("left")
        .and_then(Value::as_f64)
        .unwrap_or(all)
        .max(0.0);
    (top, right, bottom, left)
}

pub(super) fn frame_viewport_style(viewport: &FrameViewportConfig) -> String {
    format!(
        "width:100%;height:100%;min-width:0;min-height:0;overflow:hidden;display:grid;justify-items:{};align-items:{};padding:{}px {}px {}px {}px;",
        viewport.align_x,
        viewport.align_y,
        viewport.safe_top,
        viewport.safe_right,
        viewport.safe_bottom,
        viewport.safe_left,
    )
}

pub(super) fn frame_viewport_config(props: &Value) -> Option<FrameViewportConfig> {
    let map = props.as_object()?;
    let viewport = map.get("viewport")?.as_object()?;
    if viewport
        .get("enabled")
        .and_then(Value::as_bool)
        .is_some_and(|value| !value)
    {
        return None;
    }
    let design_width = viewport
        .get("design_width")
        .and_then(Value::as_f64)
        .filter(|value| *value > 0.0)?;
    let design_height = viewport
        .get("design_height")
        .and_then(Value::as_f64)
        .filter(|value| *value > 0.0)?;
    let scale_mode = viewport
        .get("scale_mode")
        .and_then(Value::as_str)
        .unwrap_or("contain")
        .to_string();
    let (align_x, align_y) = viewport_align(viewport);
    let (safe_top, safe_right, safe_bottom, safe_left) = viewport_safe_inset(viewport);
    Some(FrameViewportConfig {
        design_width,
        design_height,
        scale_mode,
        align_x,
        align_y,
        safe_top,
        safe_right,
        safe_bottom,
        safe_left,
    })
}

pub(super) fn frame_style(
    layout: Option<&LayoutDecl>,
    props: &Value,
    theme: &ThemeResolved,
) -> String {
    let mut style = surface_layout_style(layout);
    style.push_str(&container_visual_style(props));
    style.push_str(&theme_css_vars_style(theme));
    style
}

pub(super) fn frame_stage_style(
    layout: Option<&LayoutDecl>,
    props: &Value,
    viewport: &FrameViewportConfig,
    theme: &ThemeResolved,
) -> String {
    let mut style = surface_layout_style(layout);
    style.push_str(&frame_backdrop_css_vars(props));
    style.push_str(&container_visual_style_without_background(props));
    style.push_str(&theme_css_vars_style(theme));
    style.push_str(&format!(
        "width:{}px;height:{}px;transform-origin:top left;",
        viewport.design_width, viewport.design_height
    ));
    style
}
