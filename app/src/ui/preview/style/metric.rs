use serde_json::Value;


use super::layout::length_px_from_props;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FrameStageContentBounds {
    /// 内容区最大宽度（px）；`None` 表示按设计稿全宽。
    pub max_width: Option<f64>,
    pub height: f64,
    pub fallback_width: f64,
}

fn frame_width_is_fluid(props: &Value) -> bool {
    props
        .as_object()
        .and_then(|map| map.get("width"))
        .and_then(Value::as_str)
        .is_some_and(|raw| {
            let value = raw.trim();
            value.ends_with('%') || value.eq_ignore_ascii_case("auto")
        })
}

/// Frame viewport 下 stage 尺寸语义：`max_width` 为上限；`width: 100%` 在上限内铺满宿主；
/// 仅写 `width: Npx` 且无 `max_width` 时，将 N 视为上限（便于与旧示例兼容）。
pub(crate) fn frame_stage_content_bounds(
    props: &Value,
    design_width: f64,
    design_height: f64,
) -> FrameStageContentBounds {
    let height = length_px_from_props(props, "height")
        .or_else(|| length_px_from_props(props, "min_height"))
        .unwrap_or(design_height);
    let max_from_prop = length_px_from_props(props, "max_width");
    let width_px = length_px_from_props(props, "width");
    let max_width = match (max_from_prop, width_px) {
        (Some(cap), _) => Some(cap),
        (None, Some(cap)) if !frame_width_is_fluid(props) => Some(cap),
        _ => None,
    };
    FrameStageContentBounds {
        max_width,
        height,
        fallback_width: design_width,
    }
}
/// 指标槽 `metric_v_align` → component-card 上的垂直定位 class（host 缩为内容高，由 card 的 justify-content 落位）。
pub(crate) fn metric_slot_vertical_host_class(props: &Value) -> &'static str {
    let Some(raw) = props
        .as_object()
        .and_then(|map| map.get("metric_v_align"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return "component-card--slot-v-center";
    };
    match raw.to_ascii_lowercase().as_str() {
        "start" | "top" => "component-card--slot-v-start",
        "end" | "bottom" | "baseline" => "component-card--slot-v-end",
        _ => "component-card--slot-v-center",
    }
}
