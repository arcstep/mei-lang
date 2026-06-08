use serde_json::Value;

use super::deep_merge_value;

pub(super) fn merge_panel_head_theme(theme: &Value) -> Value {
    let panel_head = theme_field(theme, "panel_head");
    let heading = theme_field(theme, "heading");
    deep_merge_value(&panel_head, &heading)
}

pub(super) fn theme_field(theme: &Value, key: &str) -> Value {
    theme
        .as_object()
        .and_then(|map| map.get(key))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
}

pub(super) fn builtin_theme(theme_id: &str) -> Option<Value> {
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
            },
            "shared": {}
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
            },
            "shared": {}
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
            },
            "shared": {}
        }),
    };
    Some(value)
}
