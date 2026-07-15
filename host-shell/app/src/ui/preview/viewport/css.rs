use mei_lang_kernel::LayoutDecl;
use serde_json::Value;

use super::super::style::{
    container_visual_style, container_visual_style_without_background, frame_backdrop_css_vars,
    surface_layout_style,
};
use super::super::theme::{theme_css_vars_style, ThemeResolved};
use crate::ui::route::UiRouteMode;

use super::compute::{
    effective_canvas_width, effective_viewport_overflow, effective_viewport_safe_inset,
    fluid_relaxed_layout, frame_stage_content_bounds_for_viewport, viewport_overflow_is_debug,
    FrameViewportConfig,
};

fn frame_viewport_style_with_safe(
    viewport: &FrameViewportConfig,
    overflow_mode: &str,
    safe_top: f64,
    safe_right: f64,
    safe_bottom: f64,
    safe_left: f64,
) -> String {
    let overflow_css = if viewport_overflow_is_debug(overflow_mode) {
        "overflow-x:auto;overflow-y:auto;"
    } else {
        "overflow:hidden;"
    };
    if viewport_overflow_is_debug(overflow_mode) {
        return format!(
            "width:100%;height:100%;min-width:0;min-height:0;{overflow_css}display:grid;justify-items:{};align-items:{};align-content:start;padding:{}px {}px {}px {}px;box-sizing:border-box;--mei-viewport-design-width:{}px;--mei-viewport-design-height:{}px;--mei-viewport-aspect-ratio:{};",
            viewport.align_x,
            viewport.align_y,
            safe_top,
            safe_right,
            safe_bottom,
            safe_left,
            viewport.design_width,
            viewport.design_height,
            viewport
                .aspect_ratio
                .as_deref()
                .unwrap_or("16:9"),
        );
    }
    format!(
        "width:100%;height:100%;max-width:100%;max-height:100%;min-width:0;min-height:0;{overflow_css}display:flex;align-items:center;justify-content:center;box-sizing:border-box;padding:{}px {}px {}px {}px;--mei-viewport-design-width:{}px;--mei-viewport-design-height:{}px;--mei-viewport-aspect-ratio:{};",
        safe_top,
        safe_right,
        safe_bottom,
        safe_left,
        viewport.design_width,
        viewport.design_height,
        viewport
            .aspect_ratio
            .as_deref()
            .unwrap_or("16:9"),
    )
}

pub(crate) fn frame_viewport_style_for_route(
    viewport: &FrameViewportConfig,
    _overflow_mode: &str,
    route: UiRouteMode,
) -> String {
    if viewport.fluid_height {
        return frame_viewport_style_page_flow_for_route(viewport, route);
    }
    let (safe_top, safe_right, safe_bottom, safe_left) =
        effective_viewport_safe_inset(viewport, route);
    let mode = effective_viewport_overflow(viewport, route);
    frame_viewport_style_with_safe(
        viewport,
        mode.as_str(),
        safe_top,
        safe_right,
        safe_bottom,
        safe_left,
    )
}

/// page-flow（`fluid_height`）：块级纵向堆叠，画布左上贴齐；避免 edit-debug 网格把 shell 挤到右侧。
pub(crate) fn frame_viewport_style_page_flow_for_route(
    viewport: &FrameViewportConfig,
    route: UiRouteMode,
) -> String {
    let (safe_top, safe_right, safe_bottom, safe_left) =
        effective_viewport_safe_inset(viewport, route);
    // Manage：水平留白交给 frame grid padding；视窗铺满中间栏，避免「视窗缩窄 + 右侧对齐」假象。
    let (pad_top, pad_right, pad_bottom, pad_left) = if route == UiRouteMode::Layout {
        (safe_top, 0.0, safe_bottom, 0.0)
    } else {
        (safe_top, safe_right, safe_bottom, safe_left)
    };
    format!(
        "width:100%;min-width:0;min-height:0;height:auto;overflow-x:auto;overflow-y:auto;display:block;box-sizing:border-box;padding:{}px {}px {}px {}px;--mei-viewport-design-width:{}px;--mei-viewport-design-height:{}px;",
        pad_top,
        pad_right,
        pad_bottom,
        pad_left,
        viewport.design_width,
        viewport.design_height,
    )
}

/// `max_width` 限宽：访问态纵向滚动；管理态走 `edit-debug` 样式（见 `mod.rs` 类名分支）。
pub(crate) fn frame_viewport_style_fluid_width_for_route(
    viewport: &FrameViewportConfig,
    _overflow_mode: &str,
    route: UiRouteMode,
) -> String {
    let (safe_top, safe_right, safe_bottom, safe_left) =
        effective_viewport_safe_inset(viewport, route);
    if route == UiRouteMode::Layout {
        return frame_viewport_style_with_safe(
            viewport,
            "debug",
            safe_top,
            safe_right,
            safe_bottom,
            safe_left,
        );
    }
    format!(
        "width:100%;height:100%;max-height:100%;min-width:0;min-height:0;overflow:hidden;display:flex;align-items:center;justify-content:center;box-sizing:border-box;padding:{}px {}px {}px {}px;",
        safe_top,
        safe_right,
        safe_bottom,
        safe_left,
    )
}
pub(crate) fn frame_style(
    layout: Option<&LayoutDecl>,
    props: &Value,
    theme: &ThemeResolved,
) -> String {
    let mut style = surface_layout_style(layout);
    style.push_str(&container_visual_style(props));
    style.push_str(&theme_css_vars_style(theme));
    style
}
pub(crate) fn frame_stage_style(
    layout: Option<&LayoutDecl>,
    props: &Value,
    viewport: &FrameViewportConfig,
    theme: &ThemeResolved,
    overflow_mode: &str,
) -> String {
    if viewport_overflow_is_debug(overflow_mode) {
        let canvas_width = effective_canvas_width(props, viewport);
        let relaxed_layout = viewport
            .fluid_height
            .then(|| fluid_relaxed_layout(layout))
            .flatten();
        let stage_layout = relaxed_layout.as_ref().or(layout);
        let mut style = surface_layout_style(stage_layout);
        style.push_str(&frame_backdrop_css_vars(props));
        style.push_str(&container_visual_style_without_background(props));
        style.push_str(&theme_css_vars_style(theme));
        if viewport.fluid_height {
            style.push_str(&format!(
                "width:min(100%,{}px);max-width:100%;min-height:0;height:auto;align-content:start;transform:none;transform-origin:top left;box-sizing:border-box;",
                canvas_width
            ));
        } else {
            style.push_str(&format!(
                "width:{}px;min-height:{}px;height:auto;max-width:none;transform:none;transform-origin:top left;box-sizing:border-box;",
                canvas_width, viewport.design_height
            ));
        }
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
