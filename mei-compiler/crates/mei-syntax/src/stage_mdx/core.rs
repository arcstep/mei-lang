//! Stage MDX common core: frontmatter fence, directive helpers, Markdown safety.
//! Profile-specific parsers (Deck / Cockpit) build on these primitives.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable diagnostic codes for Stage MDX (Gate 4).
pub mod codes {
    pub const JSX_FORBIDDEN: &str = "stage_mdx_jsx_forbidden";
    pub const JS_FORBIDDEN: &str = "stage_mdx_js_forbidden";
    pub const SCENE_UNRESOLVED: &str = "stage_mdx_scene_unresolved";
    pub const SLOT_UNKNOWN: &str = "stage_mdx_slot_unknown";
    pub const CAPABILITY_MISMATCH: &str = "stage_mdx_capability_mismatch";
    pub const PRIVATE_PATH: &str = "stage_mdx_private_path";
    pub const DUAL_SOURCE: &str = "narration_aot_session_dual_source";
    pub const PARSE: &str = "stage_mdx_parse";
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{location}: [{code}] {message}")]
pub struct StageMdxError {
    pub path: Option<PathBuf>,
    pub line: usize,
    pub column: usize,
    pub code: String,
    pub message: String,
    location: String,
}

impl StageMdxError {
    pub fn new(
        path: Option<&Path>,
        line: usize,
        column: usize,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let owned_path = path.map(Path::to_path_buf);
        let location = match path {
            Some(path) => format!("{}:{line}:{column}", path.display()),
            None => format!("line {line}, column {column}"),
        };
        Self {
            path: owned_path,
            line,
            column,
            code: code.into(),
            message: message.into(),
            location,
        }
    }

