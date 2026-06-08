use serde_json::Value;

use super::layout::length_px_from_value;
use super::panel_chrome::container_visual_style;

#[derive(Debug, Clone)]
pub(crate) struct PanelHeadingConfig {
    pub(crate) variant: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) show_accent: bool,
    pub(crate) show_flair: bool,
    pub(crate) show_dots: bool,
}
pub(crate) fn panel_style(
    area: Option<&str>,
    layout: Option<&mei_lang_kernel::LayoutDecl>,
    props: &Value,
) -> String {
    let mut style = panel_position_style(area, layout, props);
    style.push_str(&container_visual_style(props));
    style
}

pub(crate) fn panel_position_style(
    area: Option<&str>,
    layout: Option<&mei_lang_kernel::LayoutDecl>,
    props: &Value,
) -> String {
    let mut style = String::new();
    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid"))
        && area == Some("full")
    {
        style.push_str("grid-column:1 / -1;");
        return style;
    }

    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid"))
        && layout
            .and_then(|value| value.areas.as_ref())
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
    {
        if let Some(area) = area {
            style.push_str(&format!("grid-area:{};", area));
            if layout
                .and_then(|value| value.justify.as_deref())
                .is_some_and(|value| value.trim().eq_ignore_ascii_case("center"))
                && props.as_object().and_then(|map| map.get("width")).is_some()
            {
                style.push_str("justify-self:center;");
            }
            return style;
        }
    }
    style
}

fn scale_factor_from_value(value: &Value) -> Option<f64> {
    let raw = if let Some(number) = value.as_f64() {
        return (number > 0.0 && (number - 1.0).abs() > f64::EPSILON).then_some(number);
    } else if let Some(number) = value.as_i64() {
        let scale = number as f64;
        return (scale > 0.0 && (scale - 1.0).abs() > f64::EPSILON).then_some(scale);
    } else {
        value.as_str()?.trim().to_string()
    };
    if raw.is_empty() {
        return None;
    }
    if let Some(percent) = raw.strip_suffix('%') {
        let numeric = percent.trim().parse::<f64>().ok()?;
        let scale = numeric / 100.0;
        return (scale > 0.0 && (scale - 1.0).abs() > f64::EPSILON).then_some(scale);
    }
    let scale = raw.parse::<f64>().ok()?;
    (scale > 0.0 && (scale - 1.0).abs() > f64::EPSILON).then_some(scale)
}

pub(crate) fn panel_scale_factor(props: &Value) -> Option<f64> {
    props
        .as_object()
        .and_then(|map| map.get("scale"))
        .and_then(scale_factor_from_value)
}

fn scaled_length_style(props: &Value, key: &str, css_name: &str, scale: f64) -> String {
    let Some(map) = props.as_object() else {
        return String::new();
    };
    let Some(value) = map.get(key).and_then(length_px_from_value) else {
        return String::new();
    };
    format!("{css_name}:{}px;", trim_float(value * scale))
}

fn trim_float(value: f64) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    let mut text = rounded.to_string();
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    text
}

pub(crate) fn panel_scaled_outer_style(
    area: Option<&str>,
    layout: Option<&mei_lang_kernel::LayoutDecl>,
    props: &Value,
    scale: f64,
) -> String {
    let mut style = panel_position_style(area, layout, props);
    style.push_str("overflow:visible;box-sizing:border-box;");
    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid")) {
        style.push_str("justify-self:center;align-self:center;");
    }
    style.push_str(&scaled_length_style(props, "width", "width", scale));
    style.push_str(&scaled_length_style(props, "height", "height", scale));
    style.push_str(&scaled_length_style(props, "min_width", "min-width", scale));
    style.push_str(&scaled_length_style(props, "max_width", "max-width", scale));
    style.push_str(&scaled_length_style(
        props,
        "min_height",
        "min-height",
        scale,
    ));
    style.push_str(&scaled_length_style(
        props,
        "max_height",
        "max-height",
        scale,
    ));
    style
}

pub(crate) fn panel_chrome_bare(props: &Value) -> bool {
    let Some(map) = props.as_object() else {
        return false;
    };
    map.get("chrome")
        .and_then(Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("bare"))
}

pub(crate) fn panel_show_heading(props: &Value) -> bool {
    let Some(map) = props.as_object() else {
        return false;
    };
    if let Some(value) = map.get("__mei_has_head").and_then(Value::as_bool) {
        return value;
    }
    if let Some(value) = map.get("show_heading").and_then(Value::as_bool) {
        return value;
    }
    false
}

pub(crate) fn panel_slot_area_style(slot: &str) -> String {
    if slot == "head" {
        return "grid-area:head;min-width:0;min-height:0;width:100%;align-self:start;box-sizing:border-box;"
            .to_string();
    }
    format!(
        "grid-area:{slot};min-width:0;min-height:0;width:100%;height:100%;box-sizing:border-box;"
    )
}

/// `panel_head` / `panel_body` / `head_props` / `body_props` 排版键 → 槽位 inline 样式（与 theme 变量互补）。
pub(crate) fn panel_slot_typography_style(props: &Value) -> String {
    let Some(map) = props.as_object() else {
        return String::new();
    };
    let mut style = String::new();
    for (key, value) in map {
        let css = match key.as_str() {
            "font" | "font_size" => value
                .as_str()
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
                .map(|raw| {
                    if raw.ends_with("px")
                        || raw.ends_with("rem")
                        || raw.ends_with("em")
                        || raw.ends_with('%')
                    {
                        format!("font-size:{raw};")
                    } else {
                        format!("font-size:var(--mei-font-{raw},14px);")
                    }
                })
                .or_else(|| {
                    value
                        .as_i64()
                        .map(|raw| format!("font-size:var(--mei-font-{raw},14px);"))
                }),
            "font_family" => value
                .as_str()
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
                .map(|raw| format!("font-family:{raw};")),
            "color" => value
                .as_str()
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
                .map(|raw| format!("color:{raw};")),
            "font_weight" => value
                .as_str()
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
                .map(|raw| format!("font-weight:{raw};"))
                .or_else(|| value.as_i64().map(|n| format!("font-weight:{n};"))),
            "letter_spacing" => value
                .as_str()
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
                .map(|raw| format!("letter-spacing:{raw};")),
            "text_align" | "align" => value
                .as_str()
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
                .map(|raw| format!("text-align:{raw};")),
            "line_height" => value
                .as_str()
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
                .map(|raw| format!("line-height:{raw};")),
            _ => None,
        };
        if let Some(chunk) = css {
            style.push_str(&chunk);
        }
    }
    style
}
