use mei_lang_kernel::{SceneContract, ThemeDecl};
use serde_json::Value;

#[derive(Debug, Clone)]
pub(super) struct ThemeResolved {
    pub(super) id: String,
    pub(super) frame: Value,
    pub(super) panel: Value,
    pub(super) panel_bare: Value,
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
    let frame = theme
        .as_object()
        .and_then(|map| map.get("frame"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let panel = theme
        .as_object()
        .and_then(|map| map.get("panel"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let panel_bare = theme
        .as_object()
        .and_then(|map| map.get("panel_bare"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let heading = theme
        .as_object()
        .and_then(|map| map.get("heading"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
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
        heading,
        components,
        css_vars,
    }
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
            "heading": {
                "variant": "screen",
                "accent": true,
                "flair": true,
                "dots": true
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
            "heading": {
                "variant": "compact",
                "accent": true
            },
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
            "heading": {
                "variant": "default",
                "accent": true
            },
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
    map.insert("heading".to_string(), theme.heading.clone());
    map.insert("font".to_string(), theme.font.clone());
    map.insert("tokens".to_string(), theme.tokens.clone());
    if !theme.components.is_null() {
        map.insert("components".to_string(), theme.components.clone());
    }
    Value::Object(map)
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
    if let Some(tokens) = theme.as_object().and_then(|map| map.get("tokens")) {
        flatten_tokens(tokens, "mei", &mut vars);
    }
    vars
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
