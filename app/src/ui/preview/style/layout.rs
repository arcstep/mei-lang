use serde_json::Value;


pub(crate) fn surface_layout_style(layout: Option<&mei_lang_kernel::LayoutDecl>) -> String {
    let Some(layout) = layout else {
        return "display:grid;gap:16px;".to_string();
    };
    match layout.layout_type.as_str() {
        "flex" => format!(
            "display:flex;flex-direction:{};{}{}gap:{};padding:{};",
            layout
                .direction
                .clone()
                .unwrap_or_else(|| "column".to_string()),
            layout_align_items_style(layout.align.as_deref()),
            layout_justify_content_style(layout.justify.as_deref()),
            layout
                .gap
                .as_deref()
                .map(normalize_css_length)
                .unwrap_or_else(|| "16px".to_string()),
            layout
                .padding
                .as_deref()
                .map(normalize_css_length)
                .unwrap_or_else(|| "0".to_string()),
        ),
        _ => format!(
            "display:grid;grid-template-columns:{};grid-template-rows:{};{}{}{}{}gap:{};padding:{};",
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
            layout_align_items_style(layout.align.as_deref()),
            layout_justify_items_style(layout.justify.as_deref()),
            layout_justify_content_style(layout.justify.as_deref()),
            layout
                .gap
                .as_deref()
                .map(normalize_css_length)
                .unwrap_or_else(|| "16px".to_string()),
            layout
                .padding
                .as_deref()
                .map(normalize_css_length)
                .unwrap_or_else(|| "0".to_string()),
        ),
    }
}

fn layout_align_items_style(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| format!("align-items:{raw};"))
        .unwrap_or_default()
}

fn layout_justify_items_style(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| format!("justify-items:{raw};"))
        .unwrap_or_default()
}

fn layout_justify_content_style(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| format!("justify-content:{raw};"))
        .unwrap_or_default()
}
pub(crate) fn length_px_from_value(value: &Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        return Some(number);
    }
    if let Some(number) = value.as_i64() {
        return Some(number as f64);
    }
    let raw = value.as_str()?.trim();
    if raw.is_empty() {
        return None;
    }
    let number = raw.strip_suffix("px").unwrap_or(raw).trim();
    number.parse().ok()
}

pub(crate) fn length_px_from_props(props: &Value, key: &str) -> Option<f64> {
    props
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(length_px_from_value)
}

pub(crate) fn normalize_css_length(raw: &str) -> String {
    let value = raw.trim();
    if value.is_empty() {
        return value.to_string();
    }
    if value
        .chars()
        .any(|ch| ch.is_ascii_alphabetic() || ch == '%')
    {
        return value.to_string();
    }
    if value.ends_with("px") {
        return value.to_string();
    }
    format!("{value}px")
}

pub(crate) fn normalize_background_image(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return "none".to_string();
    }
    if value.contains('(') || value.starts_with("var(") || value.starts_with("url(") {
        value.to_string()
    } else {
        format!("url(\"{}\")", value.replace('"', "%22"))
    }
}
pub(crate) fn block_style(
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
                let fill_row = area != "head";
                // desc/label 槽须占满行宽；width:auto 会使 mei-text（width:100%）坍缩为 0。
                let span_row_width = area == "desc" || area == "label";
                let centered = layout
                    .and_then(|value| value.justify.as_deref())
                    .is_some_and(|value| value.trim().eq_ignore_ascii_case("center"));
                let mut style = if fill_row && centered && !span_row_width {
                    format!(
                        "grid-area:{area};min-width:0;min-height:0;width:auto;height:100%;align-self:stretch;justify-self:center;box-sizing:border-box;"
                    )
                } else if fill_row {
                    format!(
                        "grid-area:{area};min-width:0;min-height:0;width:100%;height:100%;align-self:stretch;box-sizing:border-box;"
                    )
                } else {
                    format!(
                        "grid-area:{area};min-width:0;width:100%;height:100%;box-sizing:border-box;"
                    )
                };
                if layout
                    .and_then(|value| value.areas.as_ref())
                    .is_some_and(|rows| {
                        rows.first()
                            .is_some_and(|row| row.len() > 1 && row.iter().all(|cell| cell == area))
                    })
                {
                    style.push_str("grid-column:1 / -1;");
                }
                return style;
            }
        }
    }
    String::new()
}

pub(crate) fn grid_template_areas_style(layout: &mei_lang_kernel::LayoutDecl) -> String {
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
