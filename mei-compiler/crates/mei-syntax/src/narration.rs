use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::stage_mdx::core::{check_markdown_line, find_frontmatter_end, unquote};

pub const NARRATION_PARSE: &str = "narration_parse";
pub const NARRATION_SOURCE_PATH_INVALID: &str = "narration_source_path_invalid";
pub const NARRATION_TRACK_ID_DUPLICATE: &str = "narration_track_id_duplicate";
pub const NARRATION_CUE_ID_DUPLICATE: &str = "narration_cue_id_duplicate";
pub const NARRATION_SOURCE_ANCHOR_MISSING: &str = "narration_source_anchor_missing";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrationTrackFile {
    pub frontmatter: NarrationTrackFrontmatter,
    pub cues: Vec<NarrationCue>,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrationTrackFrontmatter {
    pub id: String,
    pub title: String,
    pub scope: String,
    pub entry: Option<String>,
    pub default_for: Vec<String>,
    pub summary: Option<String>,
    pub default_timing_ms: Option<u64>,
    pub voice: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum NarrationTiming {
    Milliseconds(u64),
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrationCue {
    pub id: String,
    pub target_ref: String,
    pub body: Option<String>,
    pub caption: Option<String>,
    pub speaker_notes: Option<String>,
    pub timing: Option<NarrationTiming>,
    pub actions: Vec<String>,
    pub source_anchor: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{location}: [{code}] {message}")]
pub struct NarrationParseError {
    pub path: Option<PathBuf>,
    pub line: usize,
    pub column: usize,
    pub code: String,
    pub message: String,
    location: String,
}

impl NarrationParseError {
    fn new(
        path: Option<&Path>,
        line: usize,
        column: usize,
        code: &str,
        message: impl Into<String>,
    ) -> Self {
        let location = path
            .map(|path| format!("{}:{line}:{column}", path.display()))
            .unwrap_or_else(|| format!("line {line}, column {column}"));
        Self {
            path: path.map(Path::to_path_buf),
            line,
            column,
            code: code.to_string(),
            message: message.into(),
            location,
        }
    }
}

pub fn parse_narration_track_source(
    source: &str,
) -> Result<NarrationTrackFile, NarrationParseError> {
    parse_narration_track_at(None, source)
}

pub fn parse_narration_track_file(path: &Path) -> Result<NarrationTrackFile, NarrationParseError> {
    validate_track_source_path(path)?;
    let source = std::fs::read_to_string(path).map_err(|error| {
        NarrationParseError::new(
            Some(path),
            1,
            1,
            NARRATION_PARSE,
            format!("failed to read narration track: {error}"),
        )
    })?;
    let mut track = parse_narration_track_at(Some(path), &source)?;
    track.source_path = Some(path.display().to_string().replace('\\', "/"));
    Ok(track)
}

fn validate_track_source_path(path: &Path) -> Result<(), NarrationParseError> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let in_narration =
        normalized.contains("/src/narration/") || normalized.starts_with("src/narration/");
    if in_narration && normalized.ends_with(".track.mdx") {
        return Ok(());
    }
    Err(NarrationParseError::new(
        Some(path),
        1,
        1,
        NARRATION_SOURCE_PATH_INVALID,
        "narration tracks must be `src/narration/**/*.track.mdx`",
    ))
}

fn parse_narration_track_at(
    path: Option<&Path>,
    source: &str,
) -> Result<NarrationTrackFile, NarrationParseError> {
    let lines: Vec<&str> = source.lines().collect();
    let fm_end = find_frontmatter_end(path, &lines).map_err(|error| {
        NarrationParseError::new(
            path,
            error.line,
            error.column,
            NARRATION_PARSE,
            error.message,
        )
    })?;
    let frontmatter = parse_frontmatter(path, &lines[1..fm_end])?;
    let mut parser = TrackParser {
        path,
        lines: &lines,
        index: fm_end + 1,
        cue_targets: BTreeSet::new(),
    };
    let cues = parser.parse_body()?;
    if cues.is_empty() {
        return Err(NarrationParseError::new(
            path,
            fm_end + 2,
            1,
            NARRATION_PARSE,
            "narration track must contain at least one `@cue(...)`",
        ));
    }
    Ok(NarrationTrackFile {
        frontmatter,
        cues,
        source_path: None,
    })
}

fn parse_frontmatter(
    path: Option<&Path>,
    lines: &[&str],
) -> Result<NarrationTrackFrontmatter, NarrationParseError> {
    const ALLOWED: [&str; 8] = [
        "id",
        "title",
        "scope",
        "entry",
        "default_for",
        "summary",
        "default_timing_ms",
        "voice",
    ];
    let mut values = BTreeMap::<String, (String, usize)>::new();
    for (offset, raw) in lines.iter().enumerate() {
        let line = offset + 2;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            return Err(NarrationParseError::new(
                path,
                line,
                1,
                NARRATION_PARSE,
                "frontmatter entries must use `key: value`",
            ));
        };
        let key = key.trim();
        if !ALLOWED.contains(&key) {
            return Err(NarrationParseError::new(
                path,
                line,
                1,
                NARRATION_PARSE,
                format!("unknown narration frontmatter field `{key}`"),
            ));
        }
        let value = unquote(value.trim());
        if value.is_empty() {
            return Err(NarrationParseError::new(
                path,
                line,
                key.len() + 2,
                NARRATION_PARSE,
                format!("frontmatter field `{key}` cannot be empty"),
            ));
        }
        if values.insert(key.to_string(), (value, line)).is_some() {
            return Err(NarrationParseError::new(
                path,
                line,
                1,
                NARRATION_PARSE,
                format!("duplicate narration frontmatter field `{key}`"),
            ));
        }
    }
    let required = |key: &str| {
        values
            .get(key)
            .map(|(value, _)| value.clone())
            .ok_or_else(|| {
                NarrationParseError::new(
                    path,
                    1,
                    1,
                    NARRATION_PARSE,
                    format!("missing required frontmatter field `{key}`"),
                )
            })
    };
    let id = required("id")?;
    if !valid_id(&id) {
        return Err(NarrationParseError::new(
            path,
            values["id"].1,
            1,
            NARRATION_PARSE,
            format!("invalid narration track id `{id}`"),
        ));
    }
    let scope = required("scope")?;
    if scope != "app" {
        return Err(NarrationParseError::new(
            path,
            values["scope"].1,
            1,
            NARRATION_PARSE,
            "narration track `scope` must be `app`",
        ));
    }
    let default_for = match values.get("default_for") {
        Some((value, line)) => parse_string_list(path, *line, value)?,
        None => Vec::new(),
    };
    let entry = values
        .get("entry")
        .map(|(value, line)| {
            if valid_entry_ref(value) {
                Ok(value.clone())
            } else {
                Err(NarrationParseError::new(
                    path,
                    *line,
                    1,
                    NARRATION_PARSE,
                    format!("invalid narration entry reference `{value}`"),
                ))
            }
        })
        .transpose()?;
    let default_timing_ms = match values.get("default_timing_ms") {
        Some((value, line)) => Some(value.parse::<u64>().map_err(|_| {
            NarrationParseError::new(
                path,
                *line,
                1,
                NARRATION_PARSE,
                "`default_timing_ms` must be a positive integer",
            )
        })?),
        None => None,
    };
    if default_timing_ms == Some(0) {
        return Err(NarrationParseError::new(
            path,
            values["default_timing_ms"].1,
            1,
            NARRATION_PARSE,
            "`default_timing_ms` must be greater than zero",
        ));
    }
    Ok(NarrationTrackFrontmatter {
        id,
        title: required("title")?,
        scope,
        entry,
        default_for,
        summary: values.get("summary").map(|(value, _)| value.clone()),
        default_timing_ms,
        voice: values.get("voice").map(|(value, _)| value.clone()),
    })
}

fn parse_string_list(
    path: Option<&Path>,
    line: usize,
    value: &str,
) -> Result<Vec<String>, NarrationParseError> {
    let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) else {
        return Err(NarrationParseError::new(
            path,
            line,
            1,
            NARRATION_PARSE,
            "`default_for` must be an inline list",
        ));
    };
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for raw in inner.split(',') {
        let item = unquote(raw.trim());
        if item.is_empty() {
            return Err(NarrationParseError::new(
                path,
                line,
                1,
                NARRATION_PARSE,
                "`default_for` entries cannot be empty",
            ));
        }
        if !valid_entry_ref(&item) {
            return Err(NarrationParseError::new(
                path,
                line,
                1,
                NARRATION_PARSE,
                format!("invalid `default_for` entry `{item}`"),
            ));
        }
        out.push(item);
    }
    Ok(out)
}

struct TrackParser<'a> {
    path: Option<&'a Path>,
    lines: &'a [&'a str],
    index: usize,
    cue_targets: BTreeSet<String>,
}

