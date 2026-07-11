//! Compose-time panel chrome export (`head_chrome` / `panel_shell`) with theme token resolution.

use std::path::Path;

use mei_lang_kernel::{
    is_font_scale_key, is_literal_color, is_literal_gradient, load_mei_config_for_app,
    resolve_app_root, CompiledApp, UiNodeDecl,
};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone)]
pub struct ThemeResolveContext {
    theme: Value,
}

impl ThemeResolveContext {
    pub fn new(theme: Value) -> Self {
        Self { theme }
    }

    pub fn from_compiled(workspace_root: &Path, compiled: &CompiledApp) -> Option<Self> {
        let app_root = resolve_app_root(workspace_root, compiled.app_id.as_str());
        let mei_config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
        let theme_id = compiled
            .scene_contract
            .as_ref()
            .and_then(|contract| contract.scene.theme.clone())
            .unwrap_or_else(|| "cockpit".to_string());
        mei_config.ops.themes.get(&theme_id).cloned().map(Self::new)
    }

    fn theme_object(&self) -> Option<&serde_json::Map<String, Value>> {
        self.theme.as_object()
    }

    fn token_leaf(&self, group: &str, key: &str) -> Option<String> {
        let value = self
            .theme_object()?
            .get("tokens")?
            .as_object()?
            .get(group)?
            .as_object()?
            .get(key)?;
        value
            .as_str()
            .map(str::to_string)
            .or_else(|| value.as_f64().map(|n| n.to_string()))
    }

    pub fn resolve_color_literal(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        if trimmed.eq_ignore_ascii_case("transparent") {
            return "transparent".to_string();
        }
        if is_literal_color(trimmed) {
            return trimmed.to_string();
        }
        self.token_leaf("color", trimmed)
            .unwrap_or_else(|| trimmed.to_string())
    }

    pub fn resolve_gradient_literal(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        if is_literal_gradient(trimmed) || is_literal_color(trimmed) {
            return trimmed.to_string();
        }
        if trimmed.starts_with('/') || trimmed.starts_with("url(") {
            return if trimmed.starts_with("url(") {
                trimmed.to_string()
            } else {
                format!("url(\"{trimmed}\")")
            };
        }
        self.token_leaf("gradient", trimmed)
            .unwrap_or_else(|| trimmed.to_string())
    }

    pub fn resolve_font_literal(&self, raw: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        if trimmed.ends_with("px") || trimmed.ends_with("rem") || trimmed.ends_with("em") {
            return trimmed.to_string();
        }
        if is_font_scale_key(trimmed) {
            return self
                .theme_object()
                .and_then(|map| map.get("font"))
                .and_then(Value::as_object)
                .and_then(|font| font.get(trimmed))
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{trimmed}px"));
        }
        trimmed.to_string()
    }
}

fn deep_merge_json(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            let mut merged = base_map.clone();
            for (key, value) in overlay_map {
                let entry = merged
                    .remove(key)
                    .map(|existing| deep_merge_json(&existing, value))
                    .unwrap_or_else(|| value.clone());
                merged.insert(key.clone(), entry);
            }
            Value::Object(merged)
        }
        (_, overlay) => overlay.clone(),
    }
}

fn heading_chrome_props(head_props: &Value) -> Value {
    let Some(map) = head_props.as_object() else {
        return head_props.clone();
    };
    let Some(chrome) = map.get("chrome").filter(|value| value.is_object()) else {
        return head_props.clone();
    };
    deep_merge_json(chrome, head_props)
}

fn normalize_css_length(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.ends_with("px")
        || trimmed.ends_with('%')
        || trimmed.ends_with("rem")
        || trimmed.ends_with("em")
        || trimmed.eq_ignore_ascii_case("auto")
    {
        return trimmed.to_string();
    }
    if trimmed.parse::<f64>().is_ok() {
        return format!("{trimmed}px");
    }
    trimmed.to_string()
}

fn append_background_style(style: &mut String, background: &Value, ctx: &ThemeResolveContext) {
    match background {
        Value::String(value) if !value.trim().is_empty() => {
            let trimmed = value.trim();
            if trimmed.eq_ignore_ascii_case("transparent") {
                style.push_str("background:transparent;");
            } else {
                style.push_str(&format!(
                    "background:{};",
                    ctx.resolve_gradient_literal(trimmed)
                ));
            }
        }
        Value::Object(bg) => {
            if let Some(value) = bg.get("color").and_then(Value::as_str) {
                style.push_str(&format!(
                    "background-color:{};",
                    ctx.resolve_color_literal(value)
                ));
            }
            if let Some(value) = bg.get("image").and_then(Value::as_str) {
                style.push_str(&format!(
                    "background-image:{};",
                    ctx.resolve_gradient_literal(value)
                ));
            }
            for (key, css) in [
                ("size", "background-size"),
                ("position", "background-position"),
                ("repeat", "background-repeat"),
                ("attachment", "background-attachment"),
            ] {
                if let Some(value) = bg.get(key).and_then(Value::as_str) {
                    style.push_str(&format!("{css}:{value};"));
                }
            }
        }
        _ => {}
    }
}

