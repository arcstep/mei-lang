use serde_json::Value;

use super::layout::normalize_background_image;
use crate::ui::preview::theme::{resolve_color_token, resolve_gradient_token, resolve_style_value};

fn resolve_background_image_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let resolved = resolve_gradient_token(trimmed);
    normalize_background_image(resolved.as_str())
}

fn append_background_inline(style: &mut String, background: &Value) {
    match background {
        Value::String(value) if !value.trim().is_empty() => {
            let trimmed = value.trim();
            if trimmed.eq_ignore_ascii_case("transparent") {
                style.push_str("background:transparent;");
            } else {
                style.push_str(&format!(
                    "background:{};",
                    resolve_background_image_value(trimmed)
                ));
            }
        }
        Value::Object(bg) => {
            if let Some(value) = bg.get("color").and_then(Value::as_str) {
                style.push_str(&format!("background-color:{};", resolve_color_token(value)));
            }
            if let Some(value) = bg.get("image").and_then(Value::as_str) {
                style.push_str(&format!(
                    "background-image:{};",
                    resolve_background_image_value(value)
                ));
            }
            append_background_layer_props(style, bg);
        }
        _ => {}
    }
}

fn append_background_layer_props(style: &mut String, bg: &serde_json::Map<String, Value>) {
    if let Some(value) = bg.get("size").and_then(Value::as_str) {
        style.push_str(&format!("background-size:{};", value));
    }
    if let Some(value) = bg.get("position").and_then(Value::as_str) {
        style.push_str(&format!("background-position:{};", value));
    }
    if let Some(value) = bg.get("repeat").and_then(Value::as_str) {
        style.push_str(&format!("background-repeat:{};", value));
    }
    if let Some(value) = bg.get("attachment").and_then(Value::as_str) {
        style.push_str(&format!("background-attachment:{};", value));
    }
    if let Some(value) = bg.get("blend_mode").and_then(Value::as_str) {
        style.push_str(&format!("background-blend-mode:{};", value));
    }
    if let Some(value) = bg.get("origin").and_then(Value::as_str) {
        style.push_str(&format!("background-origin:{};", value));
    }
    if let Some(value) = bg.get("clip").and_then(Value::as_str) {
        style.push_str(&format!("background-clip:{};", value));
    }
}

fn append_background_css_vars(style: &mut String, prefix: &str, background: &Value) {
    match background {
        Value::String(value) if !value.trim().is_empty() => {
            style.push_str(&format!(
                "--{prefix}-bg-image:{};",
                resolve_background_image_value(value.trim())
            ));
        }
        Value::Object(bg) => {
            if let Some(value) = bg.get("color").and_then(Value::as_str) {
                style.push_str(&format!(
                    "--{prefix}-bg-color:{};",
                    resolve_color_token(value)
                ));
            }
            if let Some(value) = bg.get("image").and_then(Value::as_str) {
                style.push_str(&format!(
                    "--{prefix}-bg-image:{};",
                    resolve_background_image_value(value)
                ));
            }
            if let Some(value) = bg.get("size").and_then(Value::as_str) {
                style.push_str(&format!("--{prefix}-bg-size:{};", value));
            }
            if let Some(value) = bg.get("position").and_then(Value::as_str) {
                style.push_str(&format!("--{prefix}-bg-position:{};", value));
            }
            if let Some(value) = bg.get("repeat").and_then(Value::as_str) {
                style.push_str(&format!("--{prefix}-bg-repeat:{};", value));
            }
            if let Some(value) = bg.get("attachment").and_then(Value::as_str) {
                style.push_str(&format!("--{prefix}-bg-attachment:{};", value));
            }
            if let Some(value) = bg.get("blend_mode").and_then(Value::as_str) {
                style.push_str(&format!("--{prefix}-bg-blend-mode:{};", value));
            }
        }
        _ => {}
    }
}

pub(crate) fn frame_background_color(props: &Value) -> Option<String> {
    let background = props.as_object()?.get("background")?;
    match background {
        Value::String(value) if !value.trim().is_empty() => Some(resolve_color_token(value.trim())),
        Value::Object(bg) => bg
            .get("color")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(resolve_color_token),
        _ => None,
    }
}

const DEFAULT_FRAME_LETTERBOX: &str = "#070d14";

