use super::super::*;

    use crate::model::ThemeDecl;
    use serde_json::json;

    #[test]
    fn rejects_literal_color_in_panel_props() {
        let panel = PanelDecl {
            kind: "panel".to_string(),
            id: "p1".to_string(),
            title: None,
            head: None,
            area: None,
            layout: None,
            blocks: vec![],
            slot: None,
            props: json!({ "color": "#fff" }),
            head_props: json!({}),
            body_props: json!({}),
            base: None,
            import_scope: None,
        };
        let mut diagnostics = Vec::new();
        validate_panel_token_refs(&panel, "test.mei", &mut diagnostics);
        assert!(diagnostics
            .iter()
            .any(|d| d.code == "literal_color_forbidden"));
    }

    #[test]
    fn allows_literal_color_in_tokens() {
        let theme = ThemeDecl {
            kind: "theme".to_string(),
            id: "cockpit".to_string(),
            frame: json!({}),
            panel: json!({}),
            panel_bare: json!({}),
            panel_head: json!({}),
            panel_body: json!({}),
            heading: json!({}),
            font: json!({"1": "12px"}),
            metric_label: json!({"color": "text_primary", "font": "2"}),
            metric_value: json!({}),
            metric_unit: json!({}),
            metric_desc: json!({}),
            metric_sub_label: json!({}),
            metric_sub_value: json!({}),
            metric_sub_unit: json!({}),
            chart_title: json!({}),
            chart_label: json!({}),
            table_head: json!({}),
            table_body: json!({}),
            filter_panel: json!({}),
            tokens: json!({"color": {"text_primary": "#e0f2fe"}}),
            shared: json!({}),
            components: json!({}),
        };
        let mut diagnostics = Vec::new();
        validate_theme_decl(&theme, "test.mei", &mut diagnostics);
        assert!(!diagnostics
            .iter()
            .any(|d| d.code == "literal_color_forbidden"));
    }

    #[test]
    fn validate_shell_theme_requires_shell_and_color_keys() {
        let theme = json!({
            "font": {"1": "11px", "2": "13px", "3": "15px", "4": "18px"},
            "tokens": {
                "shell": {
                    "bg": "#000",
                    "text": "#fff",
                    "stage": "none",
                    "stage_border": "none",
                    "chrome_top_bg": "none",
                    "chrome_bottom_bg": "none",
                    "chrome_border_top": "none",
                    "chrome_border_bottom": "none",
                    "family_ui": "sans-serif"
                },
                "color": {
                    "text_primary": "#eee",
                    "text_muted": "#aaa",
                    "text_body": "#ccc",
                    "text_inverse": "#fff",
                    "panel_bg": "rgba(0,0,0,.5)",
                    "border_default": "rgba(0,0,0,.2)"
                }
            }
        });
        let mut diagnostics = Vec::new();
        validate_shell_theme_value("host", &theme, ".mei-workspace.json", &mut diagnostics);
        assert!(!diagnostics.iter().any(|d| d.code == "missing_theme_token"));
    }

    #[test]
    fn validate_shell_theme_rejects_literal_hash_color_keys() {
        let theme = json!({
            "font": {"1": "11px", "2": "13px", "3": "15px", "4": "18px"},
            "tokens": {
                "shell": {
                    "bg": "#000",
                    "text": "#fff",
                    "stage": "none",
                    "stage_border": "none",
                    "chrome_top_bg": "none",
                    "chrome_bottom_bg": "none",
                    "chrome_border_top": "none",
                    "chrome_border_bottom": "none",
                    "family_ui": "sans-serif"
                },
                "color": {
                    "text_primary": "#eee",
                    "text_muted": "#aaa",
                    "text_body": "#ccc",
                    "text_inverse": "#fff",
                    "panel_bg": "rgba(0,0,0,.5)",
                    "border_default": "rgba(0,0,0,.2)",
                    "literal_a1b2c3d4": "#fff"
                }
            }
        });
        let mut diagnostics = Vec::new();
        validate_shell_theme_value("host", &theme, ".mei-workspace.json", &mut diagnostics);
        assert!(diagnostics
            .iter()
            .any(|d| d.code == "shell_theme_hash_key_forbidden"));
    }