impl TrackParser<'_> {
    fn parse_body(&mut self) -> Result<Vec<NarrationCue>, NarrationParseError> {
        let mut cues = Vec::new();
        while self.index < self.lines.len() {
            let raw = self.lines[self.index];
            let trimmed = raw.trim();
            let line = self.index + 1;
            if trimmed.is_empty() || is_h1(trimmed) {
                self.index += 1;
                continue;
            }
            if trimmed.starts_with('#') {
                return Err(NarrationParseError::new(
                    self.path,
                    line,
                    1,
                    NARRATION_PARSE,
                    "only H1 section headings are allowed outside cues",
                ));
            }
            let Some(target_ref) = directive_arg(trimmed, "@cue") else {
                return Err(NarrationParseError::new(
                    self.path,
                    line,
                    1,
                    NARRATION_PARSE,
                    "track body allows only H1 headings and `@cue(<fully-qualified-target>)`",
                ));
            };
            if target_ref.is_empty() || target_ref.chars().any(char::is_whitespace) {
                return Err(NarrationParseError::new(
                    self.path,
                    line,
                    1,
                    NARRATION_PARSE,
                    "`@cue` requires one fully-qualified target",
                ));
            }
            self.index += 1;
            let cue = self.collect_cue(target_ref, line, cues.len())?;
            if !self.cue_targets.insert(cue.id.clone()) {
                return Err(NarrationParseError::new(
                    self.path,
                    line,
                    1,
                    NARRATION_CUE_ID_DUPLICATE,
                    format!("duplicate narration cue id `{}`", cue.id),
                ));
            }
            cues.push(cue);
        }
        Ok(cues)
    }

    fn collect_cue(
        &mut self,
        target_ref: &str,
        start_line: usize,
        ordinal: usize,
    ) -> Result<NarrationCue, NarrationParseError> {
        let mut body_lines = Vec::new();
        let mut caption = None;
        let mut speaker_notes = None;
        let mut timing = None;
        let mut actions = Vec::new();
        while self.index < self.lines.len() {
            let raw = self.lines[self.index];
            let trimmed = raw.trim();
            let line = self.index + 1;
            if trimmed == "@end" {
                self.index += 1;
                let body = normalize_markdown(&body_lines);
                let path = self
                    .path
                    .map(|path| path.display().to_string().replace('\\', "/"))
                    .unwrap_or_else(|| "<memory>".to_string());
                return Ok(NarrationCue {
                    id: format!("cue-{:04}", ordinal + 1),
                    target_ref: target_ref.to_string(),
                    body,
                    caption,
                    speaker_notes,
                    timing,
                    actions,
                    source_anchor: format!("{path}:{start_line}-{line}#cue-{:04}", ordinal + 1),
                    line_start: start_line,
                    line_end: line,
                });
            }
            if trimmed == "@caption" {
                if caption.is_some() {
                    return Err(self.error(line, "@caption may appear once per cue"));
                }
                self.index += 1;
                caption = Some(self.collect_block("@caption", line, true)?);
                continue;
            }
            if trimmed == "@speaker_notes" {
                if speaker_notes.is_some() {
                    return Err(self.error(line, "@speaker_notes may appear once per cue"));
                }
                self.index += 1;
                speaker_notes = Some(self.collect_block("@speaker_notes", line, true)?);
                continue;
            }
            if let Some(raw_timing) = directive_arg(trimmed, "@timing") {
                if timing.is_some() {
                    return Err(self.error(line, "@timing may appear once per cue"));
                }
                timing = Some(if raw_timing == "manual" {
                    NarrationTiming::Manual
                } else {
                    let value = raw_timing.parse::<u64>().map_err(|_| {
                        self.error(
                            line,
                            "@timing expects a positive millisecond value or `manual`",
                        )
                    })?;
                    if value == 0 {
                        return Err(
                            self.error(line, "@timing milliseconds must be greater than zero")
                        );
                    }
                    NarrationTiming::Milliseconds(value)
                });
                self.index += 1;
                let contents = self.collect_block("@timing", line, false)?;
                if !contents.is_empty() {
                    return Err(self.error(line, "@timing block body must be empty"));
                }
                continue;
            }
            if trimmed == "@action" {
                self.index += 1;
                let action = self.collect_block("@action", line, false)?;
                if action.is_empty() {
                    return Err(self.error(line, "@action block cannot be empty"));
                }
                actions.push(action);
                continue;
            }
            if trimmed.starts_with('@') {
                return Err(self.error(
                    line,
                    format!("unknown or malformed cue directive `{trimmed}`"),
                ));
            }
            validate_markdown(self.path, line, raw)?;
            body_lines.push(raw);
            self.index += 1;
        }
        Err(self.error(start_line, "`@cue` block is missing `@end`"))
    }

    fn collect_block(
        &mut self,
        directive: &str,
        directive_line: usize,
        markdown: bool,
    ) -> Result<String, NarrationParseError> {
        let mut lines = Vec::new();
        while self.index < self.lines.len() {
            let raw = self.lines[self.index];
            if raw.trim() == "@end" {
                self.index += 1;
                return Ok(lines.join("\n").trim().to_string());
            }
            if raw.trim().starts_with('@') {
                return Err(self.error(
                    self.index + 1,
                    format!("directives cannot nest inside `{directive}`"),
                ));
            }
            if markdown {
                validate_markdown(self.path, self.index + 1, raw)?;
            }
            lines.push(raw);
            self.index += 1;
        }
        Err(self.error(
            directive_line,
            format!("`{directive}` block is missing `@end`"),
        ))
    }

    fn error(&self, line: usize, message: impl Into<String>) -> NarrationParseError {
        NarrationParseError::new(self.path, line, 1, NARRATION_PARSE, message)
    }
}

