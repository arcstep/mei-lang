use mei_lang_kernel::{is_font_scale_key, is_literal_color, is_literal_gradient};

/// Resolve color/gradient token names embedded in compound CSS values (border, box-shadow).
pub(crate) fn resolve_style_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed
        .split_whitespace()
        .map(resolve_style_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn resolve_style_word(word: &str) -> String {
    if word.is_empty() {
        return String::new();
    }
    if is_literal_color(word)
        || word.starts_with("var(")
        || word.ends_with("px")
        || word.ends_with("rem")
        || word.ends_with("%")
        || word.parse::<f64>().is_ok()
    {
        return word.to_string();
    }
    if matches!(
        word,
        "solid" | "dashed" | "dotted" | "double" | "none" | "hidden" | "inset" | "outset"
    ) {
        return word.to_string();
    }
    resolve_color_token(word)
}

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
        assert_eq!(
            resolve_color_token("text_primary"),
            "var(--mei-color-text-primary)"
        );
    }

    #[test]
    fn gradient_token_to_var() {
        assert_eq!(
            resolve_gradient_token("frame_cockpit"),
            "var(--mei-gradient-frame-cockpit)"
        );
    }

    #[test]
    fn style_value_resolves_embedded_color_tokens() {
        assert_eq!(
            resolve_style_value("2px solid viewport_border"),
            "2px solid var(--mei-color-viewport-border)"
        );
        assert_eq!(
            resolve_style_value("inset 0 0 0 1px viewport_border_inset"),
            "inset 0 0 0 1px var(--mei-color-viewport-border-inset)"
        );
        assert_eq!(
            resolve_style_value("2px dashed #facc15"),
            "2px dashed #facc15"
        );
    }
}
