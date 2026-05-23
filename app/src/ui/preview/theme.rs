use mei_lang_kernel::{PanelDecl, SceneContract, ThemeDecl};
use serde_json::Value;

#[derive(Debug, Clone)]
pub(super) struct ThemeResolved {
    pub(super) id: String,
    pub(super) frame: Value,
    pub(super) panel: Value,
    pub(super) panel_bare: Value,
    pub(super) panel_head: Value,
    pub(super) panel_body: Value,
    /// 兼容：`theme.heading` 已合并进 `panel_head`（保留字段供调试/后续消费）。
    #[allow(dead_code)]
    pub(super) heading: Value,
    /// 合并后的 `theme.components`，供宿主组件通过 `_mei.components` 读取。
    pub(super) components: Value,
    pub(super) css_vars: Vec<(String, String)>,
}

pub(super) fn resolve_theme(scene_contract: &SceneContract) -> ThemeResolved {
    let mut theme_id = scene_contract
        .scene
        .theme
        .clone()
        .or_else(|| scene_contract.scene.profile.clone())
        .unwrap_or_else(|| "page".to_string());
    let mut theme = builtin_theme(theme_id.as_str());
    if theme.is_none() {
        theme_id = "page".to_string();
        theme = builtin_theme("page");
    }
    let mut theme = theme.unwrap_or_else(|| serde_json::json!({}));
    if let Some(custom) = scene_contract
        .themes
        .iter()
        .find(|item| item.id == theme_id)
        .or_else(|| scene_contract.themes.first())
    {
        theme = deep_merge_value(&theme, &theme_decl_value(custom));
        if theme_id != custom.id {
            theme_id = custom.id.clone();
        }
    }
    let frame = theme_field(&theme, "frame");
    let panel = theme_field(&theme, "panel");
    let panel_bare = theme_field(&theme, "panel_bare");
    let panel_head = merge_panel_head_theme(&theme);
    let panel_body = theme_field(&theme, "panel_body");
    let heading = theme_field(&theme, "heading");
    let css_vars = collect_theme_css_vars(&theme);
    let components = theme
        .as_object()
        .and_then(|map| map.get("components"))
        .cloned()
        .filter(|value| !value.is_null())
        .unwrap_or_else(|| serde_json::json!({}));
    ThemeResolved {
        id: theme_id,
        frame,
        panel,
        panel_bare,
        panel_head,
        panel_body,
        heading,
        components,
        css_vars,
    }
}

fn theme_field(theme: &Value, key: &str) -> Value {
    theme
        .as_object()
        .and_then(|map| map.get(key))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
}

fn merge_panel_head_theme(theme: &Value) -> Value {
    let panel_head = theme_field(theme, "panel_head");
    let heading = theme_field(theme, "heading");
    deep_merge_value(&panel_head, &heading)
}

