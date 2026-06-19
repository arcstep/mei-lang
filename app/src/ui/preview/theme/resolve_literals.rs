use mei_lang_kernel::{is_literal_color, is_literal_gradient, is_font_scale_key};

/// Resolve a color token name or pass through transparent / existing var().
pub(crate) fn resolve_color_token(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.eq_ignore_ascii_case("transparent") {
        return "transparent".to_string();
    }
    if trimmed.starts_with("var(") {
        return trimmed.to_string();
    }
    if is_literal_color(trimmed) {
        return trimmed.to_string();
    }
    let key = trimmed.replace('_', "-");
    format!("var(--mei-color-{key})")
}

/// Resolve gradient token name to CSS var.
pub(crate) fn resolve_gradient_token(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with("var(") || is_literal_gradient(trimmed) || is_literal_color(trimmed) {
        return trimmed.to_string();
    }
    let key = trimmed.replace('_', "-");
    format!("var(--mei-gradient-{key})")
}

/// Resolve font scale key to CSS var.
pub(crate) fn resolve_font_token(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if is_literal_color(trimmed) || trimmed.ends_with("px") || trimmed.ends_with("rem") {
        return trimmed.to_string();
    }
    if is_font_scale_key(trimmed) {
        return format!("var(--mei-font-{trimmed}, 14px)");
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_token_to_var() {
        assert_eq!(resolve_color_token("text_primary"), "var(--mei-color-text-primary)");
    }

    #[test]
    fn gradient_token_to_var() {
        assert_eq!(
            resolve_gradient_token("frame_cockpit"),
            "var(--mei-gradient-frame-cockpit)"
        );
    }
}
