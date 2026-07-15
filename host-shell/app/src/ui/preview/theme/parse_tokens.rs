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
    map.insert("tokens".to_string(), theme.tokens.clone());
    if !theme.shared.is_null() {
        map.insert("shared".to_string(), theme.shared.clone());
    }
    if !theme.components.is_null() {
        map.insert("components".to_string(), theme.components.clone());
    }
    Value::Object(map)
}

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
            }
        }
    }
    for role in ["label", "value", "unit", "desc"] {
        let key = format!("metric_{role}");
        if let Some(entry) = theme.as_object().and_then(|map| map.get(key.as_str())) {
            push_typography_vars(entry, &format!("mei-metric-{role}"), &mut vars, false);
        }
    }
    for role in ["label", "value", "unit"] {
        let key = format!("metric_sub_{role}");
        if let Some(entry) = theme.as_object().and_then(|map| map.get(key.as_str())) {
            push_typography_vars(entry, &format!("mei-metric-sub-{role}"), &mut vars, false);
        }
    }
    if let Some(panel_head) = theme.as_object().and_then(|map| map.get("panel_head")) {
        push_typography_vars(panel_head, "mei-panel-head", &mut vars, false);
    }
    if let Some(chart_title) = theme.as_object().and_then(|map| map.get("chart_title")) {
        push_typography_vars(chart_title, "mei-chart-title", &mut vars, false);
    }
    if let Some(chart_label) = theme.as_object().and_then(|map| map.get("chart_label")) {
        push_typography_vars(chart_label, "mei-chart-label", &mut vars, false);
    }
    if let Some(table_head) = theme.as_object().and_then(|map| map.get("table_head")) {
        push_typography_vars(table_head, "mei-table-head", &mut vars, false);
    }
    if let Some(table_body) = theme.as_object().and_then(|map| map.get("table_body")) {
        push_typography_vars(table_body, "mei-table-body", &mut vars, false);
    }
    if let Some(filter_panel) = theme.as_object().and_then(|map| map.get("filter_panel")) {
        push_typography_vars(filter_panel, "mei-filter-panel", &mut vars, false);
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
