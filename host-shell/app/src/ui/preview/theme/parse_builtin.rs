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

fn page_shell_tokens() -> Value {
    serde_json::json!({
        "bg": "radial-gradient(circle at top, #1a2a41 0%, #10192a 28%, #080d16 68%, #04070d 100%)",
        "text": "#dbe8f6",
        "stage": "linear-gradient(180deg, rgba(20, 31, 47, 0.56), rgba(8, 13, 21, 0.26))",
        "stage_border": "rgba(124, 145, 173, 0.12)",
        "chrome_top_bg": "linear-gradient(180deg, rgba(18, 32, 51, 0.97), rgba(9, 18, 30, 0.97))",
        "chrome_bottom_bg": "linear-gradient(180deg, rgba(8, 15, 25, 0.97), rgba(5, 10, 18, 0.98))",
        "chrome_border_top": "rgba(96, 165, 250, 0.24)",
        "chrome_border_bottom": "rgba(45, 212, 191, 0.22)",
        "family_ui": "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif"
    })
}

fn page_shell_color_tokens() -> Value {
    serde_json::json!({
        "accent": "#fb7185",
        "accent_muted": "#fda4af",
        "banner_bg": "rgba(15, 23, 42, 0.96)",
        "banner_danger_border": "rgba(248, 113, 113, 0.35)",
        "banner_warn_border": "rgba(251, 191, 36, 0.35)",
        "border_accent_blue": "rgba(96,165,250,.24)",
        "border_accent_soft": "rgba(96, 165, 250, 0.18)",
        "border_accent_teal": "rgba(45,212,191,.34)",
        "border_default": "rgba(96, 165, 250, 0.16)",
        "border_faint": "rgba(100, 116, 139, 0.18)",
        "border_inspector": "rgba(118, 148, 188, 0.28)",
        "border_metadata": "rgba(110, 129, 154, 0.28)",
        "border_muted": "rgba(100, 116, 139, 0.2)",
        "border_nav": "rgba(100, 116, 139, 0.32)",
        "border_panel": "rgba(100, 116, 139, 0.14)",
        "border_slate": "rgba(148, 163, 184, 0.22)",
        "border_slate_soft": "rgba(148, 163, 184, 0.28)",
        "border_strong": "rgba(100, 116, 139, 0.28)",
        "border_tool": "rgba(112, 135, 164, 0.26)",
        "btn_border": "rgba(51, 65, 85, 0.55)",
        "btn_primary_bg": "linear-gradient(180deg, #38bdf8 0%, #0ea5e9 100%)",
        "btn_primary_solid": "#0ea5e9",
        "btn_primary_text": "#041320",
        "btn_secondary_bg": "rgba(15, 23, 42, 0.45)",
        "card_bg": "rgba(17, 26, 44, 0.6)",
        "chip_file_icon_bg": "linear-gradient(180deg, rgba(226, 232, 240, 0.42), rgba(148, 163, 184, 0.3))",
        "chrome_divider": "rgba(148,163,184,.16)",
        "code": "#fde68a",
        "feedback_ok": "#86efac",
        "focus_border": "rgba(14, 165, 233, 0.55)",
        "focus_ring": "rgba(14, 165, 233, 0.12)",
        "focus_ring_strong": "rgba(14, 165, 233, 0.45)",
        "glow_danger": "rgba(248, 113, 113, 0.22)",
        "glow_indigo": "rgba(129, 140, 248, 0.18)",
        "glow_teal": "rgba(45, 212, 191, 0.16)",
        "glow_warn": "rgba(251, 191, 36, 0.2)",
        "hint_bg": "rgba(15, 23, 42, 0.35)",
        "hint_border": "rgba(51, 65, 85, 0.45)",
        "host_footer_text": "#64748b",
        "host_page_bg": "linear-gradient(180deg, #0b1220 0%, #0a1628 48%, #070d18 100%)",
        "host_splash_glow": "rgba(251, 113, 133, 0.12)",
        "input_bg": "rgba(15, 23, 42, 0.55)",
        "input_border": "rgba(51, 65, 85, 0.65)",
        "inset_highlight": "rgba(226, 232, 240, 0.06)",
        "inset_highlight_subtle": "rgba(148, 163, 184, 0.05)",
        "link": "#7dd3fc",
        "manage_panel_bg": "linear-gradient(180deg, rgba(12, 18, 31, 0.92), rgba(2, 6, 23, 0.76))",
        "mode_tab_active_bg": "rgba(37, 99, 235, 0.32)",
        "mode_tab_hover_bg": "rgba(30, 58, 138, 0.28)",
        "nav_chip_active_bg": "rgba(44, 66, 98, 0.92)",
        "nav_chip_active_border": "rgba(125, 164, 216, 0.58)",
        "nav_chip_bg": "rgba(28, 42, 63, 0.84)",
        "panel_bg": "rgba(2, 6, 23, 0.38)",
        "preview_bounds_line": "rgba(148, 163, 184, 0.58)",
        "preview_bounds_line_soft": "rgba(100, 116, 139, 0.38)",
        "preview_excluded_stripe_a": "rgba(255, 255, 255, 0.09)",
        "preview_excluded_stripe_b": "rgba(0, 0, 0, 0.22)",
        "progress_bar": "linear-gradient(90deg, #38bdf8, #60a5fa, #34d399)",
        "shadow_banner": "rgba(2, 8, 23, 0.45)",
        "shadow_card": "rgba(2, 8, 23, 0.35)",
        "shadow_deep": "rgba(2, 6, 23, 0.45)",
        "shadow_overlay_lg": "rgba(2,6,23,.45)",
        "splitter_line": "rgba(148, 163, 184, 0.42)",
        "splitter_rail_grad": "linear-gradient(180deg, transparent 0 2px, rgba(148, 163, 184, 0.3) 2px 3px, transparent 3px 5px, rgba(148, 163, 184, 0.3) 5px 6px, transparent 6px 8px)",
        "status_warn": "#fbbf24",
        "tab_border_bottom": "rgba(94, 108, 130, 0.26)",
        "text_body": "#cbd5e1",
        "text_inverse": "#f8fafc",
        "text_muted": "#94a3b8",
        "text_primary": "#e2e8f0",
        "watermark": "rgba(251, 113, 133, 0.11)",
    })
}

