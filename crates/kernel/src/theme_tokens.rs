use serde_json::Value;

use crate::model::{Diagnostic, FrameDecl, PanelDecl, Severity, ThemeDecl};

const TOKEN_DEFINITION_ROOTS: &[&str] = &["tokens", "font"];

const COLOR_REF_KEYS: &[&str] = &["color"];
const FONT_REF_KEYS: &[&str] = &["font"];
const FONT_SIZE_FORBIDDEN_KEYS: &[&str] = &["font_size", "fontSize"];

const REQUIRED_COLOR_KEYS_PAGE: &[&str] = &[
    "text_primary",
    "text_muted",
    "text_body",
    "text_inverse",
    "surface_bg",
    "border_default",
];

const REQUIRED_SHELL_KEYS: &[&str] = &[
    "bg",
    "text",
    "stage",
    "stage_border",
    "chrome_top_bg",
    "chrome_bottom_bg",
    "chrome_border_top",
    "chrome_border_bottom",
    "family_ui",
];

const REQUIRED_SHELL_COLOR_KEYS: &[&str] = &[
    "text_primary",
    "text_muted",
    "text_body",
    "text_inverse",
    "panel_bg",
    "border_default",
];

const REQUIRED_SHELL_FONT_KEYS: &[&str] = &["1", "2", "3", "4"];

const REQUIRED_COLOR_KEYS_COCKPIT: &[&str] = &[
    "text_value",
    "text_unit",
    "text_accent",
    "panel_title",
    "section_border",
    "chart_1",
    "chart_2",
    "chart_3",
    "chart_4",
    "chart_5",
    "chart_6",
];

pub fn validate_theme_decl(
    theme: &ThemeDecl,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let value = theme_decl_to_value(theme);
    validate_theme_value_refs(&value, "theme", target_file, diagnostics);
    let profile = theme.id.as_str();
    validate_required_scene_theme_tokens(&value, profile, target_file, diagnostics);
}

pub fn validate_scene_theme_value_from_ops(
    id: &str,
    value: &Value,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_theme_value_refs(value, &format!("ops.themes.`{id}`"), target_file, diagnostics);
    validate_required_scene_theme_tokens(value, id, target_file, diagnostics);
}

/// Workspace / host shell theme (`ops.themes` on `.mei-workspace.json`).
pub fn validate_shell_theme_value(
    id: &str,
    value: &Value,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_theme_value_refs(
        value,
        &format!("workspace.ops.themes.`{id}`"),
        target_file,
        diagnostics,
    );
    validate_required_shell_theme_tokens(value, target_file, diagnostics);
}

#[deprecated(note = "use validate_scene_theme_value_from_ops for app scene themes")]
pub fn validate_theme_value_from_ops(
    id: &str,
    value: &Value,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_scene_theme_value_from_ops(id, value, target_file, diagnostics);
}

pub fn validate_frame_token_refs(
    frame: &FrameDecl,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let context = frame
        .id
        .as_deref()
        .map(|id| format!("frame `{id}`"))
        .unwrap_or_else(|| "frame".to_string());
    validate_props_token_refs(&frame.props, context.as_str(), target_file, diagnostics);
}

pub fn validate_panel_token_refs(
    panel: &PanelDecl,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let context = format!("panel `{}`", panel.id);
    validate_props_token_refs(&panel.props, context.as_str(), target_file, diagnostics);
    validate_props_token_refs(
        &panel.head_props,
        &format!("{context}.head_props"),
        target_file,
        diagnostics,
    );
    validate_props_token_refs(
        &panel.body_props,
        &format!("{context}.body_props"),
        target_file,
        diagnostics,
    );
}

fn validate_props_token_refs(
    value: &Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    walk_value_for_token_refs(value, context, false, target_file, diagnostics);
}

