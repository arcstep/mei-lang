use mei_lang_kernel::LayoutDecl;
use serde_json::Value;

use crate::ui::route::UiRouteMode;
use super::style::{
    container_visual_style, container_visual_style_without_background, frame_backdrop_css_vars,
    frame_stage_content_bounds, surface_layout_style, FrameStageContentBounds,
};
use super::theme::{theme_css_vars_style, ThemeResolved};

#[derive(Debug, Clone)]
pub(super) struct FrameViewportConfig {
    pub(super) design_width: f64,
    pub(super) design_height: f64,
    /// 例如 `"16:9"`，与 design_width / design_height 一并声明时用于锁定画布比例。
    pub(super) aspect_ratio: Option<String>,
    pub(super) scale_mode: String,
    /// 访问 / 运行态：`clip` 按比例缩放并裁切溢出；`scroll` 允许视口内滚动。
    pub(super) overflow: String,
    /// 管理端编辑预览：`debug` / `scroll` 等比缩放 + 分辨率框 + 可见溢出；`clip` 与访问态一致。
    pub(super) edit_overflow: String,
    /// 编辑调试态缩放（默认 `contain`）；访问态仍用 `scale_mode`（如 `cover`）。
    pub(super) edit_scale_mode: String,
    pub(super) show_design_bounds: bool,
    pub(super) align_x: String,
    pub(super) align_y: String,
    pub(super) safe_top: f64,
    pub(super) safe_right: f64,
    pub(super) safe_bottom: f64,
    pub(super) safe_left: f64,
}

fn parse_overflow_token(value: Option<&Value>, default: &str) -> String {
    let raw = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(default)
        .to_ascii_lowercase();
    match raw.as_str() {
        "debug" => "debug".to_string(),
        "scroll" | "visible" => "debug".to_string(),
        "clip" | "hidden" => raw,
        "auto" => "debug".to_string(),
        _ => default.to_string(),
    }
}

pub(super) fn effective_viewport_overflow(viewport: &FrameViewportConfig, route: UiRouteMode) -> String {
    match route {
        UiRouteMode::Manage => viewport.edit_overflow.clone(),
        UiRouteMode::Access => viewport.overflow.clone(),
    }
}

pub(super) fn viewport_overflow_is_debug(mode: &str) -> bool {
    matches!(mode, "debug" | "scroll" | "visible")
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

pub(super) fn frame_viewport_style(viewport: &FrameViewportConfig, overflow_mode: &str) -> String {
    let overflow_css = if viewport_overflow_is_debug(overflow_mode) {
        "overflow-x:auto;overflow-y:auto;"
    } else {
        "overflow:hidden;"
    };
    format!(
        "width:100%;height:100%;min-width:0;min-height:0;{overflow_css}display:grid;justify-items:{};align-items:{};align-content:{};padding:{}px {}px {}px {}px;--mei-viewport-design-width:{}px;--mei-viewport-design-height:{}px;--mei-viewport-aspect-ratio:{};",
        viewport.align_x,
        viewport.align_y,
        if viewport_overflow_is_debug(overflow_mode) {
            "start"
        } else {
            "stretch"
        },
        viewport.safe_top,
        viewport.safe_right,
        viewport.safe_bottom,
        viewport.safe_left,
        viewport.design_width,
        viewport.design_height,
        viewport
            .aspect_ratio
            .as_deref()
            .unwrap_or("16:9"),
    )
}

/// `max_width` 限宽时：视口高度仍占满宿主，但允许纵向滚动以看完所有 panel。
pub(super) fn frame_viewport_style_fluid_width(
    viewport: &FrameViewportConfig,
    _overflow_mode: &str,
) -> String {
    format!(
        "width:100%;height:100%;max-height:100%;min-width:0;min-height:0;overflow-x:hidden;overflow-y:auto;display:grid;justify-items:{};align-items:{};align-content:start;padding:{}px {}px {}px {}px;",
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
    let aspect_ratio = viewport
        .get("aspect_ratio")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string);
    let overflow = parse_overflow_token(viewport.get("overflow"), "clip");
    let edit_overflow = parse_overflow_token(
        viewport
            .get("edit_overflow")
            .or_else(|| viewport.get("editOverflow")),
        "debug",
    );
    let edit_scale_mode = viewport
        .get("edit_scale_mode")
        .or_else(|| viewport.get("editScaleMode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("contain")
        .to_string();
    let show_design_bounds = viewport
        .get("show_design_bounds")
        .or_else(|| viewport.get("showDesignBounds"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let (align_x, align_y) = viewport_align(viewport);
    let (safe_top, safe_right, safe_bottom, safe_left) = viewport_safe_inset(viewport);
    let (design_width, design_height) =
        apply_aspect_ratio_lock(design_width, design_height, aspect_ratio.as_deref());
    Some(FrameViewportConfig {
        design_width,
        design_height,
        aspect_ratio,
        scale_mode,
        overflow,
        edit_overflow,
        edit_scale_mode,
        show_design_bounds,
        align_x,
        align_y,
        safe_top,
        safe_right,
        safe_bottom,
        safe_left,
    })
}

fn apply_aspect_ratio_lock(width: f64, height: f64, aspect_ratio: Option<&str>) -> (f64, f64) {
    let Some(raw) = aspect_ratio.map(str::trim).filter(|s| !s.is_empty()) else {
        return (width, height);
    };
    let Some((w_part, h_part)) = raw.split_once(':') else {
        return (width, height);
    };
    let rw: f64 = w_part.trim().parse().unwrap_or(0.0);
    let rh: f64 = h_part.trim().parse().unwrap_or(0.0);
    if rw <= 0.0 || rh <= 0.0 {
        return (width, height);
    }
    let target = rw / rh;
    let current = width / height;
    if (current - target).abs() < 0.001 {
        return (width, height);
    }
    (width, width / target)
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

pub(super) fn frame_stage_content_bounds_for_viewport(
    props: &Value,
    viewport: &FrameViewportConfig,
) -> FrameStageContentBounds {
    frame_stage_content_bounds(props, viewport.design_width, viewport.design_height)
}

pub(super) fn frame_stage_style(
    layout: Option<&LayoutDecl>,
    props: &Value,
    viewport: &FrameViewportConfig,
    theme: &ThemeResolved,
    overflow_mode: &str,
) -> String {
    if viewport_overflow_is_debug(overflow_mode) {
        let mut style = surface_layout_style(layout);
        style.push_str(&frame_backdrop_css_vars(props));
        style.push_str(&container_visual_style_without_background(props));
        style.push_str(&theme_css_vars_style(theme));
        style.push_str(&format!(
            "width:{}px;min-height:{}px;height:auto;max-width:none;transform:none;transform-origin:top left;box-sizing:border-box;",
            viewport.design_width, viewport.design_height
        ));
        return style;
    }
    let bounds = frame_stage_content_bounds_for_viewport(props, viewport);
    let mut style = surface_layout_style(layout);
    style.push_str(&frame_backdrop_css_vars(props));
    style.push_str(&container_visual_style_without_background(props));
    style.push_str(&theme_css_vars_style(theme));
    if bounds.max_width.is_some() {
        style.push_str(
            "max-width:100%;width:100%;height:auto;min-height:0;transform:none;transform-origin:top left;",
        );
        if let Some(max_width) = bounds.max_width {
            style.push_str(&format!("--mei-frame-content-max-width:{}px;", max_width));
        }
    } else {
        style.push_str(&format!(
            "width:{}px;height:{}px;transform-origin:top left;",
            bounds.fallback_width, bounds.height
        ));
    }
    style
}
