use super::constants::*;

use super::{
    is_font_scale_key, is_forbidden_shell_color_key, is_literal_color, is_literal_font_size,
    is_literal_font_size_value, is_literal_gradient, push_diagnostic,
};

use serde_json::Value;

use crate::model::{Diagnostic, Severity};

pub(super) fn validate_theme_value_refs(
    value: &Value,
    context: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(map) = value.as_object() {
        for (key, entry) in map {
            let child_context = format!("{context}.{key}");
            let in_definition =
                TOKEN_DEFINITION_ROOTS.contains(&key.as_str()) || key.starts_with("tokens.");
            if in_definition {
                if key == "font" {
                    validate_font_definition(
                        entry,
                        child_context.as_str(),
                        target_file,
                        diagnostics,
                    );
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
                walk_value_for_token_refs(
                    entry,
                    child_context.as_str(),
                    false,
                    target_file,
                    diagnostics,
                );
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

pub(super) fn walk_value_for_token_refs(
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
            if is_literal_color(raw)
                || is_literal_gradient(raw)
                || raw.eq_ignore_ascii_case("transparent")
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

fn validate_color_ref(
    value: &Value,
    path: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

fn validate_font_ref(
    value: &Value,
    path: &str,
    target_file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

pub(super) fn validate_required_scene_theme_tokens(
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

pub(super) fn validate_required_shell_theme_tokens(
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