fn validate_theme_value_refs(
    value: &Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(map) = value.as_object() {
        for (key, entry) in map {
            let child_context = format!("{context}.{key}");
            let in_definition = TOKEN_DEFINITION_ROOTS.contains(&key.as_str())
                || key.starts_with("tokens.");
            if in_definition {
                if key == "font" {
                    validate_font_definition(entry, child_context.as_str(), target_file, diagnostics);
                }
                continue;
            }
            if matches!(
                key.as_str(),
                "metric_label"
                    | "metric_value"
                    | "metric_unit"
                    | "metric_desc"
                    | "metric_sub_label"
                    | "metric_sub_value"
                    | "metric_sub_unit"
                    | "chart_title"
                    | "chart_label"
                    | "table_head"
                    | "table_body"
                    | "filter_panel"
                    | "panel_head"
                    | "heading"
                    | "frame"
                    | "panel"
                    | "panel_bare"
                    | "panel_body"
            ) {
                walk_value_for_token_refs(entry, child_context.as_str(), false, target_file, diagnostics);
            }
        }
    }
}

fn validate_font_definition(
    value: &Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(map) = value.as_object() else {
        return;
    };
    for (key, entry) in map {
        let Some(raw) = entry.as_str() else {
            continue;
        };
        if !is_literal_font_size(raw) {
            push_diagnostic(
                diagnostics,
                Severity::Error,
                "invalid_font_definition",
                format!("{context}.{key} must be a literal font size (e.g. `14px`)"),
                target_file,
            );
        }
    }
}

fn walk_value_for_token_refs(
    value: &Value,
    path: &str,
    inside_background: bool,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        Value::Object(map) => {
            if map.get("__ref").is_some() {
                return;
            }
            for (key, entry) in map {
                let child_path = format!("{path}.{key}");
                let key_lower = key.to_ascii_lowercase();
                if COLOR_REF_KEYS.contains(&key_lower.as_str()) {
                    validate_color_ref(entry, child_path.as_str(), target_file, diagnostics);
                } else if FONT_REF_KEYS.contains(&key_lower.as_str()) {
                    validate_font_ref(entry, child_path.as_str(), target_file, diagnostics);
                } else if FONT_SIZE_FORBIDDEN_KEYS.contains(&key_lower.as_str()) {
                    if is_literal_font_size_value(entry) {
                        push_diagnostic(
                            diagnostics,
                            Severity::Error,
                            "literal_font_size_forbidden",
                            format!(
                                "`{child_path}` must use `font = \"N\"` scale key instead of literal font size"
                            ),
                            target_file,
                        );
                    }
                } else if key_lower == "background" {
                    validate_background_ref(entry, child_path.as_str(), target_file, diagnostics);
                } else {
                    let next_inside = inside_background || key_lower == "background";
                    walk_value_for_token_refs(
                        entry,
                        child_path.as_str(),
                        next_inside,
                        target_file,
                        diagnostics,
                    );
                }
            }
        }
        Value::Array(items) => {
            for (idx, entry) in items.iter().enumerate() {
                walk_value_for_token_refs(
                    entry,
                    &format!("{path}[{idx}]"),
                    inside_background,
                    target_file,
                    diagnostics,
                );
            }
        }
        Value::String(raw) if inside_background => {
            if is_literal_color(raw) || is_literal_gradient(raw) {
                push_diagnostic(
                    diagnostics,
                    Severity::Error,
                    "literal_background_forbidden",
                    format!(
                        "`{path}` must use a gradient token name; move literal to `theme.tokens.gradient`"
                    ),
                    target_file,
                );
            }
        }
        _ => {}
    }
}