fn panel_head_caret_style(head_props: &Value) -> (bool, String) {
    let Some(carets) = head_props.as_object().and_then(|map| map.get("carets")) else {
        return (false, String::new());
    };
    let Some(map) = carets.as_object() else {
        return (false, String::new());
    };
    let Some(url) = map.get("url").and_then(Value::as_str).map(str::trim) else {
        return (false, String::new());
    };
    if url.is_empty() {
        return (false, String::new());
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
    let url_css = if url.starts_with("url(") {
        url.to_string()
    } else {
        format!("url(\"{url}\")")
    };
    let mut style = format!(
        "--mei-head-caret-url:{url_css};--mei-head-caret-inset:{inset};--mei-head-caret-left-rotate:{left_rotate};--mei-head-caret-size:{size};"
    );
    if let Some(left) = map
        .get("left")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        style.push_str(&format!("--mei-head-caret-left-pos:{left};"));
    }
    if let Some(right) = map
        .get("right")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
    {
        style.push_str(&format!("--mei-head-caret-right-pos:{right};"));
    }
    (true, style)
}

fn heading_variant(head_props: &Value) -> String {
    head_props
        .as_object()
        .and_then(|map| map.get("heading_variant"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "plain".to_string())
}

fn heading_classes(variant: &str, compact: bool) -> Vec<String> {
    let mut classes = vec![
        "panel-heading".to_string(),
        format!("panel-heading-{variant}"),
    ];
    if compact {
        classes.push("panel-heading-compact".to_string());
    }
    classes
}

fn panel_title(panel: &UiNodeDecl) -> String {
    panel
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            panel
                .props
                .get("__mei_section_title")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
}

pub fn section_id_for_head_scope(preview_scope: &str) -> Option<String> {
    let normalized = preview_scope.trim().trim_end_matches("/mei.text");
    // Authoring uses area `title` (gold-case); `title_zone` / `head` remain legacy aliases.
    // Projected section title blocks are often `.../title/title`.
    let without_head = normalized
        .strip_suffix("/title/title")
        .or_else(|| normalized.strip_suffix("/title_zone"))
        .or_else(|| normalized.strip_suffix("/head"))
        .or_else(|| normalized.strip_suffix("/title"))?;
    let section_id = without_head.rsplit('/').next()?.trim();
    if section_id.is_empty() {
        None
    } else {
        Some(section_id.to_string())
    }
}

pub fn build_head_chrome(panel: &UiNodeDecl, ctx: &ThemeResolveContext) -> Value {
    let head_props = &panel.head_props;
    if head_props.as_object().is_none_or(|map| map.is_empty()) {
        return Value::Null;
    }
    let chrome_props = heading_chrome_props(head_props);
    let panel_head = ctx
        .theme_object()
        .and_then(|map| map.get("panel_head"))
        .cloned()
        .unwrap_or(Value::Null);
    let heading_props = deep_merge_json(&panel_head, &chrome_props);

    let variant = heading_variant(head_props);
    let compact = panel
        .props
        .get("__mei_padding_profile")
        .and_then(Value::as_str)
        .is_some_and(|profile| profile.contains("compact") || profile.contains("dense"));
    let classes = heading_classes(variant.as_str(), compact);

    let mut cell_style = String::new();
    if let Some(map) = chrome_props.as_object() {
        if let Some(value) = map.get("height").and_then(Value::as_str) {
            let px = normalize_css_length(value);
            cell_style.push_str(&format!("height:{px};min-height:{px};"));
        }
        if map
            .get("align")
            .and_then(Value::as_str)
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("center"))
        {
            cell_style.push_str(
                "display:flex;align-items:center;justify-content:center;padding:0;box-sizing:border-box;overflow:hidden;width:100%;",
            );
        }
        if let Some(background) = map.get("background") {
            append_background_style(&mut cell_style, background, ctx);
        }
    }

    let (caret_enabled, caret_style) = panel_head_caret_style(head_props);
    let heading_map = heading_props
        .as_object()
        .and_then(|map| map.get("heading"))
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| heading_props.as_object().cloned());

    let mut typography = Map::new();
    if let Some(map) = heading_map {
        if let Some(font) = map.get("font").and_then(Value::as_str) {
            typography.insert(
                "font_size".to_string(),
                Value::String(ctx.resolve_font_literal(font)),
            );
        }
        if let Some(font_size) = map.get("font_size").and_then(Value::as_str) {
            typography.insert(
                "font_size".to_string(),
                Value::String(ctx.resolve_font_literal(font_size)),
            );
        }
        if let Some(color) = map.get("color").and_then(Value::as_str) {
            typography.insert(
                "color".to_string(),
                Value::String(ctx.resolve_color_literal(color)),
            );
        }
        if let Some(family) = map.get("font_family").and_then(Value::as_str) {
            typography.insert("font_family".to_string(), Value::String(family.to_string()));
        }
        if let Some(weight) = map.get("font_weight").and_then(Value::as_str) {
            typography.insert("font_weight".to_string(), Value::String(weight.to_string()));
        } else if let Some(weight) = map.get("font_weight").and_then(Value::as_u64) {
            typography.insert("font_weight".to_string(), Value::Number(weight.into()));
        }
        if let Some(spacing) = map.get("letter_spacing").and_then(Value::as_str) {
            typography.insert(
                "letter_spacing".to_string(),
                Value::String(spacing.to_string()),
            );
        }
    }

    json!({
        "title": panel_title(panel),
        "heading_variant": variant,
        "heading_classes": classes,
        "cell_style": cell_style,
        "caret": {
            "enabled": caret_enabled,
            "mode": if caret_enabled { "slot" } else { "" },
            "style": caret_style,
        },
        "heading_typography": Value::Object(typography),
    })
}