fn builtin_theme(theme_id: &str) -> Option<Value> {
    let value = match theme_id {
        "cockpit" => serde_json::json!({
            "frame": {
                "background": {
                    "image": "radial-gradient(120% 80% at 50% -10%, rgba(14,165,233,.22), transparent 55%), radial-gradient(80% 50% at 100% 50%, rgba(59,130,246,.12), transparent 45%), linear-gradient(180deg, #050b14 0%, #0a1628 40%, #071018 100%)",
                    "position": "center",
                    "repeat": "no-repeat"
                },
                "border": "1px solid rgba(56,189,248,.18)",
                "radius": "8px",
                "overflow": "hidden",
                "padding": "0",
            },
            "panel": {
                "background": {
                    "color": "rgba(3,10,20,.76)",
                    "image": "radial-gradient(120% 100% at 0% 0%, rgba(34,211,238,.10), transparent 36%), radial-gradient(120% 100% at 100% 0%, rgba(59,130,246,.08), transparent 34%), linear-gradient(180deg, rgba(8,28,48,.92) 0%, rgba(4,16,30,.9) 58%, rgba(2,10,20,.94) 100%)",
                    "position": "center",
                    "size": "cover",
                    "repeat": "no-repeat"
                },
                "border": "1px solid rgba(56,189,248,.14)",
                "radius": "6px",
                "box_shadow": "inset 0 1px 0 rgba(125,211,252,.08), inset 0 0 0 1px rgba(15,23,42,.22), 0 10px 24px rgba(2,8,23,.24)",
                "padding": "0",
                "overflow": "hidden",
            },
            "panel_bare": {
                "show_heading": false,
                "background": "transparent",
                "border": "none",
                "radius": "0",
                "box_shadow": "none",
                "padding": "0",
                "overflow": "visible"
            },
            "panel_head": {
                "variant": "plain",
                "accent": false,
                "flair": false,
                "dots": false,
                "height": "44px",
                "align": "center"
            },
            "panel_body": {
                "min_height": "0"
            },
            "heading": {},
            "metric_label": {
                "font_family": "Microsoft YaHei, PingFang SC, sans-serif",
                "font_size": "16px",
                "color": "rgba(255,255,255,0.80)",
                "font_weight": "400",
                "text_align": "left",
                "line_height": "1.15"
            },
            "metric_value": {
                "font_family": "Microsoft YaHei Bold, Microsoft YaHei, PingFang SC, sans-serif",
                "font_size": "28px",
                "color": "rgba(255,255,255,0.80)",
                "font_weight": "700",
                "text_align": "right",
                "line_height": "1.05"
            },
            "metric_unit": {
                "font_family": "Microsoft YaHei, PingFang SC, sans-serif",
                "font_size": "16px",
                "color": "rgba(255,255,255,0.80)",
                "font_weight": "400",
                "text_align": "right",
                "line_height": "1.05"
            },
            "metric_sub_label": {
                "font_family": "Microsoft YaHei, PingFang SC, sans-serif",
                "font_size": "12px",
                "color": "rgba(255,255,255,0.80)",
                "font_weight": "400",
                "text_align": "left",
                "line_height": "1.05"
            },
            "metric_sub_value": {
                "font_family": "Microsoft YaHei Bold, Microsoft YaHei, PingFang SC, sans-serif",
                "font_size": "18px",
                "color": "rgba(255,255,255,0.80)",
                "font_weight": "700",
                "text_align": "right",
                "line_height": "1.05"
            },
            "metric_sub_unit": {
                "font_family": "Microsoft YaHei, PingFang SC, sans-serif",
                "font_size": "12px",
                "color": "rgba(255,255,255,0.80)",
                "font_weight": "400",
                "text_align": "right",
                "line_height": "1.05"
            },
            "font": {
                "1": "12px",
                "2": "14px",
                "3": "18px",
                "4": "24px"
            },
            "tokens": {
                "color": {
                    "text_primary": "#e0f2fe",
                    "text_muted": "#94a3b8",
                    "text_accent": "#fde68a"
                },
                "panel": {
                    "radius": "6px",
                    "padding": "12px"
                }
            },
            "components": {
                "dataset_table": {
                    "cell_preview_max_chars": 30
                }
            }
        }),
        "game" => serde_json::json!({
            "frame": {
                "background": {
                    "image": "linear-gradient(180deg, #111827 0%, #1f2937 100%)"
                },
                "padding": "0"
            },
            "panel": {
                "background": "rgba(17, 24, 39, 0.78)",
                "border": "1px solid rgba(148,163,184,.18)",
                "radius": "8px",
                "padding": "0",
                "overflow": "hidden"
            },
            "panel_bare": {
                "show_heading": false,
                "background": "transparent",
                "border": "none",
                "padding": "0",
                "overflow": "visible"
            },
            "panel_head": {
                "variant": "compact",
                "accent": true,
                "flair": false,
                "dots": false,
                "height": "40px",
                "align": "center"
            },
            "panel_body": {
                "min_height": "0"
            },
            "heading": {},
            "font": {
                "1": "12px",
                "2": "14px",
                "3": "17px",
                "4": "22px"
            },
            "tokens": {
                "color": {
                    "text_primary": "#f3f4f6",
                    "text_muted": "#9ca3af",
                    "text_accent": "#fbbf24"
                }
            },
            "components": {
                "dataset_table": {
                    "cell_preview_max_chars": 30
                }
            }
        }),
        _ => serde_json::json!({
            "frame": {
                "padding": "0"
            },
            "panel": {
                "background": "rgba(2,6,23,.32)",
                "border": "1px solid rgba(59,130,246,.18)",
                "radius": "14px",
                "padding": "12px"
            },
            "panel_bare": {
                "show_heading": false,
                "background": "transparent",
                "border": "none",
                "padding": "0",
                "overflow": "visible"
            },
            "panel_head": {
                "variant": "plain",
                "accent": false,
                "flair": false,
                "dots": false,
                "height": "40px",
                "align": "center"
            },
            "panel_body": {
                "min_height": "0"
            },
            "heading": {},
            "font": {
                "1": "12px",
                "2": "14px",
                "3": "16px",
                "4": "20px"
            },
            "tokens": {
                "color": {
                    "text_primary": "#e2e8f0",
                    "text_muted": "#94a3b8",
                    "text_accent": "#f8fafc"
                }
            },
            "components": {
                "dataset_table": {
                    "cell_preview_max_chars": 30
                }
            }
        }),
    };
    Some(value)
}