fn normalize_markdown(lines: &[&str]) -> Option<String> {
    let value = lines.join("\n").trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn validate_markdown(
    path: Option<&Path>,
    line: usize,
    raw: &str,
) -> Result<(), NarrationParseError> {
    check_markdown_line(raw)
        .map_err(|kind| NarrationParseError::new(path, line, 1, kind.code(), kind.message()))
}

fn is_h1(line: &str) -> bool {
    line.starts_with("# ") && !line.starts_with("##")
}

fn directive_arg<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    line.strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')
        .map(str::trim)
}

fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
}

fn valid_entry_ref(value: &str) -> bool {
    if let Some(stage) = value.strip_prefix("stage:") {
        return valid_id(stage);
    }
    let Some(admin) = value.strip_prefix("admin:") else {
        return false;
    };
    let mut parts = admin.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(resource), Some(module), None) if valid_id(resource) && valid_id(module)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"---
id: overview
title: Overview
scope: app
entry: stage:home
default_for: [stage:home, stage:slides]
summary: Cross profile
default_timing_ms: 8000
voice: zh-CN
---

# Opening

@cue(stage:home/viewpoint:warnings_main)
Read this aloud.
@caption
Visible caption.
@end
@speaker_notes
Private note.
@end
@timing(6000)
@end
@action
  highlight: stage:home/viewpoint:warnings_main