fn panel_is_metric_card(panel: &UiNodeDecl) -> bool {
    panel
        .props
        .get("__mei_metric_card")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn panel_props_background(props: &Value) -> Option<&Value> {
    if let Some(background) = props.get("background") {
        return Some(background);
    }
    // Unresolved `a | b` Merge keeps background under left until shell export.
    if props.get("__binop").and_then(Value::as_str) == Some("Merge") {
        return props.pointer("/left/background");
    }
    None
}

fn flatten_panel_props_merge(props: &Value) -> Value {
    if props.get("__binop").and_then(Value::as_str) != Some("Merge") {
        return props.clone();
    }
    let mut merged = props.get("left").cloned().unwrap_or_else(|| json!({}));
    if let Some(right) = props.get("right") {
        if let (Some(base), Some(overlay)) = (merged.as_object_mut(), right.as_object()) {
            for (key, value) in overlay {
                base.insert(key.clone(), value.clone());
            }
        }
    }
    merged
}

pub fn should_export_panel_shell(panel: &UiNodeDecl) -> bool {
    if panel_is_metric_card(panel) {
        return false;
    }
    if panel
        .props
        .get("__mei_slot_frame_bg")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || panel
            .props
            .pointer("/right/__mei_slot_frame_bg")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || panel
            .props
            .pointer("/left/__mei_slot_frame_bg")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return true;
    }
    panel_props_background(&panel.props).is_some()
        || panel
            .props
            .get("__mei_layout_fill")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || panel
            .body_props
            .as_object()
            .is_some_and(|map| !map.is_empty())
}

