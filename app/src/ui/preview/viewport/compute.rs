use mei_lang_kernel::LayoutDecl;
use serde_json::{json, Value};

use super::super::style::{frame_stage_content_bounds, FrameStageContentBounds};
use crate::ui::route::UiRouteMode;

#[derive(Debug, Clone)]
pub(crate) struct FrameViewportConfig {
    pub(crate) design_width: f64,
    pub(crate) design_height: f64,
    /// 例如 `"16:9"`，与 design_width / design_height 一并声明时用于锁定画布比例。
    pub(crate) aspect_ratio: Option<String>,
    pub(crate) scale_mode: String,
    /// 已解析、保留兼容；溢出行为由路由固定（Manage=显示溢出，Access=裁切）。
    #[allow(dead_code)]
    pub(crate) overflow: String,
    #[allow(dead_code)]
    pub(crate) edit_overflow: String,
    #[allow(dead_code)]
    pub(crate) edit_scale_mode: String,
    #[allow(dead_code)]
    pub(crate) show_design_bounds: bool,
    /// 为 true 时：管理态调试按 `design_width` 定宽缩放，高度随内容；`design_height` 仅作溢出参考线。
    pub(crate) fluid_height: bool,
    pub(crate) align_x: String,
    pub(crate) align_y: String,
    pub(crate) safe_top: f64,
    pub(crate) safe_right: f64,
    pub(crate) safe_bottom: f64,
    pub(crate) safe_left: f64,
    /// 管理端编辑预览专用安全区（默认同 `safe_inset`）。
    pub(crate) edit_safe_top: f64,
    pub(crate) edit_safe_right: f64,
    pub(crate) edit_safe_bottom: f64,
    pub(crate) edit_safe_left: f64,
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

/// 管理端固定调试溢出（显示裁切外内容）；访问态固定裁切。不读 `edit_overflow` / `overflow`。
pub(crate) fn effective_viewport_overflow(
    _viewport: &FrameViewportConfig,
    route: UiRouteMode,
) -> String {
    match route {
        UiRouteMode::Build | UiRouteMode::Config | UiRouteMode::Upload => "debug".to_string(),
        UiRouteMode::App => "clip".to_string(),
    }
}

pub(crate) fn viewport_overflow_is_debug(mode: &str) -> bool {
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

fn viewport_safe_inset_from(
    viewport: &serde_json::Map<String, Value>,
    inset_key: &str,
    fallback_all_key: Option<&str>,
) -> (f64, f64, f64, f64) {
    let all = fallback_all_key
        .and_then(|key| viewport.get(key))
        .or_else(|| viewport.get("safe_padding"))
        .or_else(|| viewport.get("safe_inset"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
        .max(0.0);
    let Some(inset) = viewport.get(inset_key).and_then(Value::as_object) else {
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

fn viewport_safe_inset(viewport: &serde_json::Map<String, Value>) -> (f64, f64, f64, f64) {
    viewport_safe_inset_from(viewport, "safe_inset", None)
}

pub(crate) fn effective_viewport_safe_inset(
    viewport: &FrameViewportConfig,
    route: UiRouteMode,
) -> (f64, f64, f64, f64) {
    match route {
        UiRouteMode::Build | UiRouteMode::Config | UiRouteMode::Upload => (
            viewport.edit_safe_top,
            viewport.edit_safe_right,
            viewport.edit_safe_bottom,
            viewport.edit_safe_left,
        ),
        UiRouteMode::App => (
            viewport.safe_top,
            viewport.safe_right,
            viewport.safe_bottom,
            viewport.safe_left,
        ),
    }
}
/// 页面流式布局（`profile=page` 等默认）：左右留白、左上对齐、高度随内容延伸。
pub(crate) fn default_viewport_page_flow() -> FrameViewportConfig {
    frame_viewport_config(&json!({
        "viewport": {
            "enabled": true,
            "design_width": 1280,
            "design_height": 720,
            "scale_mode": "contain",
            "fluid_height": true,
            "align": "top-left",
            "edit_scale_mode": "fit-width",
            "show_design_bounds": true,
            "safe_inset": { "top": 0, "right": 0, "bottom": 0, "left": 0 },
            "edit_safe_inset": { "top": 32, "right": 24, "bottom": 16, "left": 24 }
        }
    }))
    .expect("default page-flow viewport")
}

/// 固定舞台（`profile=cockpit` 默认）：锁定宽高比，contain 缩放，不足处 letterbox 居中。
pub(crate) fn default_viewport_stage_lock() -> FrameViewportConfig {
    frame_viewport_config(&json!({
        "viewport": {
            "enabled": true,
            "design_width": 1920,
            "design_height": 1080,
            "aspect_ratio": "16:9",
            "scale_mode": "contain",
            "fluid_height": false,
            "align": "center",
            "edit_scale_mode": "contain",
            "show_design_bounds": true,
            "safe_inset": { "top": 0, "right": 0, "bottom": 0, "left": 0 },
            "edit_safe_inset": { "top": 32, "right": 16, "bottom": 12, "left": 16 }
        }
    }))
    .expect("default stage-lock viewport")
}

/// 无 `frame.props.viewport` 时按 scene profile 选用默认视窗。
pub(crate) fn default_viewport_for_profile(profile: Option<&str>) -> FrameViewportConfig {
    match profile.unwrap_or("page").trim() {
        "cockpit" => default_viewport_stage_lock(),
        _ => default_viewport_page_flow(),
    }
}

/// 是否在 `frame.props` 中显式声明了 `viewport`（不含 profile 默认）。
pub(crate) fn frame_viewport_is_explicit(props: &Value) -> bool {
    frame_viewport_config(props).is_some()
}

/// 合并显式 `frame.props.viewport` 与 profile 默认。
pub(crate) fn resolve_frame_viewport(
    props: &Value,
    profile: Option<&str>,
) -> Option<FrameViewportConfig> {
    frame_viewport_config(props).or_else(|| Some(default_viewport_for_profile(profile)))
}

pub(crate) fn frame_viewport_config(props: &Value) -> Option<FrameViewportConfig> {
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
        .unwrap_or("fit-width")
        .to_string();
    let show_design_bounds = viewport
        .get("show_design_bounds")
        .or_else(|| viewport.get("showDesignBounds"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let fluid_height = viewport
        .get("fluid_height")
        .or_else(|| viewport.get("fluidHeight"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            viewport
                .get("lock_height")
                .or_else(|| viewport.get("lockHeight"))
                .and_then(Value::as_bool)
                .is_some_and(|locked| !locked)
        });
    let (align_x, align_y) = viewport_align(viewport);
    let (safe_top, safe_right, safe_bottom, safe_left) = viewport_safe_inset(viewport);
    let (edit_safe_top, edit_safe_right, edit_safe_bottom, edit_safe_left) =
        if viewport.contains_key("edit_safe_inset") {
            viewport_safe_inset_from(viewport, "edit_safe_inset", None)
        } else {
            (safe_top, safe_right, safe_bottom, safe_left)
        };
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
        fluid_height,
        align_x,
        align_y,
        safe_top,
        safe_right,
        safe_bottom,
        safe_left,
        edit_safe_top,
        edit_safe_right,
        edit_safe_bottom,
        edit_safe_left,
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
pub(crate) fn frame_stage_content_bounds_for_viewport(
    props: &Value,
    viewport: &FrameViewportConfig,
) -> FrameStageContentBounds {
    frame_stage_content_bounds(props, viewport.design_width, viewport.design_height)
}

/// 舞台可用宽度：`frame.max_width` 等上限与 `design_width` 取较小值，避免设计画布宽于实际布局。
pub(crate) fn effective_canvas_width(props: &Value, viewport: &FrameViewportConfig) -> f64 {
    let bounds = frame_stage_content_bounds_for_viewport(props, viewport);
    match bounds.max_width {
        Some(cap) => cap.min(viewport.design_width),
        None => viewport.design_width,
    }
}

/// page-flow：把 `1fr` / `minmax(..., 1fr)` 行改为 `auto`，避免表格行被撑满设计高度留白。
pub(crate) fn fluid_relaxed_layout(layout: Option<&LayoutDecl>) -> Option<LayoutDecl> {
    let layout = layout.cloned()?;
    if layout.layout_type != "grid" {
        return Some(layout);
    }
    let mut relaxed = layout;
    if let Some(rows) = relaxed.rows.as_mut() {
        *rows = rows
            .iter()
            .map(|row| {
                let trimmed = row.trim();
                if trimmed.eq_ignore_ascii_case("1fr")
                    || trimmed.contains("minmax(") && trimmed.contains("fr")
                    || trimmed.ends_with("fr")
                {
                    "auto".to_string()
                } else {
                    row.clone()
                }
            })
            .collect();
    }
    Some(relaxed)
}
