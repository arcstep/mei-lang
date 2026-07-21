use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::stage_mdx::core::{
    check_markdown_line, find_frontmatter_end as core_frontmatter_end, parse_directive_arg,
    parse_heading, unquote, MarkdownForbidden,
};
use crate::v2::{slide_pattern_areas, SLIDE_PATTERNS};

pub const DECK_NARRATION_DIRECTIVE_FORBIDDEN: &str = "deck_narration_directive_forbidden";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckFile {
    pub frontmatter: DeckFrontmatter,
    pub slides: Vec<DeckSlide>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckFrontmatter {
    pub id: String,
    pub title: String,
    pub short_title: Option<String>,
    pub theme: Option<String>,
    pub canvas: Option<String>,
    pub summary: Option<String>,
    pub default_for_stage: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckSlide {
    pub id: String,
    pub title: String,
    pub pattern: String,
    pub chapter: Option<String>,
    pub caption: Option<DeckMarkdown>,
    pub speaker_notes: Option<DeckMarkdown>,
    pub source: Option<DeckSource>,
    pub slots: Vec<DeckSlot>,
    pub steps: Vec<DeckStep>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckSlot {
    pub name: String,
    pub viewpoint_id: String,
    pub content: DeckMarkdown,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckStep {
    pub viewpoint_id: String,
    pub content: DeckMarkdown,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckSource {
    /// Safe relative path under the presentation stage, e.g. `custom/demo.mei`.
    pub path: String,
    /// Template name inside that file (`#fragment`).
    pub fragment: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckMarkdown {
    pub markdown: String,
    pub html: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{location}: {message}")]
pub struct DeckParseError {
    pub path: Option<PathBuf>,
    pub line: usize,
    pub column: usize,
    pub message: String,
    location: String,
}

impl DeckParseError {
    fn new(path: Option<&Path>, line: usize, column: usize, message: impl Into<String>) -> Self {
        let owned_path = path.map(Path::to_path_buf);
        let location = match path {
            Some(path) => format!("{}:{line}:{column}", path.display()),
            None => format!("line {line}, column {column}"),
        };
        Self {
            path: owned_path,
            line,
            column,
            message: message.into(),
            location,
        }
    }

    fn io(path: &Path, error: std::io::Error) -> Self {
        Self {
            path: Some(path.to_path_buf()),
            line: 1,
            column: 1,
            message: format!("failed to read deck: {error}"),
            location: format!("{}:1:1", path.display()),
        }
    }
}

pub fn parse_deck_source(source: &str) -> Result<DeckFile, DeckParseError> {
    parse_deck_source_at(None, source)
}

pub fn parse_deck_source_file(path: &Path) -> Result<DeckFile, DeckParseError> {
    let source = std::fs::read_to_string(path).map_err(|error| DeckParseError::io(path, error))?;
    parse_deck_source_at(Some(path), &source)
}

fn parse_deck_source_at(path: Option<&Path>, source: &str) -> Result<DeckFile, DeckParseError> {
    let lines: Vec<&str> = source.lines().collect();
    let frontmatter_end = parse_frontmatter_end(path, &lines)?;
    let frontmatter = parse_frontmatter(path, &lines[1..frontmatter_end])?;
    let mut parser = DeckParser {
        path,
        lines: &lines,
        index: frontmatter_end + 1,
        slide_ids: BTreeSet::new(),
        viewpoint_ids: BTreeMap::new(),
    };
    let slides = parser.parse_slides()?;
    if slides.is_empty() {
        return Err(DeckParseError::new(
            path,
            frontmatter_end + 2,
            1,
            "deck must contain at least one H1 slide",
        ));
    }
    Ok(DeckFile {
        frontmatter,
        slides,
    })
}

fn parse_frontmatter_end(path: Option<&Path>, lines: &[&str]) -> Result<usize, DeckParseError> {
    core_frontmatter_end(path, lines).map_err(|e| {
        DeckParseError::new(
            path,
            e.line,
            e.column,
            e.message.replace("document must start", "deck must start"),
        )
    })
}

fn parse_frontmatter(
    path: Option<&Path>,
    lines: &[&str],
) -> Result<DeckFrontmatter, DeckParseError> {
    let allowed = [
        "id",
        "title",
        "short_title",
        "theme",
        "canvas",
        "summary",
        "default_for_stage",
    ];
    let mut values = BTreeMap::<String, (String, usize)>::new();
    for (offset, raw) in lines.iter().enumerate() {
        let line_number = offset + 2;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(DeckParseError::new(
                path,
                line_number,
                1,
                "frontmatter entries must use `key: value`",
            ));
        };
        let key = key.trim();
        if !allowed.contains(&key) {
            let hint = match key {
                "deck_id" => " (use `id`)",
                "default_template" => " (use `default_for_stage`)",
                _ => "",
            };
            return Err(DeckParseError::new(
                path,
                line_number,
                1,
                format!("unknown frontmatter field `{key}`{hint}"),
            ));
        }
        let value = unquote(value.trim());
        if value.is_empty() {
            return Err(DeckParseError::new(
                path,
                line_number,
                key.len() + 2,
                format!("frontmatter field `{key}` cannot be empty"),
            ));
        }
        if values
            .insert(key.to_string(), (value, line_number))
            .is_some()
        {
            return Err(DeckParseError::new(
                path,
                line_number,
                1,
                format!("duplicate frontmatter field `{key}`"),
            ));
        }
    }

    let id = required_frontmatter(path, &values, "id")?;
    validate_id(path, values["id"].1, &id, "deck id")?;
    let title = required_frontmatter(path, &values, "title")?;
    let default_for_stage = match values.get("default_for_stage") {
        None => false,
        Some((value, line)) if value.eq_ignore_ascii_case("true") => true,
        Some((value, line)) if value.eq_ignore_ascii_case("false") => false,
        Some((_, line)) => {
            return Err(DeckParseError::new(
                path,
                *line,
                1,
                "`default_for_stage` must be `true` or `false`",
            ))
        }
    };
    Ok(DeckFrontmatter {
        id,
        title,
        short_title: values.get("short_title").map(|(value, _)| value.clone()),
        theme: values.get("theme").map(|(value, _)| value.clone()),
        canvas: values.get("canvas").map(|(value, _)| value.clone()),
        summary: values.get("summary").map(|(value, _)| value.clone()),
        default_for_stage,
    })
}

fn required_frontmatter(
    path: Option<&Path>,
    values: &BTreeMap<String, (String, usize)>,
    key: &str,
) -> Result<String, DeckParseError> {
    values
        .get(key)
        .map(|(value, _)| value.clone())
        .ok_or_else(|| {
            DeckParseError::new(
                path,
                1,
                1,
                format!("missing required frontmatter field `{key}`"),
            )
        })
}

struct DeckParser<'a> {
    path: Option<&'a Path>,
    lines: &'a [&'a str],
    index: usize,
    slide_ids: BTreeSet<String>,
    viewpoint_ids: BTreeMap<String, usize>,
}

impl DeckParser<'_> {
    fn parse_slides(&mut self) -> Result<Vec<DeckSlide>, DeckParseError> {
        let mut slides = Vec::new();
        while self.index < self.lines.len() {
            if self.lines[self.index].trim().is_empty() {
                self.index += 1;
                continue;
            }
            let line = self.index + 1;
            let Some((title, id)) = parse_heading(self.lines[self.index], 1) else {
                return Err(DeckParseError::new(
                    self.path,
                    line,
                    1,
                    "expected H1 slide heading `# Title {#slide-id}`",
                ));
            };
            validate_id(self.path, line, &id, "slide id")?;
            if !self.slide_ids.insert(id.clone()) {
                return Err(DeckParseError::new(
                    self.path,
                    line,
                    1,
                    format!("duplicate slide id `{id}`"),
                ));
            }
            self.index += 1;
            slides.push(self.parse_slide(title, id, line)?);
        }
        Ok(slides)
    }

    fn parse_slide(
        &mut self,
        title: String,
        id: String,
        line: usize,
    ) -> Result<DeckSlide, DeckParseError> {
        let mut pattern = None::<(String, usize)>;
        let mut chapter = None;
        let caption = None;
        let speaker_notes = None;
        let mut source = None;
        let mut slots = Vec::new();
        let steps = Vec::new();

        while self.index < self.lines.len() {
            let raw = self.lines[self.index];
            let trimmed = raw.trim();
            let current_line = self.index + 1;
            if trimmed.is_empty() {
                self.index += 1;
                continue;
            }
            if parse_heading(raw, 1).is_some() {
                break;
            }
            if let Some((slot_name, viewpoint_id)) = parse_heading(raw, 2) {
                validate_slot_name(self.path, current_line, &slot_name)?;
                validate_id(self.path, current_line, &viewpoint_id, "viewpoint id")?;
                if let Some(first_line) = self
                    .viewpoint_ids
                    .insert(viewpoint_id.clone(), current_line)
                {
                    return Err(DeckParseError::new(
                        self.path,
                        current_line,
                        1,
                        format!(
                            "duplicate viewpoint id `{viewpoint_id}` (first declared at line {first_line})"
                        ),
                    ));
                }
                if slots.iter().any(|slot: &DeckSlot| slot.name == slot_name) {
                    return Err(DeckParseError::new(
                        self.path,
                        current_line,
                        1,
                        format!("duplicate slot `{slot_name}` in slide `{id}`"),
                    ));
                }
                self.index += 1;
                let markdown = self.collect_slot_markdown()?;
                slots.push(DeckSlot {
                    name: slot_name,
                    viewpoint_id,
                    content: markdown,
                    line: current_line,
                });
                continue;
            }
            if trimmed.starts_with("##") {
                return Err(DeckParseError::new(
                    self.path,
                    current_line,
                    1,
                    "invalid H2 slot heading; expected `## slot-name {#viewpoint-id}`",
                ));
            }
            if let Some(value) = parse_directive_arg(trimmed, "@template") {
                if pattern.is_some() {
                    return Err(DeckParseError::new(
                        self.path,
                        current_line,
                        1,
                        "a slide may declare `@template` only once",
                    ));
                }
                if slide_pattern_areas(value).is_none() {
                    return Err(DeckParseError::new(
                        self.path,
                        current_line,
                        1,
                        format!(
                            "unknown slide pattern `{value}`; expected one of: {}",
                            SLIDE_PATTERNS.join(", ")
                        ),
                    ));
                }
                pattern = Some((value.to_string(), current_line));
                self.index += 1;
                continue;
            }
            if let Some(value) = parse_directive_arg(trimmed, "@chapter") {
                if chapter.replace(value.to_string()).is_some() {
                    return Err(DeckParseError::new(
                        self.path,
                        current_line,
                        1,
                        "a slide may declare `@chapter` only once",
                    ));
                }
                self.index += 1;
                continue;
            }
            if let Some(value) = parse_directive_arg(trimmed, "@source") {
                let (path, fragment) = parse_source_ref(self.path, current_line, value)?;
                if source
                    .replace(DeckSource {
                        path,
                        fragment,
                        line: current_line,
                    })
                    .is_some()
                {
                    return Err(DeckParseError::new(
                        self.path,
                        current_line,
                        1,
                        "a slide may declare `@source` only once",
                    ));
                }
                self.index += 1;
                continue;
            }
            if trimmed == "@caption" {
                return Err(DeckParseError::new(
                    self.path,
                    current_line,
                    1,
                    format!(
                        "[{DECK_NARRATION_DIRECTIVE_FORBIDDEN}] deck narration directive `@caption` is forbidden; use `src/narration/**/*.track.mdx`"
                    ),
                ));
            }
            if trimmed == "@speaker_notes" {
                return Err(DeckParseError::new(
                    self.path,
                    current_line,
                    1,
                    format!(
                        "[{DECK_NARRATION_DIRECTIVE_FORBIDDEN}] deck narration directive `@speaker_notes` is forbidden; use `src/narration/**/*.track.mdx`"
                    ),
                ));
            }
            if parse_directive_arg(trimmed, "@step").is_some() {
                return Err(DeckParseError::new(
                    self.path,
                    current_line,
                    1,
                    format!(
                        "[{DECK_NARRATION_DIRECTIVE_FORBIDDEN}] deck narration directive `@step` is forbidden; use `src/narration/**/*.track.mdx`"
                    ),
                ));
            }
            if trimmed.starts_with('@') {
                return Err(DeckParseError::new(
                    self.path,
                    current_line,
                    1,
                    format!("unknown or malformed directive `{trimmed}`"),
                ));
            }
            return Err(DeckParseError::new(
                self.path,
                current_line,
                1,
                "slide body must be inside an H2 semantic slot or a supported directive block",
            ));
        }

        let Some((pattern, pattern_line)) = pattern else {
            return Err(DeckParseError::new(
                self.path,
                line,
                1,
                format!("slide `{id}` is missing required `@template(pattern)`"),
            ));
        };
        let expected = slide_pattern_areas(&pattern).unwrap_or_default();
        let actual: BTreeSet<&str> = slots.iter().map(|slot| slot.name.as_str()).collect();
        for slot in &slots {
            if !expected.contains(&slot.name.as_str()) {
                return Err(DeckParseError::new(
                    self.path,
                    slot.line,
                    1,
                    format!(
                        "unknown slot `{}` for pattern `{pattern}`; expected: {}",
                        slot.name,
                        expected.join(", ")
                    ),
                ));
            }
        }
        if let Some(source) = &source {
            for slot in &slots {
                if !slot.content.markdown.trim().is_empty() {
                    return Err(DeckParseError::new(
                        self.path,
                        slot.line,
                        1,
                        format!(
                            "[deck_source_mixed_content] `@source({})` cannot mix Markdown slot body; leave H2 slots empty or remove them",
                            source_display(source)
                        ),
                    ));
                }
            }
        } else {
            let missing: Vec<&str> = expected
                .iter()
                .copied()
                .filter(|name| !actual.contains(name))
                .collect();
            if !missing.is_empty() {
                return Err(DeckParseError::new(
                    self.path,
                    pattern_line,
                    1,
                    format!(
                        "slide `{id}` is missing required slot(s) for pattern `{pattern}`: {}",
                        missing.join(", ")
                    ),
                ));
            }
        }
        Ok(DeckSlide {
            id,
            title,
            pattern,
            chapter,
            caption,
            speaker_notes,
            source,
            slots,
            steps,
            line,
        })
    }

    fn collect_slot_markdown(&mut self) -> Result<DeckMarkdown, DeckParseError> {
        let start = self.index;
        while self.index < self.lines.len() {
            let raw = self.lines[self.index];
            let trimmed = raw.trim();
            if parse_heading(raw, 1).is_some()
                || parse_heading(raw, 2).is_some()
                || trimmed.starts_with('@')
            {
                break;
            }
            validate_markdown_line(self.path, self.index + 1, raw)?;
            self.index += 1;
        }
        markdown_from_lines(&self.lines[start..self.index])
    }
}

fn validate_id(
    path: Option<&Path>,
    line: usize,
    id: &str,
    label: &str,
) -> Result<(), DeckParseError> {
    let mut chars = id.chars();
    let valid_start = chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_');
    let valid_rest = chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'));
    if valid_start && valid_rest {
        Ok(())
    } else {
        Err(DeckParseError::new(
            path,
            line,
            1,
            format!("invalid {label} `{id}`; expected `[A-Za-z_][A-Za-z0-9_-]*`"),
        ))
    }
}

fn validate_slot_name(path: Option<&Path>, line: usize, name: &str) -> Result<(), DeckParseError> {
    if name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        && name
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase())
    {
        Ok(())
    } else {
        Err(DeckParseError::new(
            path,
            line,
            1,
            format!("invalid slot name `{name}`; expected lower_snake_case"),
        ))
    }
}

fn source_display(source: &DeckSource) -> String {
    format!("{}#{}", source.path, source.fragment)
}

fn parse_source_ref(
    path: Option<&Path>,
    line: usize,
    raw: &str,
) -> Result<(String, String), DeckParseError> {
    let normalized = unquote(raw.trim()).replace('\\', "/");
    let Some((file_path, fragment)) = normalized.split_once('#') else {
        return Err(DeckParseError::new(
            path,
            line,
            1,
            format!(
                "invalid `@source` path `{raw}`; expected `custom/*.mei#fragment`"
            ),
        ));
    };
    let file_path = file_path.trim();
    let fragment = fragment.trim();
    let path_ok = file_path.starts_with("custom/")
        && file_path.ends_with(".mei")
        && !file_path.contains("//")
        && !file_path
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
        && file_path
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.'));
    let fragment_ok = !fragment.is_empty()
        && fragment
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && fragment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    if path_ok && fragment_ok {
        Ok((file_path.to_string(), fragment.to_string()))
    } else {
        Err(DeckParseError::new(
            path,
            line,
            1,
            format!(
                "invalid `@source` path `{raw}`; expected a safe `custom/*.mei#fragment` (fragment must be an identifier)"
            ),
        ))
    }
}

fn validate_markdown_line(
    path: Option<&Path>,
    line: usize,
    raw: &str,
) -> Result<(), DeckParseError> {
    match check_markdown_line(raw) {
        Ok(()) => Ok(()),
        Err(MarkdownForbidden::BareDirective) => Err(DeckParseError::new(
            path,
            line,
            1,
            format!(
                "directives are not allowed inside Markdown blocks: `{}`",
                raw.trim()
            ),
        )),
        Err(MarkdownForbidden::JsxHtml) => Err(DeckParseError::new(
            path,
            line,
            1,
            "JSX/HTML is not allowed in deck Markdown",
        )),
        Err(MarkdownForbidden::JsxExpr) => Err(DeckParseError::new(
            path,
            line,
            1,
            "JSX expressions are not allowed in deck Markdown",
        )),
    }
}

fn markdown_from_lines(lines: &[&str]) -> Result<DeckMarkdown, DeckParseError> {
    let markdown = lines.join("\n").trim().to_string();
    Ok(DeckMarkdown {
        html: render_markdown(&markdown),
        markdown,
    })
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

fn unordered_item(line: &str) -> Option<&str> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
}

fn ordered_item(line: &str) -> Option<&str> {
    let (number, content) = line.split_once(". ")?;
    (!number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit())).then_some(content)
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
        if chars[index] == '*'
            && chars.get(index + 1) == Some(&'*')
            && chars[index + 2..]
                .windows(2)
                .position(|pair| pair == ['*', '*'])
                .is_some()
        {
            let end = chars[index + 2..]
                .windows(2)
                .position(|pair| pair == ['*', '*'])
                .unwrap_or(0);
            let content: String = chars[index + 2..index + 2 + end].iter().collect();
            html.push_str("<strong>");
            html.push_str(&render_inline(&content));
            html.push_str("</strong>");
            index += end + 4;
            continue;
        }
        if matches!(chars[index], '*' | '_') {
            let marker = chars[index];
            if let Some(end) = chars[index + 1..].iter().position(|ch| *ch == marker) {
                let content: String = chars[index + 1..index + 1 + end].iter().collect();
                html.push_str("<em>");
                html.push_str(&render_inline(&content));
                html.push_str("</em>");
                index += end + 2;
                continue;
            }
        }
        push_escaped_char(&mut html, chars[index]);
        index += 1;
    }
    html
}

