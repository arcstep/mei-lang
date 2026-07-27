use mei_lang_kernel::ThemeDecl;
use serde_json::Value;

use super::parse::ThemeResolved;
use super::resolve_literals::resolve_color_token;

pub(super) fn theme_decl_value(theme: &ThemeDecl) -> Value {
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
    map.insert("chart_title".to_string(), theme.chart_title.clone());
    map.insert("chart_label".to_string(), theme.chart_label.clone());
    map.insert("table_head".to_string(), theme.table_head.clone());
    map.insert("table_body".to_string(), theme.table_body.clone());
    map.insert("filter_panel".to_string(), theme.filter_panel.clone());
    map.insert("body".to_string(), theme.body.clone());
    map.insert("muted".to_string(), theme.muted.clone());
    map.insert("header_title".to_string(), theme.header_title.clone());
    map.insert("tokens".to_string(), theme.tokens.clone());
    if !theme.shared.is_null() {
        map.insert("shared".to_string(), theme.shared.clone());
    }
    if !theme.components.is_null() {
        map.insert("components".to_string(), theme.components.clone());
    }
    Value::Object(map)
}

/// Theme-key → CSS var prefix (`mei-…`) → `.mei-text-{kebab}` suffix.
const TEXT_ROLE_EMIT: &[(&str, &str)] = &[
    ("metric_label", "mei-metric-label"),
    ("metric_value", "mei-metric-value"),
    ("metric_unit", "mei-metric-unit"),
    ("metric_desc", "mei-metric-desc"),
    ("metric_sub_label", "mei-metric-sub-label"),
    ("metric_sub_value", "mei-metric-sub-value"),
    ("metric_sub_unit", "mei-metric-sub-unit"),
    ("panel_head", "mei-panel-head"),
    ("chart_title", "mei-chart-title"),
    ("chart_label", "mei-chart-label"),
    ("table_head", "mei-table-head"),
    ("table_body", "mei-table-body"),
    ("filter_panel", "mei-filter-panel"),
    ("body", "mei-body"),
    ("muted", "mei-muted"),
    ("header_title", "mei-header-title"),
];

/// Scene viewport track: color, gradient, typography roles; excludes `tokens.shell`.
pub(super) fn collect_scene_css_vars(theme: &Value) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    if let Some(font) = theme
        .as_object()
        .and_then(|map| map.get("font"))
        .and_then(Value::as_object)
    {
        for (key, value) in font {
            if let Some(raw) = value.as_str() {
                vars.push((format!("--mei-font-{key}"), raw.to_string()));
                // 字阶最低档同时作为 typography 最小字号真源（图表/二级看板钳位）。
                if key == "1"
                    && !vars
                        .iter()
                        .any(|(name, _)| name == "--mei-typography-min-font-size")
                {
                    vars.push((
                        "--mei-typography-min-font-size".to_string(),
                        raw.to_string(),
                    ));
                }
            }
        }
    }
    for (theme_key, var_prefix) in TEXT_ROLE_EMIT {
        if let Some(entry) = theme.as_object().and_then(|map| map.get(*theme_key)) {
            push_typography_vars(entry, var_prefix, &mut vars, false);
        }
    }
    if let Some(tokens) = theme.as_object().and_then(|map| map.get("tokens")) {
        flatten_scene_tokens(tokens, &mut vars);
    }
    vars
}

/// Host shell track for `<body>`: shell chrome + shell semantic colors + shell font scale.
pub(super) fn collect_shell_css_vars(theme: &Value) -> Vec<(String, String)> {
    let mut vars = Vec::new();
    if let Some(font) = theme
        .as_object()
        .and_then(|map| map.get("font"))
        .and_then(Value::as_object)
    {
        for (key, value) in font {
            if let Some(raw) = value.as_str() {
                let resolved = if key == "1" {
                    shell_font_with_minimum(raw, 16.0)
                } else {
                    raw.to_string()
                };
                vars.push((format!("--mei-shell-font-{key}"), resolved));
            }
        }
    }
    if let Some(tokens) = theme.as_object().and_then(|map| map.get("tokens")) {
        if let Some(shell) = tokens.as_object().and_then(|map| map.get("shell")) {
            flatten_shell_partition(shell, &mut vars);
        }
        if let Some(color) = tokens.as_object().and_then(|map| map.get("color")) {
            flatten_tokens(color, "mei-shell-color", &mut vars);
        }
    }
    vars
}

