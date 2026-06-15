use anyhow::{bail, Result};

const FORBIDDEN_TOKENS: &[&str] = &["for", "while", "lambda", "load", "import", "open"];

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

pub fn validate_authoring_policy(source: &str) -> Result<()> {
    let sanitized = sanitize_for_policy(source);
    for token in sanitized
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|token| !token.is_empty())
    {
        if FORBIDDEN_TOKENS.contains(&token) {
            bail!("authoring source contains forbidden token `{token}`");
        }
    }
    Ok(())
}

pub fn forbidden_authoring_tokens() -> &'static [&'static str] {
    FORBIDDEN_TOKENS
}