pub fn build_panel_shell(panel: &UiNodeDecl, ctx: &ThemeResolveContext) -> Value {
    let flattened = flatten_panel_props_merge(&panel.props);
    let mut props = flattened.as_object().cloned().unwrap_or_default();
    if let Some(body) = panel.body_props.as_object() {
        for (key, value) in body {
            props.insert(key.clone(), value.clone());
        }
    }
    if let Some(background) = props.get("background").cloned() {
        let resolved = match background {
            Value::String(raw) => Value::String(ctx.resolve_gradient_literal(raw.as_str())),
            Value::Object(mut map) => {
                if let Some(image) = map.get("image").and_then(Value::as_str) {
                    map.insert(
                        "image".to_string(),
                        Value::String(ctx.resolve_gradient_literal(image)),
                    );
                }
                if let Some(color) = map.get("color").and_then(Value::as_str) {
                    map.insert(
                        "color".to_string(),
                        Value::String(ctx.resolve_color_literal(color)),
                    );
                }
                Value::Object(map)
            }
            other => other,
        };
        props.insert("background".to_string(), resolved);
    }
    json!({
        "mount_role": "panel-shell",
        "props": Value::Object(props),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cockpit_theme() -> Value {
        json!({
            "font": { "4": "32px", "2": "18px" },
            "panel_head": { "color": "panel_title", "font": "4" },
            "tokens": {
                "color": { "panel_title": "#ffffff" },
                "gradient": {
                    "panel_title_bar": "linear-gradient(90deg, #245284 0%, #105796 50%, #163349 100%)",
                    "panel_glow_bg": "linear-gradient(180deg, rgba(32, 96, 168, 0.72) 0%, rgba(14, 48, 92, 0.88) 100%)"
                }
            }
        })
    }

    fn warning_section_panel() -> UiNodeDecl {
        UiNodeDecl {
            kind: "panel".to_string(),
            id: "warning".to_string(),
            title: Some("监督预警".to_string()),
            head: None,
            area: Some("warning".to_string()),
            layout: None,
            blocks: vec![],
            slot: None,
            props: json!({"__mei_padding_profile": "compact"}),
            head_props: json!({
                "heading_variant": "plain",
                "heading": {
                    "font_family": "Microsoft YaHei Bold, Microsoft YaHei, PingFang SC, sans-serif",
                    "font": "4",
                    "font_weight": "700",
                    "letter_spacing": "20px",
                    "color": "panel_title"
                },
                "background": {
                    "image": "panel_title_bar",
                    "position": "center",
                    "size": "100% 100%",
                    "repeat": "no-repeat"
                },
                "carets": {
                    "url": "/workspace-app-assets/templates/cockpit/assets/panel/caret-left-filled@3x.svg",
                    "left": "26.2%",
                    "right": "71.2%",
                    "left_rotate": "180deg",
                    "size": "14px 24px"
                },
                "height": "54px",
                "align": "center"
            }),
            body_props: json!({}),
            base: None,
            import_scope: None,
        }
    }

    #[test]
    fn build_head_chrome_resolves_panel_title_bar_and_carets() {
        let ctx = ThemeResolveContext::new(cockpit_theme());
        let chrome = build_head_chrome(&warning_section_panel(), &ctx);
        assert_eq!(chrome["title"], "监督预警");
        assert_eq!(chrome["heading_variant"], "plain");
        let cell_style = chrome["cell_style"].as_str().unwrap_or("");
        assert!(cell_style.contains("linear-gradient(90deg, #245284"));
        assert!(cell_style.contains("height:54px"));
        assert_eq!(chrome["caret"]["enabled"], true);
        assert_eq!(chrome["heading_typography"]["font_size"], "32px");
        assert_eq!(chrome["heading_typography"]["color"], "#ffffff");
    }

    #[test]
    fn should_export_panel_shell_detects_merge_slot_frame_flag() {
        let panel = UiNodeDecl {
            kind: "panel".to_string(),
            id: "supervision_triptych_first".to_string(),
            title: None,
            head: None,
            area: Some("first".to_string()),
            layout: None,
            blocks: vec![],
            slot: None,
            props: json!({
                "__binop": "Merge",
                "left": {
                    "background": "linear-gradient(#71F1EA,#71F1EA) left top / 4px 2px no-repeat,rgba(98,190,235,0.10)",
                    "padding": "0",
                    "chrome": "bare"
                },
                "right": {
                    "__mei_slot_frame_bg": true
                }
            }),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        assert!(should_export_panel_shell(&panel));
        let ctx = ThemeResolveContext::new(cockpit_theme());
        let shell = build_panel_shell(&panel, &ctx);
        assert_eq!(shell["props"]["__mei_slot_frame_bg"], true);
        let bg = shell["props"]["background"].as_str().unwrap_or("");
        assert!(bg.contains("#71F1EA"), "expected corner decor, got {bg}");
    }

    #[test]
    fn build_panel_shell_resolves_panel_glow_bg() {
        let ctx = ThemeResolveContext::new(cockpit_theme());
        let panel = UiNodeDecl {
            kind: "panel".to_string(),
            id: "supervision-stats".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![],
            slot: None,
            props: json!({
                "background": "panel_glow_bg",
                "width": "100%"
            }),
            head_props: json!({}),
            body_props: json!({"padding": "8px"}),
            base: None,
            import_scope: None,
        };
        let shell = build_panel_shell(&panel, &ctx);
        assert_eq!(shell["mount_role"], "panel-shell");
        let bg = shell["props"]["background"].as_str().unwrap_or("");
        assert!(bg.contains("rgba(32, 96, 168"));
    }

    #[test]
    fn section_id_for_head_scope_parses_warning() {
        assert_eq!(
            section_id_for_head_scope("t1/right_rail/warning/head").as_deref(),
            Some("warning")
        );
        assert_eq!(
            section_id_for_head_scope("t1/right_rail/warning/title_zone").as_deref(),
            Some("warning")
        );
        assert_eq!(
            section_id_for_head_scope("t1/right_rail/warning/title_zone/mei.text").as_deref(),
            Some("warning")
        );
        assert_eq!(
            section_id_for_head_scope("t1/main/enforcement/title").as_deref(),
            Some("enforcement")
        );
        assert_eq!(
            section_id_for_head_scope("t1/main/enforcement/title/title").as_deref(),
            Some("enforcement")
        );
    }
}