fn shell_font_with_minimum(raw: &str, min_px: f64) -> String {
    let trimmed = raw.trim();
    if trimmed.ends_with("px") {
        if let Ok(value) = trimmed.trim_end_matches("px").trim().parse::<f64>() {
            if value < min_px {
                return format!("{}px", min_px as u32);
            }
        }
    }
    trimmed.to_string()
}

#[allow(dead_code)]
pub(super) fn collect_theme_css_vars(theme: &Value) -> Vec<(String, String)> {
    let mut vars = collect_shell_css_vars(theme);
    vars.extend(collect_scene_css_vars(theme));
    vars
}

fn flatten_scene_tokens(tokens: &Value, vars: &mut Vec<(String, String)>) {
    let Some(map) = tokens.as_object() else {
        return;
    };
    for (key, entry) in map {
        if key == "shell" {
            continue;
        }
        // typography.scale is promoted to theme.font; only emit family/weight leaves.
        if key == "typography" {
            if let Some(typo) = entry.as_object() {
                for (typo_key, typo_value) in typo {
                    if typo_key == "scale" {
                        continue;
                    }
                    push_token_leaf(
                        typo_value,
                        &format!("--mei-typography-{}", typo_key.replace('_', "-")),
                        vars,
                    );
                }
            }
            continue;
        }
        let path = format!("mei-{}", key.replace('_', "-"));
        flatten_tokens(entry, path.as_str(), vars);
    }
}

fn flatten_shell_partition(value: &Value, vars: &mut Vec<(String, String)>) {
    let Some(map) = value.as_object() else {
        return;
    };
    for (key, entry) in map {
        if key.starts_with("chrome_") {
            let suffix = key.trim_start_matches("chrome_").replace('_', "-");
            push_token_leaf(entry, &format!("--mei-chrome-{suffix}"), vars);
        } else {
            let path = format!("mei-shell-{}", key.replace('_', "-"));
            flatten_tokens(entry, path.as_str(), vars);
        }
    }
}

fn push_token_leaf(value: &Value, var_name: &str, vars: &mut Vec<(String, String)>) {
    match value {
        Value::String(raw) if !raw.trim().is_empty() => {
            vars.push((var_name.to_string(), raw.to_string()));
        }
        Value::Number(raw) => {
            vars.push((var_name.to_string(), raw.to_string()));
        }
        Value::Bool(raw) => {
            vars.push((var_name.to_string(), raw.to_string()));
        }
        _ => {}
    }
}

fn typography_css_suffix(key: &str) -> Option<&'static str> {
    match key {
        "font" | "font_size" => Some("font-size"),
        "font_family" => Some("font-family"),
        "color" => Some("color"),
        "font_weight" => Some("font-weight"),
        "font_style" | "style" => Some("font-style"),
        "letter_spacing" => Some("letter-spacing"),
        "text_align" | "align" => Some("text-align"),
        "line_height" => Some("line-height"),
        _ => None,
    }
}

fn resolve_font_size_value(raw: &str, shell: bool) -> String {
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
    } else if shell {
        format!("var(--mei-shell-font-{font_key})")
    } else {
        format!("var(--mei-font-{font_key})")
    }
}

fn resolve_font_weight_value(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "regular" | "normal" => "var(--mei-typography-weight-regular, 400)".to_string(),
        "medium" => "var(--mei-typography-weight-medium, 500)".to_string(),
        "bold" => "var(--mei-typography-weight-bold, 700)".to_string(),
        other if !other.is_empty() => other.to_string(),
        _ => String::new(),
    }
}