@end
@end

@cue(admin:demo/overview/document_anchor:basic)
@timing(manual)
@end
@end
"#;

    #[test]
    fn parses_frozen_track_surface_and_stable_anchors() {
        let track = parse_narration_track_source(VALID).expect("valid track");
        assert_eq!(track.frontmatter.scope, "app");
        assert_eq!(track.frontmatter.default_for.len(), 2);
        assert_eq!(track.cues.len(), 2);
        assert_eq!(
            track.cues[0].target_ref,
            "stage:home/viewpoint:warnings_main"
        );
        assert_eq!(track.cues[0].actions.len(), 1);
        assert!(track.cues[0].source_anchor.ends_with("#cue-0001"));
        assert_eq!(track.cues[1].timing, Some(NarrationTiming::Manual));
    }

    #[test]
    fn rejects_unknown_frontmatter_and_unclosed_cue() {
        let unknown = VALID.replace("voice: zh-CN", "profile: cockpit");
        let error = parse_narration_track_source(&unknown).expect_err("unknown field");
        assert_eq!(error.code, NARRATION_PARSE);
        assert!(error.message.contains("unknown narration frontmatter"));

        let unclosed = VALID.trim_end_matches("\n@end\n").to_string();
        let error = parse_narration_track_source(&unclosed).expect_err("unclosed cue");
        assert!(error.message.contains("missing `@end`"));
    }

    #[test]
    fn rejects_non_app_scope_and_nested_directives() {
        let scope = VALID.replace("scope: app", "scope: stage");
        let error = parse_narration_track_source(&scope).expect_err("scope");
        assert!(error.message.contains("must be `app`"));

        let nested = VALID.replace("Visible caption.", "@speaker_notes");
        let error = parse_narration_track_source(&nested).expect_err("nested");
        assert!(error.message.contains("cannot nest"));
    }
}
