use serde_json::Value;

use super::super::theme::deep_merge_value;
use super::layout::{normalize_background_image, normalize_css_length, surface_layout_style};
use super::panel::PanelHeadingConfig;

/// `head_props.carets`：单张图右侧原图、左侧 `left_rotate`（默认 180deg），由 CSS 伪元素绘制。
pub(crate) fn panel_head_carets_enabled(head_props: &Value) -> bool {
    head_props
        .as_object()
        .and_then(|map| map.get("carets"))
        .and_then(|value| value.as_object())
        .and_then(|map| map.get("url"))
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
}

fn caret_pos_str(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        map.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(crate) fn panel_head_carets_slot_mode(head_props: &Value) -> bool {
    let Some(map) = head_props
        .as_object()
        .and_then(|head| head.get("carets"))
        .and_then(Value::as_object)
    else {
        return false;
    };
    caret_pos_str(map, &["left", "left_slot"]).is_some()
        && caret_pos_str(map, &["right", "right_slot"]).is_some()
}

pub(crate) fn panel_head_caret_style(head_props: &Value) -> String {
    let Some(carets) = head_props.as_object().and_then(|map| map.get("carets")) else {
        return String::new();
    };
    let Some(map) = carets.as_object() else {
        return String::new();
    };
    let Some(url) = map.get("url").and_then(Value::as_str) else {
        return String::new();
    };
    let url = url.trim();
    if url.is_empty() {
        return String::new();
    }
    let inset = map
        .get("inset")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("14px");
    let left_rotate = map
        .get("left_rotate")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("180deg");
    let size = map
        .get("size")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("14px 24px");
    let mut style = format!(
        "--mei-head-caret-url:{};--mei-head-caret-inset:{};--mei-head-caret-left-rotate:{};--mei-head-caret-size:{};",
        normalize_background_image(url),
        inset,
        left_rotate,
        size
    );
    if let Some(left_pos) = caret_pos_str(map, &["left", "left_slot"]) {
        style.push_str(&format!("--mei-head-caret-left-pos:{left_pos};"));
    }
    if let Some(right_pos) = caret_pos_str(map, &["right", "right_slot"]) {
        style.push_str(&format!("--mei-head-caret-right-pos:{right_pos};"));
    }
    style
}

/// 整卡 grid：来自 `panel.layout`；`props.heading.height` 与 `rows` 合并为 `grid-template-rows`。
pub(crate) fn panel_card_layout_style(
    layout: Option<&mei_lang_kernel::LayoutDecl>,
    props: &Value,
) -> String {
    let Some(layout) = layout else {
        return String::new();
    };
    let slots: Vec<&str> = layout
        .areas
        .as_ref()
        .map(|areas| {
            areas
                .iter()
                .flat_map(|row| row.iter().map(String::as_str))
                .collect()
        })
        .unwrap_or_default();
    let has_head = slots.iter().any(|slot| *slot == "head");
    let has_body = slots.iter().any(|slot| *slot == "body");
    let mut style = surface_layout_style(Some(layout));
    let heading_height = if has_head {
        heading_chrome_props(props)
            .as_object()
            .and_then(|map| map.get("height"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    } else {
        None
    };
    if let Some(heading_height) = heading_height.as_deref() {
        let heading_row = normalize_css_length(heading_height);
        let body_row = layout
            .rows
            .as_ref()
            .and_then(|rows| rows.get(1).map(String::as_str))
            .or_else(|| {
                layout
                    .rows
                    .as_ref()
                    .and_then(|rows| rows.first().map(String::as_str))
            });
        if let Some(body_row) = body_row {
            if has_body {
                style.push_str(&format!(
                    "grid-template-rows:{} {};",
                    heading_row,
                    normalize_css_length(body_row)
                ));
            } else {
                style.push_str(&format!("grid-template-rows:{};", heading_row));
            }
        } else {
            style.push_str(&format!("grid-template-rows:{} 1fr;", heading_row));
        }
    } else {
        patch_default_head_body_grid_rows(&mut style, layout);
    }
    if has_head || has_body {
        style.push_str("gap:0;");
    }
    style
}

fn patch_default_head_body_grid_rows(style: &mut String, layout: &mei_lang_kernel::LayoutDecl) {
    let Some(rows) = layout.rows.as_ref() else {
        return;
    };
    let slots: Vec<&str> = layout
        .areas
        .as_ref()
        .map(|areas| {
            areas
                .iter()
                .flat_map(|row| row.iter().map(String::as_str))
                .collect()
        })
        .unwrap_or_default();
    let has_head = slots.iter().any(|slot| *slot == "head");
    let has_body = slots.iter().any(|slot| *slot == "body");
    if has_head && has_body && rows.len() >= 2 && rows[0] == "auto" && rows[1] == "1fr" {
        style.push_str("grid-template-rows:minmax(max-content,auto) minmax(0,1fr);");
    } else if has_head && !has_body && rows.first().is_some_and(|row| row == "auto") {
        style.push_str("grid-template-rows:minmax(max-content,auto);");
    }
}

fn heading_chrome_props(head_props: &Value) -> Value {
    let Some(map) = head_props.as_object() else {
        return head_props.clone();
    };
    let Some(chrome) = map.get("chrome").filter(|value| value.is_object()) else {
        return head_props.clone();
    };
    deep_merge_value(chrome, head_props)
}

pub(crate) fn panel_heading_config(
    theme_panel_head: &Value,
    head_props: &Value,
    card_props: &Value,
) -> PanelHeadingConfig {
    let mut variant = "default".to_string();
    let mut subtitle = None;
    let mut show_accent = None;
    let mut show_flair = None;
    let mut show_dots = None;

    let heading_props = heading_chrome_props(head_props);
    let heading_props = deep_merge_value(theme_panel_head, &heading_props);

    if let Some(map) = card_props.as_object() {
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

/// `head_props` 的 height / align → head 单元格 inline 样式。
pub(crate) fn panel_heading_style(head_props: &Value) -> String {
    let chrome_props = heading_chrome_props(head_props);
    let Some(map) = chrome_props.as_object() else {
        return String::new();
    };
    let mut style = String::new();
    if let Some(value) = map.get("height").and_then(Value::as_str) {
        let px = normalize_css_length(value);
        style.push_str(&format!("height:{px};min-height:{px};"));
    } else {
        if let Some(value) = map.get("min_height").and_then(Value::as_str) {
            style.push_str(&format!("min-height:{};", normalize_css_length(value)));
        }
        if let Some(value) = map.get("max_height").and_then(Value::as_str) {
            style.push_str(&format!("max-height:{};", normalize_css_length(value)));
        }
    }
    if map
        .get("align")
        .and_then(Value::as_str)
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("center"))
    {
        style.push_str(
            "display:flex;align-items:center;justify-content:center;padding:0;box-sizing:border-box;overflow:hidden;",
        );
        style.push_str("width:100%;");
    }
    style
}

/// 无标题槽、且 `layout.areas` 不含 `head`/`body`（如 `m0 m1 m2`）时，grid 应落在 `panel-body-cell` 上。
pub(crate) fn panel_layout_content_on_body_slot(
    layout: Option<&mei_lang_kernel::LayoutDecl>,
) -> bool {
    let Some(layout) = layout else {
        return false;
    };
    let Some(areas) = layout.areas.as_ref() else {
        return false;
    };
    if areas.is_empty() {
        return false;
    }
    !areas
        .iter()
        .flatten()
        .any(|cell| cell == "head" || cell == "body")
}

pub(crate) fn panel_body_layout_centered(layout: &mei_lang_kernel::LayoutDecl) -> bool {
    layout
        .align
        .as_deref()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("center"))
        && layout
            .justify
            .as_deref()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("center"))
}