fn theme_decl_value(theme: &ThemeDecl) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("frame".to_string(), theme.frame.clone());
    map.insert("panel".to_string(), theme.panel.clone());
    map.insert("panel_bare".to_string(), theme.panel_bare.clone());
    map.insert("panel_head".to_string(), theme.panel_head.clone());
    map.insert("panel_body".to_string(), theme.panel_body.clone());
    map.insert("heading".to_string(), theme.heading.clone());
    map.insert("font".to_string(), theme.font.clone());
    map.insert("metric_label".to_string(), theme.metric_label.clone());
    map.insert("metric_value".to_string(), theme.metric_value.clone());
    map.insert("metric_unit".to_string(), theme.metric_unit.clone());
    map.insert("metric_desc".to_string(), theme.metric_desc.clone());
    map.insert(
        "metric_sub_label".to_string(),
        theme.metric_sub_label.clone(),
    );
    map.insert(
        "metric_sub_value".to_string(),
        theme.metric_sub_value.clone(),
    );
    map.insert("metric_sub_unit".to_string(), theme.metric_sub_unit.clone());
    map.insert("tokens".to_string(), theme.tokens.clone());
    if !theme.components.is_null() {
        map.insert("components".to_string(), theme.components.clone());
    }
    Value::Object(map)
}

/// 整卡 panel：theme.panel + `props`（剥离槽位键）。
pub(super) fn resolve_panel_card_props(theme: &ThemeResolved, panel: &PanelDecl) -> Value {
    let merged = resolve_panel_props(theme, &panel.props);
    strip_slot_keys_from_card_props(&merged)
}

pub(super) fn resolve_panel_props(theme: &ThemeResolved, props: &Value) -> Value {
    let use_bare = props
        .as_object()
        .and_then(|map| map.get("chrome"))
        .and_then(Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("bare"));
    if use_bare {
        deep_merge_value(&theme.panel_bare, props)
    } else {
        deep_merge_value(&theme.panel, props)
    }
}

pub(super) fn resolve_panel_head_props(theme: &ThemeResolved, panel: &PanelDecl) -> Value {
    deep_merge_value(&theme.panel_head, &panel.head_props)
}

pub(super) fn resolve_panel_body_props(theme: &ThemeResolved, panel: &PanelDecl) -> Value {
    deep_merge_value(&theme.panel_body, &panel.body_props)
}

fn strip_slot_keys_from_card_props(props: &Value) -> Value {
    let Some(map) = props.as_object() else {
        return props.clone();
    };
    let mut map = map.clone();
    map.remove("heading");
    Value::Object(map)
}

pub(super) fn deep_merge_value(base: &Value, overlay: &Value) -> Value {
    let (Some(base_obj), Some(overlay_obj)) = (base.as_object(), overlay.as_object()) else {
        return overlay.clone();
    };
    let mut merged = base_obj.clone();
    for (key, value) in overlay_obj {
        let next = if let Some(existing) = merged.get(key) {
            deep_merge_value(existing, value)
        } else {
            value.clone()
        };
        merged.insert(key.clone(), next);
    }
    Value::Object(merged)
}

fn collect_theme_css_vars(theme: &Value) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    if let Some(font) = theme
        .as_object()
        .and_then(|map| map.get("font"))
        .and_then(Value::as_object)
    {
        for (key, value) in font {
            if let Some(raw) = value.as_str() {
                vars.push((format!("--mei-font-{key}"), raw.to_string()));
            }
        }
    }
    for role in ["label", "value", "unit", "desc"] {
        let key = format!("metric_{role}");
        if let Some(entry) = theme.as_object().and_then(|map| map.get(key.as_str())) {
            push_typography_vars(entry, &format!("mei-metric-{role}"), &mut vars);
        }
    }
    for role in ["label", "value", "unit"] {
        let key = format!("metric_sub_{role}");
        if let Some(entry) = theme.as_object().and_then(|map| map.get(key.as_str())) {
            push_typography_vars(entry, &format!("mei-metric-sub-{role}"), &mut vars);
        }
    }
    if let Some(panel_head) = theme.as_object().and_then(|map| map.get("panel_head")) {
        push_typography_vars(panel_head, "mei-panel-head", &mut vars);
    }
    if let Some(tokens) = theme.as_object().and_then(|map| map.get("tokens")) {
        flatten_tokens(tokens, "mei", &mut vars);
    }
    vars
}

