use serde_json::Value;

use crate::model::{Diagnostic, Severity, ThemeDecl};

pub(super) fn is_forbidden_shell_color_key(key: &str) -> bool {
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

pub(super) fn is_literal_font_size_value(value: &Value) -> bool {
    match value {
        Value::String(raw) => is_literal_font_size(raw),
        Value::Number(_) => true,
        _ => false,
    }
}

pub fn is_font_scale_key(raw: &str) -> bool {
    raw.chars().all(|ch| ch.is_ascii_digit()) && !raw.is_empty()
}

pub(super) fn push_diagnostic(
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

pub(super) fn theme_decl_to_value(theme: &ThemeDecl) -> Value {
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

