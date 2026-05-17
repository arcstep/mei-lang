use serde_json::Value;

use super::theme::deep_merge_value;

#[derive(Debug, Clone)]
pub(super) struct PanelHeadingConfig {
    pub(super) variant: String,
    pub(super) subtitle: Option<String>,
    pub(super) show_accent: bool,
    pub(super) show_flair: bool,
    pub(super) show_dots: bool,
}

pub(super) fn surface_layout_style(layout: Option<&mei_lang_kernel::LayoutDecl>) -> String {
    let Some(layout) = layout else {
        return "display:grid;gap:16px;".to_string();
    };
    match layout.layout_type.as_str() {
        "flex" => format!(
            "display:flex;flex-direction:{};gap:{};padding:{};",
            layout
                .direction
                .clone()
                .unwrap_or_else(|| "column".to_string()),
            layout.gap.clone().unwrap_or_else(|| "16px".to_string()),
            layout.padding.clone().unwrap_or_else(|| "0".to_string()),
        ),
        _ => format!(
            "display:grid;grid-template-columns:{};grid-template-rows:{};{}gap:{};padding:{};",
            layout
                .columns
                .clone()
                .unwrap_or_else(|| vec!["1fr".to_string()])
                .join(" "),
            layout
                .rows
                .clone()
                .unwrap_or_else(|| vec!["auto".to_string()])
                .join(" "),
            grid_template_areas_style(layout),
            layout.gap.clone().unwrap_or_else(|| "16px".to_string()),
            layout.padding.clone().unwrap_or_else(|| "0".to_string()),
        ),
    }
}

pub(super) fn panel_style(
    area: Option<&str>,
    layout: Option<&mei_lang_kernel::LayoutDecl>,
    props: &Value,
) -> String {
    let mut style = String::new();
    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid"))
        && area == Some("full")
    {
        style.push_str("grid-column:1 / -1;");
        style.push_str(&container_visual_style(props));
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
            style.push_str(&container_visual_style(props));
            return style;
        }
    }
    style.push_str(&container_visual_style(props));
    style
}

pub(super) fn panel_show_heading(props: &Value) -> bool {
    let Some(map) = props.as_object() else {
        return true;
    };
    if let Some(value) = map.get("show_heading").and_then(Value::as_bool) {
        return value;
    }
    !matches!(map.get("chrome").and_then(Value::as_str), Some("bare"))
}

pub(super) fn panel_heading_config(theme_heading: &Value, props: &Value) -> PanelHeadingConfig {
    let mut variant = "default".to_string();
    let mut subtitle = None;
    let mut show_accent = None;
    let mut show_flair = None;
    let mut show_dots = None;

    let heading_props = deep_merge_value(
        theme_heading,
        &props
            .as_object()
            .and_then(|map| map.get("heading"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    );

    if let Some(map) = props.as_object() {
        subtitle = map
            .get("subtitle")
            .and_then(Value::as_str)
            .map(|value| value.to_string());
    }
    if let Some(heading) = heading_props.as_object() {
        if let Some(value) = heading.get("variant").and_then(Value::as_str) {
            variant = value.to_string();
        }
        if let Some(value) = heading.get("subtitle").and_then(Value::as_str) {
            subtitle = Some(value.to_string());
        }
        show_accent = heading.get("accent").and_then(Value::as_bool);
        show_flair = heading.get("flair").and_then(Value::as_bool);
        show_dots = heading.get("dots").and_then(Value::as_bool);
    }

    let (default_accent, default_flair, default_dots) = match variant.as_str() {
        "screen" => (true, true, true),
        "compact" => (true, false, false),
        "plain" => (false, false, false),
        _ => (true, false, false),
    };

    PanelHeadingConfig {
        variant,
        subtitle,
        show_accent: show_accent.unwrap_or(default_accent),
        show_flair: show_flair.unwrap_or(default_flair),
        show_dots: show_dots.unwrap_or(default_dots),
    }
}

pub(super) fn container_visual_style(props: &Value) -> String {
    let Some(map) = props.as_object() else {
        return String::new();
    };
    let mut style = String::new();

    if let Some(background) = map.get("background") {
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
            _ => {}
        }
    }

    append_string_style(&mut style, map.get("padding"), "padding");
    append_string_style(&mut style, map.get("margin"), "margin");
    append_string_style(&mut style, map.get("border"), "border");
    append_string_style(&mut style, map.get("radius"), "border-radius");
    append_string_style(&mut style, map.get("box_shadow"), "box-shadow");
    append_string_style(&mut style, map.get("overflow"), "overflow");
    append_string_style(&mut style, map.get("min_height"), "min-height");
    append_string_style(&mut style, map.get("min_width"), "min-width");

    style
}

pub(super) fn append_string_style(style: &mut String, value: Option<&Value>, css_name: &str) {
    if let Some(value) = value.and_then(Value::as_str) {
        style.push_str(&format!("{css_name}:{value};"));
    }
}

pub(super) fn normalize_background_image(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "none".to_string();
    }
    if value.contains('(') || value.starts_with("var(") || value.starts_with("url(") {
        value.to_string()
    } else {
        format!("url(\"{}\")", value.replace('"', "%22"))
    }
}

pub(super) fn panel_body_style(layout: Option<&mei_lang_kernel::LayoutDecl>) -> String {
    let Some(layout) = layout else {
        return String::new();
    };
    match layout.layout_type.as_str() {
        "flex" => format!(
            "display:flex;flex-direction:{};gap:{};padding:{};",
            layout
                .direction
                .clone()
                .unwrap_or_else(|| "column".to_string()),
            layout.gap.clone().unwrap_or_else(|| "12px".to_string()),
            layout.padding.clone().unwrap_or_else(|| "0".to_string()),
        ),
        _ => format!(
            "display:grid;grid-template-columns:{};grid-template-rows:{};{}gap:{};padding:{};",
            layout
                .columns
                .clone()
                .unwrap_or_else(|| vec!["1fr".to_string()])
                .join(" "),
            layout
                .rows
                .clone()
                .unwrap_or_else(|| vec!["auto".to_string()])
                .join(" "),
            grid_template_areas_style(layout),
            layout.gap.clone().unwrap_or_else(|| "12px".to_string()),
            layout.padding.clone().unwrap_or_else(|| "0".to_string()),
        ),
    }
}

pub(super) fn block_style(
    area: Option<&str>,
    layout: Option<&mei_lang_kernel::LayoutDecl>,
) -> String {
    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid"))
        && area == Some("full")
    {
        return "grid-column:1 / -1;".to_string();
    }

    if matches!(layout.map(|value| value.layout_type.as_str()), Some("grid"))
        && layout
            .and_then(|value| value.areas.as_ref())
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
    {
        if let Some(area) = area {
            if !area.trim().is_empty() && area != "auto" {
                return format!("grid-area:{};", area);
            }
        }
    }
    String::new()
}

pub(super) fn grid_template_areas_style(layout: &mei_lang_kernel::LayoutDecl) -> String {
    let Some(rows) = layout.areas.as_ref() else {
        return String::new();
    };
    let rows = rows
        .iter()
        .filter(|row| !row.is_empty())
        .map(|row| {
            let template = row
                .iter()
                .map(|area| {
                    let area = area.trim();
                    if area.is_empty() {
                        "."
                    } else {
                        area
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("'{template}'")
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        String::new()
    } else {
        format!("grid-template-areas:{};", rows.join(" "))
    }
}
