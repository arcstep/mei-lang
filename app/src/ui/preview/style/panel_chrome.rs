use serde_json::Value;

use super::layout::normalize_background_image;

fn append_background_inline(style: &mut String, background: &Value) {
    match background {
        Value::String(value) if !value.trim().is_empty() => {
            style.push_str(&format!("background:{};", value.trim()));
        }
        Value::Object(bg) => {
            if let Some(value) = bg.get("color").and_then(Value::as_str) {
                style.push_str(&format!("background-color:{};", value));
            }
            if let Some(value) = bg.get("image").and_then(Value::as_str) {
                style.push_str(&format!(
                    "background-image:{};",
                    normalize_background_image(value)
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
}

fn append_background_css_vars(style: &mut String, prefix: &str, background: &Value) {
    match background {
        Value::String(value) if !value.trim().is_empty() => {
            style.push_str(&format!("--{prefix}-bg-image:{};", value.trim()));
        }
        Value::Object(bg) => {
            if let Some(value) = bg.get("color").and_then(Value::as_str) {
                style.push_str(&format!("--{prefix}-bg-color:{};", value));
            }
            if let Some(value) = bg.get("image").and_then(Value::as_str) {
                style.push_str(&format!(
                    "--{prefix}-bg-image:{};",
                    normalize_background_image(value)
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
        Value::String(value) if !value.trim().is_empty() => Some(value.trim().to_string()),
        Value::Object(bg) => bg
            .get("color")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        _ => None,
    }
}

pub(crate) fn frame_viewport_letterbox_style(props: &Value) -> String {
    frame_background_color(props)
        .map(|color| format!("background:{color};"))
        .unwrap_or_default()
}

pub(crate) fn frame_backdrop_css_vars(props: &Value) -> String {
    let Some(map) = props.as_object() else {
        return String::new();
    };
    let Some(background) = map.get("background") else {
        return String::new();
    };
    let mut style = String::new();
    append_background_css_vars(&mut style, "mei-frame", background);
    if let Some(color) = frame_background_color(props) {
        style.push_str(&format!("--mei-frame-letterbox:{color};"));
    }
    style
}

pub(crate) fn has_frame_backdrop(props: &Value) -> bool {
    props
        .as_object()
        .and_then(|map| map.get("background"))
        .is_some_and(|background| match background {
            Value::String(value) => !value.trim().is_empty(),
            Value::Object(bg) => {
                bg.get("color")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
                    || bg
                        .get("image")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.trim().is_empty())
            }
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
        style.push_str(&format!("{css_name}:{value};"));
    }
}