    pub fn io(path: &Path, error: std::io::Error) -> Self {
        Self::new(
            Some(path),
            1,
            1,
            codes::PARSE,
            format!("failed to read stage mdx: {error}"),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownForbidden {
    BareDirective,
    JsxHtml,
    JsxExpr,
}

impl MarkdownForbidden {
    pub fn code(self) -> &'static str {
        match self {
            Self::BareDirective | Self::JsxExpr => codes::JS_FORBIDDEN,
            Self::JsxHtml => codes::JSX_FORBIDDEN,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::BareDirective => "directives are not allowed inside Markdown blocks",
            Self::JsxHtml => "JSX/HTML is not allowed in Stage MDX Markdown",
            Self::JsxExpr => "JSX expressions are not allowed in Stage MDX Markdown",
        }
    }
}

/// Reject JSX/HTML/`{}`/bare `@` inside Markdown content lines.
pub fn check_markdown_line(raw: &str) -> Result<(), MarkdownForbidden> {
    let trimmed = raw.trim();
    if trimmed.starts_with('@') {
        return Err(MarkdownForbidden::BareDirective);
    }
    if raw.contains('<') || raw.contains('>') {
        return Err(MarkdownForbidden::JsxHtml);
    }
    if raw.contains('{') || raw.contains('}') {
        return Err(MarkdownForbidden::JsxExpr);
    }
    Ok(())
}

pub fn find_frontmatter_end(path: Option<&Path>, lines: &[&str]) -> Result<usize, StageMdxError> {
    if lines.first().map(|line| line.trim()) != Some("---") {
        return Err(StageMdxError::new(
            path,
            1,
            1,
            codes::PARSE,
            "document must start with `---` frontmatter",
        ));
    }
    lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (line.trim() == "---").then_some(index))
        .ok_or_else(|| {
            StageMdxError::new(
                path,
                1,
                1,
                codes::PARSE,
                "frontmatter is missing closing `---`",
            )
        })
}

pub fn unquote(value: &str) -> String {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if matches!(
            (bytes[0], bytes[value.len() - 1]),
            (b'"', b'"') | (b'\'', b'\'')
        ) {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

/// Parse simple `key: value` frontmatter into a map (line numbers are 1-based file lines).
pub fn parse_frontmatter_map(
    path: Option<&Path>,
    lines: &[&str],
    start_line: usize,
    allowed: &[&str],
) -> Result<BTreeMap<String, (String, usize)>, StageMdxError> {
    let mut values = BTreeMap::<String, (String, usize)>::new();
    for (offset, raw) in lines.iter().enumerate() {
        let line_number = start_line + offset;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(StageMdxError::new(
                path,
                line_number,
                1,
                codes::PARSE,
                "frontmatter entries must use `key: value`",
            ));
        };
        let key = key.trim();
        if !allowed.contains(&key) {
            return Err(StageMdxError::new(
                path,
                line_number,
                1,
                codes::PARSE,
                format!("unknown frontmatter field `{key}`"),
            ));
        }
        let value = unquote(value.trim());
        if value.is_empty() {
            return Err(StageMdxError::new(
                path,
                line_number,
                key.len() + 2,
                codes::PARSE,
                format!("frontmatter field `{key}` cannot be empty"),
            ));
        }
        if values
            .insert(key.to_string(), (value, line_number))
            .is_some()
        {
            return Err(StageMdxError::new(
                path,
                line_number,
                1,
                codes::PARSE,
                format!("duplicate frontmatter field `{key}`"),
            ));
        }
    }
    Ok(values)
}

pub fn parse_directive_arg<'a>(line: &'a str, directive: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(directive)?;
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?.trim();
    (!inner.is_empty()).then_some(inner)
}

/// Parse `@name(key=value, key2=value2)` named args.
pub fn parse_named_directive_args(line: &str, directive: &str) -> Option<BTreeMap<String, String>> {
    let rest = line.strip_prefix(directive)?;
    let inner = rest.strip_prefix('(')?.strip_suffix(')')?.trim();
    if inner.is_empty() {
        return None;
    }
    let mut out = BTreeMap::new();
    for part in inner.split(',') {
        let part = part.trim();
        let Some((k, v)) = part.split_once('=') else {
            return None;
        };
        out.insert(k.trim().to_string(), unquote(v.trim()));
    }
    Some(out)
}

pub fn parse_heading(line: &str, level: usize) -> Option<(String, String)> {
    let marker = if level == 1 { "# " } else { "## " };
    let content = line.strip_prefix(marker)?;
    let marker_start = content.rfind(" {#")?;
    if !content.ends_with('}') {
        return None;
    }
    let title = content[..marker_start].trim();
    let id = &content[marker_start + 3..content.len() - 1];
    if title.is_empty() || id.is_empty() {
        return None;
    }
    Some((title.to_string(), id.to_string()))
}

pub fn validate_id_token(id: &str) -> bool {
    let mut chars = id.chars();
    let valid_start = chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_');
    let valid_rest = chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'));
    valid_start && valid_rest
}

/// True if a path looks like a Scene-private layout path (not a public ABI target).
pub fn looks_like_private_scene_path(path: &str) -> bool {
    let p = path.replace('\\', "/").to_ascii_lowercase();
    p.contains("/t1/")
        || p.contains("/t2/")
        || p.contains("/t0/")
        || p.contains("/r-")
        || p.contains("/s-")
        || p.contains("region/")
        || p.contains("section/")
        || p.contains("panel/")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageMarkdown {
    pub markdown: String,
    pub html: String,
}

pub fn markdown_from_lines(lines: &[&str]) -> StageMarkdown {
    let markdown = lines.join("\n").trim().to_string();
    StageMarkdown {
        html: render_markdown(&markdown),
        markdown,
    }
}

fn render_markdown(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut html = String::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        if line.is_empty() {
            index += 1;
            continue;
        }
        if let Some((level, heading)) = markdown_heading(line) {
            html.push_str(&format!("<h{level}>"));
            html.push_str(&render_inline(heading));
            html.push_str(&format!("</h{level}>"));
            index += 1;
            continue;
        }
        if unordered_item(line).is_some() {
            html.push_str("<ul>");
            while index < lines.len() {
                let Some(item) = unordered_item(lines[index].trim()) else {
                    break;
                };
                html.push_str("<li>");
                html.push_str(&render_inline(item));
                html.push_str("</li>");
                index += 1;
            }
            html.push_str("</ul>");
            continue;
        }
        if ordered_item(line).is_some() {
            html.push_str("<ol>");
            while index < lines.len() {
                let Some(item) = ordered_item(lines[index].trim()) else {
                    break;
                };
                html.push_str("<li>");
                html.push_str(&render_inline(item));
                html.push_str("</li>");
                index += 1;
            }
            html.push_str("</ol>");
            continue;
        }
        let mut paragraph = Vec::new();
        while index < lines.len() {
            let candidate = lines[index].trim();
            if candidate.is_empty()
                || markdown_heading(candidate).is_some()
                || unordered_item(candidate).is_some()
                || ordered_item(candidate).is_some()
            {
                break;
            }
            paragraph.push(candidate);
            index += 1;
        }
        html.push_str("<p>");
        html.push_str(&render_inline(&paragraph.join(" ")));
        html.push_str("</p>");
    }
    html
}

fn markdown_heading(line: &str) -> Option<(usize, &str)> {
    let level = line
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let content = line.get(level..)?.strip_prefix(' ')?.trim();
    (!content.is_empty()).then_some((level, content))
}

fn unordered_item(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
}

fn ordered_item(line: &str) -> Option<&str> {
    let (number, content) = line.split_once(". ")?;
    if number.chars().all(|ch| ch.is_ascii_digit()) {
        Some(content)
    } else {
        None
    }
}

fn render_inline(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut html = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '`' {
            if let Some(end) = chars[index + 1..].iter().position(|ch| *ch == '`') {
                let content: String = chars[index + 1..index + 1 + end].iter().collect();
                html.push_str("<code>");
                html.push_str(&escape_html(&content));
                html.push_str("</code>");
                index += end + 2;
                continue;
            }
        }
        if chars[index] == '*' && index + 1 < chars.len() && chars[index + 1] == '*' {
            if let Some(rel) = chars[index + 2..].iter().position(|ch| *ch == '*') {
                if index + 2 + rel + 1 < chars.len() && chars[index + 2 + rel + 1] == '*' {
                    let content: String = chars[index + 2..index + 2 + rel].iter().collect();
                    html.push_str("<strong>");
                    html.push_str(&render_inline(&content));
                    html.push_str("</strong>");
                    index += rel + 4;
                    continue;
                }
            }
        }
        if chars[index] == '*' {
            if let Some(end) = chars[index + 1..].iter().position(|ch| *ch == '*') {
                let content: String = chars[index + 1..index + 1 + end].iter().collect();
                if !content.is_empty() {
                    html.push_str("<em>");
                    html.push_str(&render_inline(&content));
                    html.push_str("</em>");
                    index += end + 2;
                    continue;
                }
            }
        }
        push_escaped_char(&mut html, chars[index]);
        index += 1;
    }
    html
}

fn escape_html(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        push_escaped_char(&mut out, ch);
    }
    out
}

fn push_escaped_char(output: &mut String, ch: char) {
    match ch {
        '&' => output.push_str("&amp;"),
        '<' => output.push_str("&lt;"),
        '>' => output.push_str("&gt;"),
        '"' => output.push_str("&quot;"),
        _ => output.push(ch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_jsx_and_braces() {
        assert_eq!(
            check_markdown_line("<div/>"),
            Err(MarkdownForbidden::JsxHtml)
        );
        assert_eq!(
            check_markdown_line("hello {world}"),
            Err(MarkdownForbidden::JsxExpr)
        );
        assert!(check_markdown_line("plain text").is_ok());
    }

    #[test]
    fn renders_markdown_headings_as_headings() {
        let body = markdown_from_lines(&["介绍", "", "## 基本信息", "", "正文"]);
        assert_eq!(body.html, "<p>介绍</p><h2>基本信息</h2><p>正文</p>");
    }
}
