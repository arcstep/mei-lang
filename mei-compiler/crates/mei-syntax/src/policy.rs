use std::path::Path;

use thiserror::Error;

const FORBIDDEN_TOKENS: &[&str] = &["for", "while", "lambda", "load", "import", "open"];
const GRID_ONLY_POLICY_ROOTS: &[&str] = &[
    "/workspaces/ws-demo-v2/apps/pretty-panels/",
    "/workspaces/ws-demo-v2/stock/templates/cockpit/",
];
const GRID_ONLY_POLICY_EXCLUDE_ROOTS: &[&str] = &[
    "/workspaces/ws-demo-v2/stock/templates/cockpit/drilldown/",
];
const GRID_ONLY_DEPRECATED_PATTERNS: &[(&str, &str)] = &[
    ("flex(", "flex(...)"),
    ("panel_slot(", "panel_slot(...)"),
    ("layout_policy", "layout_policy"),
];

fn sanitize_for_policy(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_string: Option<char> = None;
    let mut escaped = false;
    for ch in source.chars() {
        if let Some(quote) = in_string {
            if ch == '\n' {
                out.push('\n');
                escaped = false;
                continue;
            }
            out.push(' ');
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            continue;
        }
        match ch {
            '#' => out.push(' '),
            '"' | '\'' => {
                in_string = Some(ch);
                out.push(' ');
            }
            _ => out.push(ch),
        }
    }
    out
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct ForbiddenTokenError {
    pub token: String,
    pub message: String,
}

impl ForbiddenTokenError {
    fn forbidden_token(token: &str) -> Self {
        Self {
            token: token.to_string(),
            message: format!("authoring source contains forbidden token `{token}`"),
        }
    }

    fn deprecated_pattern(pattern: &str) -> Self {
        Self {
            token: pattern.to_string(),
            message: format!(
                "authoring source uses deprecated layout authoring `{pattern}`; use `grid + slot + content` instead"
            ),
        }
    }
}

pub fn validate_authoring_policy(source: &str) -> Result<(), ForbiddenTokenError> {
    let sanitized = sanitize_for_policy(source);
    for token in sanitized
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|token| !token.is_empty())
    {
        if FORBIDDEN_TOKENS.contains(&token) {
            return Err(ForbiddenTokenError::forbidden_token(token));
        }
    }
    Ok(())
}

pub fn validate_authoring_policy_for_path(
    path: &Path,
    source: &str,
) -> Result<(), ForbiddenTokenError> {
    validate_authoring_policy(source)?;
    if !should_enforce_grid_only(path) {
        return Ok(());
    }
    let sanitized = sanitize_for_policy(source);
    for (needle, label) in GRID_ONLY_DEPRECATED_PATTERNS {
        if sanitized.contains(needle) {
            return Err(ForbiddenTokenError::deprecated_pattern(label));
        }
    }
    Ok(())
}

pub fn forbidden_authoring_tokens() -> &'static [&'static str] {
    FORBIDDEN_TOKENS
}

fn should_enforce_grid_only(path: &Path) -> bool {
    let raw = path.to_string_lossy().replace('\\', "/");
    if GRID_ONLY_POLICY_EXCLUDE_ROOTS
        .iter()
        .any(|segment| raw.contains(segment))
    {
        return false;
    }
    GRID_ONLY_POLICY_ROOTS
        .iter()
        .any(|segment| raw.contains(segment))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_only_policy_rejects_flex_in_cockpit_templates() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/stock/templates/cockpit/example.mei",
        );
        let err = validate_authoring_policy_for_path(
            path,
            r#"frame(id = "home_frame", layout = flex(direction = "column"))"#,
        )
        .expect_err("flex should be rejected in cockpit templates");
        assert!(err.to_string().contains("grid + slot + content"));
    }

    #[test]
    fn grid_only_policy_keeps_legacy_paths_compatible() {
        let path = Path::new(
            "/tmp/workspaces/ws-demo-v2/stock/components/chart/echarts/previews/chart.bar.mei",
        );
        validate_authoring_policy_for_path(
            path,
            r#"frame(id = "home_frame", layout = flex(direction = "column"))"#,
        )
        .expect("legacy preview path remains compatible");
    }
}