pub(super) fn builtin_theme(theme_id: &str) -> Option<Value> {
    let value = match theme_id {
        "cockpit" => serde_json::json!({
            "frame": {
                "background": {
                    "image": "frame_cockpit",
                    "position": "center",
                    "repeat": "no-repeat"
                },
                "border": "1px solid border_accent",
                "radius": "0",
                "overflow": "hidden",
                "padding": "0",
            },
            "panel": {
                "background": {
                    "color": "surface_panel",
                    "image": "panel_cockpit",
                    "position": "center",
                    "size": "cover",
                    "repeat": "no-repeat"
                },
                "border": "1px solid border_panel",
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
                "align": "center",
                "color": "panel_title",
                "font": "4",
                "font_weight": "medium"
            },
            "panel_body": {
                "min_height": "0"
            },
            "heading": {},
            "header_title": {
                "font": "5",
                "color": "text_primary",
                "font_weight": "bold"
            },
            "body": {
                "font": "2",
                "color": "text_body",
                "font_weight": "regular"
            },
            "muted": {
                "font": "1",
                "color": "text_muted",
                "font_weight": "regular"
            },
            "metric_label": {
                "font": "2",
                "color": "text_value",
                "font_weight": "regular",
                "text_align": "left",
                "line_height": "1.15"
            },
            "metric_value": {
                "font": "4",
                "color": "text_value",
                "font_weight": "bold",
                "text_align": "right",
                "line_height": "1.05"
            },
            "metric_unit": {
                "font": "2",
                "color": "text_value",
                "font_weight": "regular",
                "text_align": "right",
                "line_height": "1.05"
            },
            "metric_desc": {
                "font": "1",
                "color": "text_muted",
                "font_weight": "regular"
            },
            "metric_sub_label": {
                "font": "1",
                "color": "text_value",
                "font_weight": "regular",
                "text_align": "left",
                "line_height": "1.05"
            },
            "metric_sub_value": {
                "font": "3",
                "color": "text_value",
                "font_weight": "bold",
                "text_align": "right",
                "line_height": "1.05"
            },
            "metric_sub_unit": {
                "font": "1",
                "color": "text_value",
                "font_weight": "regular",
                "text_align": "right",
                "line_height": "1.05"
            },
            "chart_title": {
                "font": "2",
                "color": "text_primary",
                "font_weight": "medium"
            },
            "chart_label": {
                "font": "1",
                "color": "text_muted",
                "font_weight": "regular"
            },
            "table_head": {
                "font": "2",
                "color": "text_primary",
                "font_weight": "medium"
            },
            "table_body": {
                "font": "2",
                "color": "text_body",
                "font_weight": "regular"
            },
            "filter_panel": {
                "font": "2",
                "color": "text_body",
                "font_weight": "regular"
            },
            "font": {
                "1": "12px",
                "2": "14px",
                "3": "18px",
                "4": "24px",
                "5": "32px"
            },
            "tokens": {
                "color": {
                    "text_primary": "#e0f2fe",
                    "text_muted": "#94a3b8",
                    "text_accent": "#fde68a",
                    "text_value": "rgba(255,255,255,0.80)",
                    "text_unit": "#7dd3fc",
                    "text_body": "#cbd5e1",
                    "text_inverse": "#f8fafc",
                    "panel_title": "#ecfeff",
                    "section_border": "rgba(52, 82, 108, 0.5)",
                    "surface_bg": "rgb(29, 47, 65)",
                    "surface_panel": "rgba(3,10,20,.76)",
                    "border_default": "rgba(56,189,248,.18)",
                    "border_accent": "rgba(56,189,248,.18)",
                    "border_panel": "rgba(56,189,248,.14)",
                    "chart_1": "#d1fae5",
                    "chart_2": "#a7f3d0",
                    "chart_3": "#6ee7b7",
                    "chart_4": "#34d399",
                    "chart_5": "#10b981",
                    "chart_6": "#059669"
                },
                "gradient": {
                    "frame_cockpit": "radial-gradient(120% 80% at 50% -10%, rgba(14,165,233,.22), transparent 55%), radial-gradient(80% 50% at 100% 50%, rgba(59,130,246,.12), transparent 45%), linear-gradient(180deg, #050b14 0%, #0a1628 40%, #071018 100%)",
                    "panel_cockpit": "radial-gradient(120% 100% at 0% 0%, rgba(34,211,238,.10), transparent 36%), radial-gradient(120% 100% at 100% 0%, rgba(59,130,246,.08), transparent 34%), linear-gradient(180deg, rgba(8,28,48,.92) 0%, rgba(4,16,30,.9) 58%, rgba(2,10,20,.94) 100%)"
                },
                "shadow": {
                    "header_title": "0 20px 30px #0091ff, 0 0 4px #0d74c2",
                    "panel_title": "0 0 10px rgba(0, 145, 255, 0.55), 0 0 2px rgba(13, 116, 194, 0.9)"
                },
                "shell": page_shell_tokens(),
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
                    "image": "frame_game"
                },
                "padding": "0"
            },
            "panel": {
                "background": {
                    "color": "surface_panel"
                },
                "border": "1px solid border_default",
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
                "align": "center",
                "color": "text_primary",
                "font": "3",
                "font_weight": "medium"
            },
            "panel_body": {
                "min_height": "0"
            },
            "heading": {},
            "header_title": {
                "font": "4",
                "color": "text_primary",
                "font_weight": "bold"
            },
            "body": {
                "font": "2",
                "color": "text_body",
                "font_weight": "regular"
            },
            "muted": {
                "font": "1",
                "color": "text_muted",
                "font_weight": "regular"
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
                    "text_body": "#d1d5db",
                    "text_inverse": "#f9fafb",
                    "text_accent": "#fbbf24",
                    "surface_bg": "rgba(17, 24, 39, 0.78)",
                    "surface_panel": "rgba(17, 24, 39, 0.78)",
                    "border_default": "rgba(148,163,184,.18)"
                },
                "gradient": {
                    "frame_game": "linear-gradient(180deg, #111827 0%, #1f2937 100%)"
                },
                "shell": page_shell_tokens()
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
                "background": {
                    "color": "surface_bg"
                },
                "border": "1px solid border_default",
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
                "align": "center",
                "color": "text_primary",
                "font": "3",
                "font_weight": "medium"
            },
            "panel_body": {
                "min_height": "0"
            },
            "heading": {},
            "header_title": {
                "font": "4",
                "color": "text_primary",
                "font_weight": "bold"
            },
            "body": {
                "font": "2",
                "color": "text_body",
                "font_weight": "regular"
            },
            "muted": {
                "font": "1",
                "color": "text_muted",
                "font_weight": "regular"
            },
            "font": {
                "1": "16px",
                "2": "14px",
                "3": "18px",
                "4": "20px"
            },
            "tokens": {
                "color": page_shell_color_tokens(),
                "shell": page_shell_tokens()
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
