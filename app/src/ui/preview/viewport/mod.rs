mod compute;
mod css;

pub(crate) use compute::{
    effective_canvas_width, effective_viewport_overflow, frame_stage_content_bounds_for_viewport,
    frame_viewport_is_explicit, resolve_frame_viewport,
};

#[cfg(test)]
pub(crate) use compute::{
    default_viewport_page_flow, default_viewport_stage_lock, effective_viewport_safe_inset,
    frame_viewport_config, viewport_overflow_is_debug,
};
pub(crate) use css::{
    frame_stage_style, frame_style, frame_viewport_style_fluid_width_for_route,
    frame_viewport_style_for_route,
};