fn typography_css_suffix(key: &str) -> Option<&'static str> {
    match key {
        "font" | "font_size" => Some("font-size"),
        "font_family" => Some("font-family"),
        "color" => Some("color"),
        "font_weight" => Some("font-weight"),
        "letter_spacing" => Some("letter-spacing"),
        "text_align" | "align" => Some("text-align"),
        "line_height" => Some("line-height"),
        _ => None,
    }
}

fn resolve_font_size_value(raw: &str) -> String {
    let font_key = raw.trim();
    if font_key.is_empty() {
        return String::new();
    }
    if font_key.ends_with("px")
        || font_key.ends_with("rem")
        || font_key.ends_with("em")
        || font_key.ends_with('%')
    {
        font_key.to_string()
    } else {
        format!("var(--mei-font-{font_key}, 14px)")
    }
}

fn push_typography_vars(entry: &Value, var_prefix: &str, vars: &mut Vec<(String, String)>) {
    let Some(map) = entry.as_object() else {
        return;
    };
    for (key, value) in map {
        let Some(suffix) = typography_css_suffix(key) else {
            continue;
        };
        let resolved = match value {
            Value::String(raw) if !raw.trim().is_empty() => {
                if suffix == "font-size" {
                    resolve_font_size_value(raw)
                } else {
                    raw.trim().to_string()
                }
            }
            Value::Number(raw) if suffix == "font-size" => raw.to_string(),
            _ => continue,
        };
        if resolved.is_empty() {
            continue;
        }
        vars.push((format!("--{var_prefix}-{suffix}"), resolved));
    }
}

fn flatten_tokens(value: &Value, prefix: &str, vars: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, entry) in map {
                let path = format!("{prefix}-{}", key.replace('_', "-"));
                flatten_tokens(entry, path.as_str(), vars);
            }
        }
        Value::String(raw) if !raw.trim().is_empty() => {
            vars.push((format!("--{prefix}"), raw.to_string()));
        }
        Value::Number(raw) => {
            vars.push((format!("--{prefix}"), raw.to_string()));
        }
        Value::Bool(raw) => {
            vars.push((format!("--{prefix}"), raw.to_string()));
        }
        _ => {}
    }
}

pub(super) fn theme_css_vars_style(theme: &ThemeResolved) -> String {
    let mut style = String::new();
    style.push_str(&format!("--mei-theme-id:'{}';", theme.id));
    for (key, value) in &theme.css_vars {
        style.push_str(&format!("{key}:{value};"));
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::ThemeDecl;
    use serde_json::json;

    #[test]
    fn theme_decl_value_preserves_metric_role_fonts() {
        let decl = ThemeDecl {
            kind: "theme".to_string(),
            id: "cockpit".to_string(),
            frame: json!({}),
            panel: json!({}),
            panel_bare: json!({}),
            panel_head: json!({}),
            panel_body: json!({}),
            heading: json!({}),
            font: json!({"4": "20px"}),
            metric_label: json!({"font": "2"}),
            metric_value: json!({"font": "4"}),
            metric_unit: json!({"font": "1"}),
            metric_desc: json!({}),
            metric_sub_label: json!({"font_size": "12px"}),
            metric_sub_value: json!({"font_size": "18px", "text_align": "right"}),
            metric_sub_unit: json!({}),
            tokens: json!({}),
            components: json!({}),
        };
        let merged = theme_decl_value(&decl);
        let vars = collect_theme_css_vars(&merged);
        assert!(vars
            .iter()
            .any(|(k, v)| k == "--mei-metric-value-font-size" && v.contains("--mei-font-4")));
        assert!(vars
            .iter()
            .any(|(k, v)| k == "--mei-metric-sub-value-font-size" && v == "18px"));
        assert!(vars
            .iter()
            .any(|(k, v)| k == "--mei-metric-sub-value-text-align" && v == "right"));
    }
}