fn escape_html(input: &str) -> String {
    let mut escaped = String::new();
    for ch in input.chars() {
        push_escaped_char(&mut escaped, ch);
    }
    escaped
}

fn push_escaped_char(output: &mut String, ch: char) {
    match ch {
        '&' => output.push_str("&amp;"),
        '<' => output.push_str("&lt;"),
        '>' => output.push_str("&gt;"),
        '"' => output.push_str("&quot;"),
        '\'' => output.push_str("&#39;"),
        other => output.push(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_DECK: &str = r#"---
id: intro
title: "MeiLang 入门"
short_title: 入门
theme: presentation
canvas: 16:9
summary: 图原生演说
default_for_stage: true
---

# 同一套结构语言 {#slide-01-why}
@template(claim_evidence)
@chapter(动机)

## claim {#vp_claim}
驾驶舱与演说使用 *同一套* 结构语言。

- scene
- presentation

## evidence {#vp_evidence}
1. graph
2. runtime
"#;

    #[test]
    fn parses_full_deck_and_renders_safe_markdown() {
        let deck = parse_deck_source(VALID_DECK).expect("valid deck");
        assert_eq!(deck.frontmatter.id, "intro");
        assert_eq!(deck.frontmatter.short_title.as_deref(), Some("入门"));
        assert!(deck.frontmatter.default_for_stage);
        assert_eq!(deck.slides.len(), 1);
        let slide = &deck.slides[0];
        assert_eq!(slide.pattern, "claim_evidence");
        assert_eq!(slide.slots.len(), 2);
        assert!(slide.steps.is_empty());
        assert!(slide.slots[0].content.html.contains("<em>同一套</em>"));
        assert!(slide.slots[0].content.html.contains("<ul>"));
        assert!(slide.slots[1].content.html.contains("<ol>"));
        assert!(slide.caption.is_none());
    }

    #[test]
    fn parses_custom_source_with_fragment_into_ast() {
        let source = VALID_DECK.replace(
            "@chapter(动机)",
            "@chapter(动机)\n@source(custom/customer-slide.mei#customer_slide)",
        );
        // Clear slot bodies — @source forbids mixed Markdown content.
        let source = source
            .replace(
                "## claim {#vp_claim}\n驾驶舱与演说使用 *同一套* 结构语言。\n\n- scene\n- presentation\n\n",
                "## claim {#vp_claim}\n\n",
            )
            .replace("## evidence {#vp_evidence}\n1. graph\n2. runtime\n", "## evidence {#vp_evidence}\n");
        let deck = parse_deck_source(&source).expect("source with fragment");
        let src = deck.slides[0].source.as_ref().expect("source");
        assert_eq!(src.path, "custom/customer-slide.mei");
        assert_eq!(src.fragment, "customer_slide");
    }

    #[test]
    fn rejects_source_without_fragment() {
        let source = VALID_DECK.replace(
            "@chapter(动机)",
            "@chapter(动机)\n@source(custom/customer-slide.mei)",
        );
        let error = parse_deck_source(&source).expect_err("fragment required");
        assert!(error.to_string().contains("custom/*.mei#fragment"));
    }

    #[test]
    fn rejects_source_mixed_with_markdown_slots() {
        let source = VALID_DECK.replace(
            "@chapter(动机)",
            "@chapter(动机)\n@source(custom/customer-slide.mei#customer_slide)",
        );
        let error = parse_deck_source(&source).expect_err("mixed content");
        assert!(error.to_string().contains("deck_source_mixed_content"));
    }

    #[test]
    fn rejects_unknown_and_missing_slots_with_line() {
        let unknown = VALID_DECK.replace("## evidence", "## action");
        let error = parse_deck_source(&unknown).expect_err("unknown slot");
        assert_eq!(error.line, 21);
        assert!(error.to_string().contains("unknown slot `action`"));

        let missing =
            VALID_DECK.replace("\n## evidence {#vp_evidence}\n1. graph\n2. runtime\n", "\n");
        let error = parse_deck_source(&missing).expect_err("missing slot");
        assert!(error.to_string().contains("missing required slot"));
        assert!(error.to_string().contains("evidence"));
    }

    #[test]
    fn rejects_duplicate_slide_and_viewpoint_ids() {
        let duplicate_slide = format!(
            "{VALID_DECK}\n{}",
            &VALID_DECK[VALID_DECK.find("# 同").unwrap()..]
        );
        let error = parse_deck_source(&duplicate_slide).expect_err("duplicate slide");
        assert!(error.to_string().contains("duplicate slide id"));

        let duplicate_viewpoint = VALID_DECK.replace("{#vp_evidence}", "{#vp_claim}");
        let error = parse_deck_source(&duplicate_viewpoint).expect_err("duplicate viewpoint");
        assert!(error.to_string().contains("duplicate viewpoint id"));
    }

    #[test]
    fn rejects_unknown_pattern_illegal_id_and_forbidden_syntax() {
        let pattern = VALID_DECK.replace("claim_evidence", "two_columns");
        let error = parse_deck_source(&pattern).expect_err("pattern");
        assert!(error.to_string().contains("unknown slide pattern"));

        let id = VALID_DECK.replace("{#slide-01-why}", "{#01-why}");
        let error = parse_deck_source(&id).expect_err("id");
        assert!(error.to_string().contains("invalid slide id"));

        let html = VALID_DECK.replace("驾驶舱与演说", "<b>驾驶舱</b>与演说");
        let error = parse_deck_source(&html).expect_err("html");
        assert!(error.to_string().contains("JSX/HTML"));

        let directive = VALID_DECK.replace("@chapter(动机)", "@layout(grid)");
        let error = parse_deck_source(&directive).expect_err("legacy directive");
        assert!(error.to_string().contains("unknown or malformed directive"));
    }

    #[test]
    fn rejects_legacy_frontmatter_and_h2_without_viewpoint_id_with_hints() {
        let deck_id = VALID_DECK.replace("id: intro", "deck_id: intro");
        let error = parse_deck_source(&deck_id).expect_err("deck_id");
        let message = error.to_string();
        assert!(
            message.contains("unknown frontmatter field `deck_id`"),
            "{message}"
        );
        assert!(message.contains("use `id`"), "{message}");

        let default_template = VALID_DECK.replace(
            "default_for_stage: true",
            "default_template: claim_evidence",
        );
        let error = parse_deck_source(&default_template).expect_err("default_template");
        let message = error.to_string();
        assert!(
            message.contains("unknown frontmatter field `default_template`"),
            "{message}"
        );
        assert!(message.contains("use `default_for_stage`"), "{message}");

        let bare_h2 = VALID_DECK.replace("## claim {#vp_claim}", "## claim");
        let error = parse_deck_source(&bare_h2).expect_err("h2");
        assert!(error
            .to_string()
            .contains("expected `## slot-name {#viewpoint-id}`"));
    }

    #[test]
    fn rejects_narration_directives_in_deck() {
        for directive in [
            "@caption\ncaption\n@end",
            "@speaker_notes\nnotes\n@end",
            "@step(vp_claim)\nstep\n@end",
        ] {
            let source =
                VALID_DECK.replace("@chapter(动机)", &format!("@chapter(动机)\n{directive}"));
            let error = parse_deck_source(&source).expect_err("narration must be rejected");
            assert!(
                error
                    .to_string()
                    .contains(DECK_NARRATION_DIRECTIVE_FORBIDDEN),
                "{error}"
            );
        }
    }
}
