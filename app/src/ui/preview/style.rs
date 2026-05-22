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

pub(super) fn panel_chrome_bare(props: &Value) -> bool {
    let Some(map) = props.as_object() else {
        return false;
    };
    map.get("chrome")
        .and_then(Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("bare"))
}

pub(super) fn panel_show_heading(props: &Value) -> bool {
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

pub(super) fn panel_slot_area_style(slot: &str) -> String {
    if slot == "head" {
        return "grid-area:head;min-width:0;min-height:0;width:100%;align-self:start;box-sizing:border-box;"
            .to_string();
    }
    format!("grid-area:{slot};min-width:0;min-height:0;width:100%;height:100%;box-sizing:border-box;")
}

/// `panel_head` / `panel_body` / `head_props` / `body_props` 的 `font` 键 → 槽位默认字号（子组件可 `props.font` 覆盖）。
pub(super) fn panel_slot_typography_style(props: &Value) -> String {
    let Some(map) = props.as_object() else {
        return String::new();
    };
    let Some(font) = map.get("font") else {
        return String::new();
    };
    let key = match font {
        Value::String(raw) => raw.trim().to_string(),
        Value::Number(raw) => {
            if let Some(n) = raw.as_i64() {
                n.to_string()
            } else if let Some(n) = raw.as_f64() {
                if (n - n.round()).abs() < f64::EPSILON {
                    format!("{}", n.round() as i64)
                } else {
                    n.to_string()
                }
            } else {
                return String::new();
            }
        }
        _ => return String::new(),
    };
    if key.is_empty() {
        return String::new();
    }
    if key.ends_with("px")
        || key.ends_with("rem")
        || key.ends_with("em")
        || key.ends_with('%')
    {
        return format!("font-size:{key};");
    }
    format!("font-size:var(--mei-font-{key},14px);")
}

/// `head_props.carets`：单张图右侧原图、左侧 `left_rotate`（默认 180deg），由 CSS 伪元素绘制。
pub(super) fn panel_head_carets_enabled(head_props: &Value) -> bool {
    head_props
        .as_object()
        .and_then(|map| map.get("carets"))
        .and_then(|value| value.as_object())
        .and_then(|map| map.get("url"))
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
}

pub(super) fn panel_head_caret_style(head_props: &Value) -> String {
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
    format!(
        "--mei-head-caret-url:{};--mei-head-caret-inset:{};--mei-head-caret-left-rotate:{};--mei-head-caret-size:{};",
        normalize_background_image(url),
        inset,
        left_rotate,
        size
    )
}

/// 整卡 grid：来自 `panel.layout`；`props.heading.height` 与 `rows` 合并为 `grid-template-rows`。
pub(super) fn panel_card_layout_style(
    layout: Option<&mei_lang_kernel::LayoutDecl>,
    props: &Value,
) -> String {
    let Some(layout) = layout else {
        return String::new();
    };
    let mut style = surface_layout_style(Some(layout));
    let chrome_props = heading_chrome_props(props);
    let heading_height = chrome_props
        .as_object()
        .and_then(|map| map.get("height"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(heading_height) = heading_height {
        let heading_row = normalize_css_length(heading_height);
        let body_row = layout
            .rows
            .as_ref()
            .and_then(|rows| rows.get(1).map(String::as_str))
            .or_else(|| layout.rows.as_ref().and_then(|rows| rows.first().map(String::as_str)));
        if let Some(body_row) = body_row {
            if layout
                .areas
                .as_ref()
                .is_some_and(|areas| areas.iter().flatten().any(|cell| cell == "body"))
            {
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
    style.push_str("gap:0;");
    style
}

fn patch_default_head_body_grid_rows(style: &mut String, layout: &mei_lang_kernel::LayoutDecl) {
    let Some(rows) = layout.rows.as_ref() else {
        return;
    };
    let slots: Vec<&str> = layout
        .areas
        .as_ref()
        .map(|areas| areas.iter().flat_map(|row| row.iter().map(String::as_str)).collect())
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

pub(super) fn panel_heading_config(
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
pub(super) fn panel_heading_style(head_props: &Value) -> String {
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

pub(super) fn panel_body_layout_centered(layout: &mei_lang_kernel::LayoutDecl) -> bool {
    layout
        .align
        .as_deref()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("center"))
        && layout
            .justify
            .as_deref()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("center"))
}

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

pub(super) fn frame_background_color(props: &Value) -> Option<String> {
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

pub(super) fn frame_viewport_letterbox_style(props: &Value) -> String {
    frame_background_color(props)
        .map(|color| format!("background:{color};"))
        .unwrap_or_default()
}

pub(super) fn frame_backdrop_css_vars(props: &Value) -> String {
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

pub(super) fn has_frame_backdrop(props: &Value) -> bool {
    props
        .as_object()
        .and_then(|map| map.get("background"))
        .is_some_and(|background| match background {
            Value::String(value) => !value.trim().is_empty(),
            Value::Object(bg) => {
                bg.get("color")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty())
                    || bg.get("image")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.trim().is_empty())
            }
            _ => false,
        })
}

pub(super) fn container_visual_style_without_background(props: &Value) -> String {
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
    append_string_style(&mut style, map.get("max_width"), "max-width");
    append_string_style(&mut style, map.get("min_width"), "min-width");
    append_string_style(&mut style, map.get("box_sizing"), "box-sizing");
    style
}

pub(super) fn length_px_from_value(value: &Value) -> Option<f64> {
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

pub(super) fn length_px_from_props(props: &Value, key: &str) -> Option<f64> {
    props
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(length_px_from_value)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FrameStageContentBounds {
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
pub(super) fn frame_stage_content_bounds(
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

pub(super) fn container_visual_style(props: &Value) -> String {
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

pub(super) fn append_string_style(style: &mut String, value: Option<&Value>, css_name: &str) {
    if let Some(value) = value.and_then(Value::as_str) {
        style.push_str(&format!("{css_name}:{value};"));
    }
}

/// `gap` / `padding` 等：纯数字自动补 `px`，避免 `gap:5` 被浏览器忽略。
pub(super) fn normalize_css_length(raw: &str) -> String {
    let value = raw.trim();
    if value.is_empty() {
        return value.to_string();
    }
    if value.chars().any(|ch| ch.is_ascii_alphabetic() || ch == '%') {
        return value.to_string();
    }
    if value.ends_with("px") {
        return value.to_string();
    }
    format!("{value}px")
}

pub(super) fn normalize_background_image(value: &str) -> String {
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
                let fill_row = area != "head";
                let mut style = if fill_row {
                    format!(
                        "grid-area:{area};min-width:0;min-height:0;width:100%;height:100%;align-self:stretch;box-sizing:border-box;"
                    )
                } else {
                    format!(
                        "grid-area:{area};min-width:0;width:100%;box-sizing:border-box;"
                    )
                };
                if layout
                    .and_then(|value| value.areas.as_ref())
                    .is_some_and(|rows| {
                        rows.first().is_some_and(|row| {
                            row.len() > 1 && row.iter().all(|cell| cell == area)
                        })
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