/// 视口外 letterbox 底色：与舞台 `frame.background` 分离，默认可搭配棋盘格纹。
pub(crate) fn frame_letterbox_color(props: &Value) -> String {
    let Some(map) = props.as_object() else {
        return DEFAULT_FRAME_LETTERBOX.to_string();
    };
    let Some(letterbox) = map.get("letterbox") else {
        return DEFAULT_FRAME_LETTERBOX.to_string();
    };
    match letterbox {
        Value::String(value) if !value.trim().is_empty() => resolve_color_token(value.trim()),
        Value::Object(lb) => lb
            .get("color")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(resolve_color_token)
            .unwrap_or_else(|| DEFAULT_FRAME_LETTERBOX.to_string()),
        _ => DEFAULT_FRAME_LETTERBOX.to_string(),
    }
}

pub(crate) fn frame_viewport_letterbox_style(props: &Value) -> String {
    format!("--mei-frame-letterbox:{};", frame_letterbox_color(props))
}

pub(crate) fn frame_backdrop_css_vars(props: &Value) -> String {
    let Some(map) = props.as_object() else {
        return String::new();
    };
    let mut style = format!("--mei-frame-letterbox:{};", frame_letterbox_color(props));
    if let Some(background) = map.get("background") {
        append_background_css_vars(&mut style, "mei-frame", background);
    }
    style
}

pub(crate) fn has_frame_backdrop(props: &Value) -> bool {
    if frame_background_color(props).is_some() {
        return true;
    }
    props
        .as_object()
        .and_then(|map| map.get("background"))
        .is_some_and(|background| match background {
            Value::String(value) => !value.trim().is_empty(),
            Value::Object(bg) => bg
                .get("image")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty()),
            _ => false,
        })
}

pub(crate) fn container_visual_style_without_background(props: &Value) -> String {
    let Some(map) = props.as_object() else {
        return String::new();
    };
    let mut style = String::new();
    append_string_style(&mut style, map.get("padding"), "padding");
    append_string_style(&mut style, map.get("margin"), "margin");
    append_string_style(&mut style, map.get("border"), "border");
    append_string_style(&mut style, map.get("radius"), "border-radius");
    append_string_style(&mut style, map.get("box_shadow"), "box-shadow");
    append_string_style(&mut style, map.get("overflow"), "overflow");
    append_string_style(&mut style, map.get("min_height"), "min-height");
    append_string_style(&mut style, map.get("height"), "height");
    append_string_style(&mut style, map.get("width"), "width");
    append_string_style(&mut style, map.get("position"), "position");
    append_string_style(
        &mut style,
        map.get("z_index").or_else(|| map.get("z-index")),
        "z-index",
    );
    append_string_style(&mut style, map.get("top"), "top");
    append_string_style(&mut style, map.get("left"), "left");
    append_string_style(&mut style, map.get("right"), "right");
    append_string_style(&mut style, map.get("bottom"), "bottom");
    append_string_style(&mut style, map.get("max_width"), "max-width");
    append_string_style(&mut style, map.get("min_width"), "min-width");
    append_string_style(&mut style, map.get("box_sizing"), "box-sizing");
    append_string_style(
        &mut style,
        map.get("pointer_events")
            .or_else(|| map.get("pointer-events")),
        "pointer-events",
    );
    style
}
pub(crate) fn container_visual_style(props: &Value) -> String {
    let Some(map) = props.as_object() else {
        return String::new();
    };
    let mut style = String::new();

    if let Some(background) = map.get("background") {
        append_background_inline(&mut style, background);
    }

    style.push_str(&container_visual_style_without_background(props));
    style
}

pub(crate) fn append_string_style(style: &mut String, value: Option<&Value>, css_name: &str) {
    if let Some(value) = value.and_then(Value::as_str) {
        let resolved = if matches!(css_name, "border" | "box-shadow") {
            resolve_style_value(value)
        } else {
            value.to_string()
        };
        style.push_str(&format!("{css_name}:{resolved};"));
        return;
    }
    if css_name == "z-index" {
        if let Some(number) = value.and_then(Value::as_i64) {
            style.push_str(&format!("{css_name}:{number};"));
        } else if let Some(number) = value.and_then(Value::as_u64) {
            style.push_str(&format!("{css_name}:{number};"));
        } else if let Some(number) = value.and_then(Value::as_f64) {
            style.push_str(&format!("{css_name}:{number};"));
        }
    }
}
