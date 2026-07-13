//! Container / metric_card `surface` defaults (replaces chrome_profile + *.metric.json).

use serde_json::{json, Value};

const TPL_METRICS: &str = "/workspace-app-assets/templates/cockpit/assets/metrics";

/// Visual chrome props for a surface token. Field layout is separate (`surface_field_layout_call`).
pub fn surface_chrome_props(surface: &str) -> Option<Value> {
    match normalize_surface(surface) {
        "none" | "plain" | "compound_top_row" | "compound_sub_stack" => None,
        "solid" | "solid_stack" => Some(chrome_like("solid_stack")),
        "narrow" | "narrow_metric" => Some(chrome_like("narrow_metric")),
        "compound" | "compound_frame" => Some(chrome_like("compound_frame")),
        "stack_desc" | "icon_left" => Some(chrome_like("stack_desc")),
        // progress 条由 slot 底图 + desc_mode=progress；内层 metric 透明壳。
        "stack_progress" => Some(chrome_like("stack_progress")),
        "strip_icon_left" => Some(chrome_like("strip_icon_left")),
        "solid_row_accent" | "solid_row_compact" => Some(chrome_like("solid_row_accent")),
        other => Some(chrome_like(other)),
    }
}

/// Template preset name used by metric shell / slot generation.
pub fn surface_template_name(surface: &str) -> &'static str {
    match normalize_surface(surface) {
        "solid" | "solid_stack" => "solid_stack",
        "narrow" | "narrow_metric" | "plain" | "none" | "compound_sub_stack" => "plain",
        "compound_top_row" | "solid_row_accent" | "solid_row_compact" => "solid_row_accent",
        "stack_desc" | "stack_progress" => "stack_desc",
        "icon_left" => "icon_left",
        "strip_icon_left" => "strip_icon_left",
        _ => "plain",
    }
}

/// Default field grid for a surface when author omits `layout = grid(...)`.
pub fn surface_field_layout_call(surface: &str) -> Value {
    match normalize_surface(surface) {
        "compound_top_row" | "solid_row_accent" | "solid_row_compact" | "strip_icon_left" => {
            grid_call(
                &["1fr"],
                &["auto", "auto", "auto"],
                &[vec!["label", "value", "unit"]],
                "4px",
                "center",
                "start",
            )
        }
        // 与 stack_progress_skin / metric_stack_desc 同构：label 通栏 + value|unit 并排 + desc。
        // 用 minmax/fr 随 slot 拉伸，禁止六行单列把 value/unit 拆开（会导致挤压重叠）。
        "stack_desc" | "stack_progress" => grid_call(
            &[
                "minmax(4px, 1fr)",
                "minmax(14px, auto)",
                "minmax(20px, 2fr)",
                "minmax(4px, 0.5fr)",
                "10px",
                "minmax(2px, 1fr)",
            ],
            &["auto", "auto"],
            &[
                vec![".", "."],
                vec!["label", "label"],
                vec!["value", "unit"],
                vec![".", "."],
                vec!["desc", "desc"],
                vec![".", "."],
            ],
            "0",
            "stretch",
            "center",
        ),
        _ => grid_call(
            &["2fr", "3fr"],
            &["1fr", "auto"],
            &[vec!["label", "label"], vec!["value", "unit"]],
            "2px",
            "stretch",
            "stretch",
        ),
    }
}

pub fn normalize_surface(surface: &str) -> &str {
    let s = surface.trim();
    if s.is_empty() {
        "none"
    } else {
        s
    }
}