fn push_typography_vars(
    entry: &Value,
    var_prefix: &str,
    vars: &mut Vec<(String, String)>,
    shell: bool,
) {
    let Some(map) = entry.as_object() else {
        return;
    };
    let has_font_token = map.get("font").is_some_and(|value| {
        value.as_str().is_some_and(|raw| !raw.trim().is_empty()) || value.as_i64().is_some()
    });
    for (key, value) in map {
        if has_font_token && matches!(key.as_str(), "font_size" | "fontSize") {
            continue;
        }
        let Some(suffix) = typography_css_suffix(key) else {
            continue;
        };
        let resolved = match value {
            Value::String(raw) if !raw.trim().is_empty() => {
                if suffix == "font-size" {
                    resolve_font_size_value(raw, shell)
                } else if suffix == "color" {
                    resolve_color_token(raw)
                } else if suffix == "font-weight" {
                    resolve_font_weight_value(raw)
                } else {
                    raw.trim().to_string()
                }
            }
            Value::Number(raw) if suffix == "font-size" || suffix == "font-weight" => {
                raw.to_string()
            }
            _ => continue,
        };
        if resolved.is_empty() {
            continue;
        }
        vars.push((format!("--{var_prefix}-{suffix}"), resolved));
    }
}

/// Compose `.mei-text-*` utility rules from already-emitted role CSS variables.
pub fn compose_text_role_utility_css(css_vars: &[(String, String)]) -> String {
    let present: std::collections::BTreeSet<&str> = css_vars
        .iter()
        .filter_map(|(key, _)| key.strip_prefix("--"))
        .collect();
    let mut out = String::new();
    for (_, var_prefix) in TEXT_ROLE_EMIT {
        let class = format!(
            ".mei-text-{}",
            var_prefix
                .strip_prefix("mei-")
                .unwrap_or(var_prefix)
                .replace('_', "-")
        );
        let size_key = format!("{var_prefix}-font-size");
        let color_key = format!("{var_prefix}-color");
        let weight_key = format!("{var_prefix}-font-weight");
        let family_key = format!("{var_prefix}-font-family");
        let style_key = format!("{var_prefix}-font-style");
        let has_any = [
            size_key.as_str(),
            color_key.as_str(),
            weight_key.as_str(),
            family_key.as_str(),
            style_key.as_str(),
        ]
        .iter()
        .any(|k| present.contains(k));
        if !has_any {
            continue;
        }
        out.push_str(&class);
        out.push('{');
        if present.contains(size_key.as_str()) {
            out.push_str(&format!("font-size:var(--{size_key});"));
        }
        if present.contains(color_key.as_str()) {
            out.push_str(&format!("color:var(--{color_key});"));
        }
        out.push_str(&format!(
            "font-weight:var(--{weight_key},var(--mei-typography-weight-regular,400));"
        ));
        out.push_str(&format!(
            "font-family:var(--{family_key},var(--mei-typography-family,system-ui,sans-serif));"
        ));
        out.push_str(&format!("font-style:var(--{style_key},normal);"));
        out.push('}');
    }
    out
}

/// Full scene theme stylesheet: selector-scoped vars + `.mei-text-*` utilities.
pub fn scene_theme_stylesheet(theme_id: &str, css_vars: &[(String, String)]) -> String {
    let mut out = String::new();
    out.push_str(
        ":root,.mei-compose-scene-root,#mei-compose-root,.preview-viewport,[data-mei-frame-viewport],body{",
    );
    out.push_str(&css_vars_to_style(theme_id, css_vars));
    out.push('}');
    out.push_str(&compose_text_role_utility_css(css_vars));
    out
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

pub(crate) fn css_vars_to_style(theme_id: &str, css_vars: &[(String, String)]) -> String {
    let mut style = String::new();
    style.push_str(&format!("--mei-theme-id:'{theme_id}';"));
    for (key, value) in css_vars {
        style.push_str(&format!("{key}:{value};"));
    }
    style
}

pub(crate) fn theme_css_vars_style(theme: &ThemeResolved) -> String {
    css_vars_to_style(theme.id.as_str(), &theme.css_vars)
}

pub(crate) fn shell_css_vars_style(theme_id: &str, css_vars: &[(String, String)]) -> String {
    css_vars_to_style(theme_id, css_vars)
}

pub(crate) fn scene_css_vars_style(theme: &ThemeResolved) -> String {
    css_vars_to_style(theme.id.as_str(), &theme.css_vars)
}

pub(crate) fn scene_theme_stylesheet_for_resolved(theme: &ThemeResolved) -> String {
    scene_theme_stylesheet(theme.id.as_str(), &theme.css_vars)
}