fn validate_background_ref(
    value: &Value,
    path: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        Value::String(raw) => {
            if is_literal_color(raw) || is_literal_gradient(raw) || raw.eq_ignore_ascii_case("transparent")
            {
                if !raw.eq_ignore_ascii_case("transparent") {
                    push_diagnostic(
                        diagnostics,
                        Severity::Error,
                        "literal_background_forbidden",
                        format!(
                            "`{path}` must be a gradient token name; move literal to `theme.tokens.gradient`"
                        ),
                        target_file,
                    );
                }
            }
        }
        Value::Object(map) => {
            if let Some(color) = map.get("color") {
                validate_color_ref(color, &format!("{path}.color"), target_file, diagnostics);
            }
            if let Some(image) = map.get("image").and_then(Value::as_str) {
                if is_literal_color(image) || is_literal_gradient(image) {
                    push_diagnostic(
                        diagnostics,
                        Severity::Error,
                        "literal_background_forbidden",
                        format!(
                            "`{path}.image` must be a gradient token name; move literal to `theme.tokens.gradient`"
                        ),
                        target_file,
                    );
                }
            }
            for (key, entry) in map {
                if matches!(key.as_str(), "color" | "image") {
                    continue;
                }
                walk_value_for_token_refs(
                    entry,
                    &format!("{path}.{key}"),
                    true,
                    target_file,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

fn validate_color_ref(value: &Value, path: &str, target_file: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(raw) = value.as_str() else {
        return;
    };
    if is_literal_color(raw) {
        push_diagnostic(
            diagnostics,
            Severity::Error,
            "literal_color_forbidden",
            format!("`{path}` must be a color token name (e.g. `text_primary`), not a literal"),
            target_file,
        );
    }
}

fn validate_font_ref(value: &Value, path: &str, target_file: &str, diagnostics: &mut Vec<Diagnostic>) {
    let raw = match value {
        Value::String(raw) => raw.as_str(),
        Value::Number(raw) => {
            let key = raw.to_string();
            if is_font_scale_key(key.as_str()) {
                return;
            }
            push_diagnostic(
                diagnostics,
                Severity::Error,
                "invalid_font_ref",
                format!("`{path}` must be a font scale key (`\"1\"`..`\"5\"`)"),
                target_file,
            );
            return;
        }
        _ => return,
    };
    if is_literal_font_size(raw) {
        push_diagnostic(
            diagnostics,
            Severity::Error,
            "literal_font_size_forbidden",
            format!("`{path}` must be a font scale key (`\"1\"`..`\"5\"`), not a literal size"),
            target_file,
        );
        return;
    }
    if !is_font_scale_key(raw) {
        push_diagnostic(
            diagnostics,
            Severity::Error,
            "invalid_font_ref",
            format!("`{path}` must be a font scale key (`\"1\"`..`\"5\"`)"),
            target_file,
        );
    }
}

fn validate_required_scene_theme_tokens(
    value: &Value,
    profile: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let colors = value
        .as_object()
        .and_then(|map| map.get("tokens"))
        .and_then(Value::as_object)
        .and_then(|tokens| tokens.get("color"))
        .and_then(Value::as_object);

    for key in REQUIRED_COLOR_KEYS_PAGE {
        if colors.and_then(|map| map.get(*key)).is_none() {
            push_diagnostic(
                diagnostics,
                Severity::Warning,
                "missing_theme_token",
                format!("theme is missing required tokens.color.{key}"),
                target_file,
            );
        }
    }
    if profile == "cockpit" || profile.contains("cockpit") {
        for key in REQUIRED_COLOR_KEYS_COCKPIT {
            if colors.and_then(|map| map.get(*key)).is_none() {
                push_diagnostic(
                    diagnostics,
                    Severity::Warning,
                    "missing_theme_token",
                    format!("cockpit theme is missing required tokens.color.{key}"),
                    target_file,
                );
            }
        }
    }
}

fn validate_required_shell_theme_tokens(
    value: &Value,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let colors = value
        .as_object()
        .and_then(|map| map.get("tokens"))
        .and_then(Value::as_object)
        .and_then(|tokens| tokens.get("color"))
        .and_then(Value::as_object);
    let shell = value
        .as_object()
        .and_then(|map| map.get("tokens"))
        .and_then(Value::as_object)
        .and_then(|tokens| tokens.get("shell"))
        .and_then(Value::as_object);
    let font = value
        .as_object()
        .and_then(|map| map.get("font"))
        .and_then(Value::as_object);

    for key in REQUIRED_SHELL_KEYS {
        if shell.and_then(|map| map.get(*key)).is_none() {
            push_diagnostic(
                diagnostics,
                Severity::Warning,
                "missing_theme_token",
                format!("shell theme is missing required tokens.shell.{key}"),
                target_file,
            );
        }
    }
    for key in REQUIRED_SHELL_COLOR_KEYS {
        if colors.and_then(|map| map.get(*key)).is_none() {
            push_diagnostic(
                diagnostics,
                Severity::Warning,
                "missing_theme_token",
                format!("shell theme is missing required tokens.color.{key}"),
                target_file,
            );
        }
    }
    for key in REQUIRED_SHELL_FONT_KEYS {
        if font.and_then(|map| map.get(*key)).is_none() {
            push_diagnostic(
                diagnostics,
                Severity::Warning,
                "missing_theme_token",
                format!("shell theme is missing required font.{key}"),
                target_file,
            );
        }
    }
    if let Some(map) = colors {
        for key in map.keys() {
            if is_forbidden_shell_color_key(key) {
                push_diagnostic(
                    diagnostics,
                    Severity::Warning,
                    "shell_theme_hash_key_forbidden",
                    format!(
                        "shell theme tokens.color.{key} uses a hash/literal key; use semantic snake_case names (see topic 33)"
                    ),
                    target_file,
                );
            }
        }
    }
}

fn is_forbidden_shell_color_key(key: &str) -> bool {
    if key.starts_with("literal_") {
        return true;
    }
    let Some((prefix, suffix)) = key.rsplit_once('_') else {
        return false;
    };
    prefix.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        && suffix.len() == 8
        && suffix.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_literal_color(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.starts_with('#')
        || trimmed.starts_with("rgb(")
        || trimmed.starts_with("rgba(")
        || trimmed.starts_with("hsl(")
        || trimmed.starts_with("hsla(")
}

pub fn is_literal_gradient(raw: &str) -> bool {
    let lower = raw.trim().to_ascii_lowercase();
    lower.contains("gradient(") || lower.contains("url(")
}

pub fn is_literal_font_size(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.ends_with("px")
        || trimmed.ends_with("rem")
        || trimmed.ends_with("em")
        || trimmed.ends_with('%')
}

fn is_literal_font_size_value(value: &Value) -> bool {
    match value {
        Value::String(raw) => is_literal_font_size(raw),
        Value::Number(_) => true,
        _ => false,
    }
}

pub fn is_font_scale_key(raw: &str) -> bool {
    raw.chars().all(|ch| ch.is_ascii_digit()) && !raw.is_empty()
}

fn push_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    severity: Severity,
    code: &str,
    message: String,
    target_file: &str,
) {
    diagnostics.push(Diagnostic {
        severity,
        code: code.to_string(),
        message,
        source_path: Some(target_file.to_string()),
    });
}

fn theme_decl_to_value(theme: &ThemeDecl) -> Value {
    serde_json::json!({
        "id": theme.id,
        "frame": theme.frame,
        "panel": theme.panel,
        "panel_bare": theme.panel_bare,
        "panel_head": theme.panel_head,
        "panel_body": theme.panel_body,
        "heading": theme.heading,
        "font": theme.font,
        "metric_label": theme.metric_label,
        "metric_value": theme.metric_value,
        "metric_unit": theme.metric_unit,
        "metric_desc": theme.metric_desc,
        "metric_sub_label": theme.metric_sub_label,
        "metric_sub_value": theme.metric_sub_value,
        "metric_sub_unit": theme.metric_sub_unit,
        "chart_title": theme.chart_title,
        "chart_label": theme.chart_label,
        "table_head": theme.table_head,
        "table_body": theme.table_body,
        "filter_panel": theme.filter_panel,
        "tokens": theme.tokens,
        "shared": theme.shared,
        "components": theme.components,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(diagnostics.iter().any(|d| d.code == "literal_color_forbidden"));
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
        assert!(!diagnostics.iter().any(|d| d.code == "literal_color_forbidden"));
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
        assert!(diagnostics.iter().any(|d| d.code == "shell_theme_hash_key_forbidden"));
    }
}