fn chrome_like(profile: &str) -> Value {
    match profile {
        "transparent" => json!({
            "padding": "0",
            "background": "transparent",
            "border": "none",
            "width": "100%",
            "height": "100%",
            "min_height": "0",
            "box_sizing": "border-box",
            "overflow": "hidden"
        }),
        "bare_fill" => json!({
            "padding": "0",
            "background": "transparent",
            "width": "100%",
            "height": "100%",
            "min_height": "0",
            "box_sizing": "border-box",
            "overflow": "hidden",
            "__mei_layout_fill": true
        }),
        // 纯色/SVG 指标卡默认无圆角；作者 props.radius 仍可覆盖。
        "narrow_metric" | "solid_stack" | "stack_desc" | "icon_left" => json!({
            "padding": "0 2px",
            "background": slot_stretch_background("metric-bg-normal@3x.svg"),
            "border": "none",
            "radius": "0",
            "width": "100%",
            "height": "100%",
            "min_height": "0",
            "box_sizing": "border-box",
            "overflow": "hidden",
            "__mei_slot_frame_bg": true
        }),
        "stack_progress" => json!({
            "padding": "0 2px",
            "background": slot_stretch_background("metric-bg-clean@3x.svg"),
            "border": "none",
            "radius": "0",
            "width": "100%",
            "height": "100%",
            "min_height": "0",
            "box_sizing": "border-box",
            "overflow": "hidden",
            "__mei_slot_frame_bg": true,
            "__mei_slot_bg_stretch": true,
            "__mei_metric_template": "stack_desc",
            "__mei_metric_density": "compact",
            "__mei_metric_inline_align": "compact",
            "__mei_metric_desc_mode": "progress",
            "metric_desc_mode": "progress",
            "metric_desc_shell": {
                "inset_x": "0px",
                "extend_x": "10px",
                "height": "10px",
                "border_radius": "0",
                "fill": "#C9E9F8"
            },
            "__mei_metric_label_v_align": "center",
            "__mei_metric_value_v_align": "end",
            "__mei_metric_unit_v_align": "end"
        }),
        "compound_frame" => json!({
            "padding": "0 4px",
            "background": slot_stretch_background("metric-bg-target@3x.svg"),
            "border": "none",
            "radius": "4px",
            "width": "100%",
            "height": "100%",
            "min_height": "0",
            "box_sizing": "border-box",
            "overflow": "hidden",
            "__mei_slot_frame_bg": true
        }),
        "strip_icon_left" => json!({
            "padding": "0",
            "background": slot_stretch_background("metric-bg-strip@3x.svg"),
            "border": "none",
            "width": "100%",
            "height": "100%",
            "min_height": "0",
            "box_sizing": "border-box",
            "overflow": "hidden",
            "__mei_slot_frame_bg": true
        }),
        "solid_row_accent" | "solid_row_compact" => json!({
            "padding": "0 4px",
            "background": {
                "color": "rgba(98,190,235,0.10)",
                "size": "100% 100%",
                "origin": "border-box",
                "clip": "border-box",
            },
            "border": "none",
            "width": "100%",
            "height": "100%",
            "min_height": "0",
            "box_sizing": "border-box",
            "overflow": "hidden",
            "__mei_slot_frame_bg": true
        }),
        _ => json!({
            "padding": "0",
            "background": "transparent",
            "width": "100%",
            "height": "100%",
            "min_height": "0",
            "box_sizing": "border-box",
            "overflow": "hidden"
        }),
    }
}

fn slot_stretch_background(image: &str) -> Value {
    json!({
        "color": "rgba(98,190,235,0.10)",
        "image": format!("url({TPL_METRICS}/{image})"),
        "size": "100% 100%",
        "position": "center",
        "repeat": "no-repeat",
        "origin": "border-box",
        "clip": "border-box",
    })
}

fn grid_call(
    rows: &[&str],
    columns: &[&str],
    areas: &[Vec<&str>],
    gap: &str,
    align: &str,
    justify: &str,
) -> Value {
    json!({
        "__call": "grid",
        "__args": {
            "rows": rows,
            "columns": columns,
            "areas": areas,
            "gap": gap,
            "align": align,
            "justify": justify,
        }
    })
}

/// Apply surface chrome onto panel/metric props. Records `surface` for runtime.
pub fn apply_surface_to_props(props: &mut Value, surface: &str) {
    let surface = normalize_surface(surface);
    if let Some(map) = props.as_object_mut() {
        map.insert("surface".to_string(), json!(surface));
    }
    if let Some(chrome) = surface_chrome_props(surface) {
        deep_merge(props, &chrome);
    }
}

fn deep_merge(base: &mut Value, overlay: &Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                if let Some(existing) = base_map.get_mut(key) {
                    deep_merge(existing, value);
                } else {
                    base_map.insert(key.clone(), value.clone());
                }
            }
        }
        (base_slot, overlay) => *base_slot = overlay.clone(),
    }
}
