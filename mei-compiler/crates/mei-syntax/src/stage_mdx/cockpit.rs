//! Cockpit Stage MDX profile (`*.stage.mdx`).
//!
//! Frozen structural directives:
//! - `@scene(use="scene/home")`
//! - `@fill(slot="…", content="…")`

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::core::{
    codes, find_frontmatter_end, looks_like_private_scene_path, parse_directive_arg,
    parse_frontmatter_map, parse_named_directive_args, validate_id_token, StageMarkdown,
    StageMdxError,
};

pub const STAGE_NARRATION_DIRECTIVE_FORBIDDEN: &str = "stage_narration_directive_forbidden";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CockpitStageFile {
    pub frontmatter: CockpitFrontmatter,
    pub scene_use: String,
    pub fills: Vec<CockpitFill>,
    pub steps: Vec<CockpitNarrationStep>,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CockpitFrontmatter {
    pub stage_id: String,
    pub profile: String,
    pub title: Option<String>,
    pub short_title: Option<String>,
    pub theme: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CockpitFill {
    pub slot: String,
    pub content: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CockpitNarrationStep {
    pub id: String,
    pub target: String,
    pub caption: Option<StageMarkdown>,
    pub speaker_notes: Option<StageMarkdown>,
    pub line: usize,
}

pub fn parse_cockpit_stage_source(source: &str) -> Result<CockpitStageFile, StageMdxError> {
    parse_cockpit_stage_at(None, source)
}

pub fn parse_cockpit_stage_file(path: &Path) -> Result<CockpitStageFile, StageMdxError> {
    let source = std::fs::read_to_string(path).map_err(|e| StageMdxError::io(path, e))?;
    let mut doc = parse_cockpit_stage_at(Some(path), &source)?;
    doc.source_path = Some(path.display().to_string().replace('\\', "/"));
    Ok(doc)
}

fn parse_cockpit_stage_at(
    path: Option<&Path>,
    source: &str,
) -> Result<CockpitStageFile, StageMdxError> {
    let lines: Vec<&str> = source.lines().collect();
    let fm_end = find_frontmatter_end(path, &lines)?;
    let allowed = ["stage_id", "profile", "title", "short_title", "theme"];
    let values = parse_frontmatter_map(path, &lines[1..fm_end], 2, &allowed)?;
    let stage_id = values
        .get("stage_id")
        .map(|(v, _)| v.clone())
        .ok_or_else(|| {
            StageMdxError::new(
                path,
                1,
                1,
                codes::PARSE,
                "missing required frontmatter field `stage_id`",
            )
        })?;
    if !validate_id_token(&stage_id) {
        let line = values["stage_id"].1;
        return Err(StageMdxError::new(
            path,
            line,
            1,
            codes::PARSE,
            format!("invalid stage_id `{stage_id}`"),
        ));
    }
    let profile = values
        .get("profile")
        .map(|(v, _)| v.clone())
        .unwrap_or_else(|| "cockpit".to_string());
    let profile_norm = profile.to_ascii_lowercase();
    if profile_norm != "cockpit" && profile_norm != "page" {
        let line = values.get("profile").map(|(_, l)| *l).unwrap_or(1);
        return Err(StageMdxError::new(
            path,
            line,
            1,
            codes::PARSE,
            format!("stage mdx requires profile=cockpit|page, got `{profile}`"),
        ));
    }

    let mut parser = CockpitParser {
        path,
        lines: &lines,
        index: fm_end + 1,
        scene_use: None,
        fills: Vec::new(),
        steps: Vec::new(),
    };
    parser.parse_body()?;
    let scene_use = parser.scene_use.ok_or_else(|| {
        StageMdxError::new(
            path,
            fm_end + 2,
            1,
            codes::PARSE,
            "stage mdx requires `@scene(use=\"…\")`",
        )
    })?;

    Ok(CockpitStageFile {
        frontmatter: CockpitFrontmatter {
            stage_id,
            profile: profile_norm,
            title: values.get("title").map(|(v, _)| v.clone()),
            short_title: values.get("short_title").map(|(v, _)| v.clone()),
            theme: values.get("theme").map(|(v, _)| v.clone()),
        },
        scene_use,
        fills: parser.fills,
        steps: parser.steps,
        source_path: None,
    })
}

struct CockpitParser<'a> {
    path: Option<&'a Path>,
    lines: &'a [&'a str],
    index: usize,
    scene_use: Option<String>,
    fills: Vec<CockpitFill>,
    steps: Vec<CockpitNarrationStep>,
}

impl CockpitParser<'_> {
    fn parse_body(&mut self) -> Result<(), StageMdxError> {
        while self.index < self.lines.len() {
            let raw = self.lines[self.index];
            let trimmed = raw.trim();
            let line = self.index + 1;
            if trimmed.is_empty() {
                self.index += 1;
                continue;
            }
            if let Some(args) = parse_named_directive_args(trimmed, "@scene") {
                let use_path = args.get("use").cloned().ok_or_else(|| {
                    StageMdxError::new(
                        self.path,
                        line,
                        1,
                        codes::PARSE,
                        "`@scene` requires `use=\"…\"`",
                    )
                })?;
                if looks_like_private_scene_path(&use_path) {
                    return Err(StageMdxError::new(
                        self.path,
                        line,
                        1,
                        codes::PRIVATE_PATH,
                        format!("`@scene` must not reference private Scene path `{use_path}`"),
                    ));
                }
                if self.scene_use.replace(use_path).is_some() {
                    return Err(StageMdxError::new(
                        self.path,
                        line,
                        1,
                        codes::PARSE,
                        "`@scene` may be declared only once",
                    ));
                }
                self.index += 1;
                continue;
            }
            if let Some(args) = parse_named_directive_args(trimmed, "@fill") {
                let slot = args.get("slot").cloned().ok_or_else(|| {
                    StageMdxError::new(
                        self.path,
                        line,
                        1,
                        codes::PARSE,
                        "`@fill` requires `slot=\"…\"`",
                    )
                })?;
                let content = args.get("content").cloned().ok_or_else(|| {
                    StageMdxError::new(
                        self.path,
                        line,
                        1,
                        codes::PARSE,
                        "`@fill` requires `content=\"…\"`",
                    )
                })?;
                if looks_like_private_scene_path(&slot) || looks_like_private_scene_path(&content) {
                    return Err(StageMdxError::new(
                        self.path,
                        line,
                        1,
                        codes::PRIVATE_PATH,
                        "@fill must use public slot/content ids, not private Scene paths",
                    ));
                }
                self.fills.push(CockpitFill {
                    slot,
                    content,
                    line,
                });
                self.index += 1;
                continue;
            }
            if let Some(target) = parse_directive_arg(trimmed, "@step") {
                return Err(StageMdxError::new(
                    self.path,
                    line,
                    1,
                    STAGE_NARRATION_DIRECTIVE_FORBIDDEN,
                    format!(
                        "legacy Stage narration directive `@step({target})` is forbidden; use `src/narration/**/*.track.mdx`"
                    ),
                ));
            }
            if matches!(trimmed, "@caption" | "@speaker_notes")
                || trimmed.starts_with("@timing(")
                || trimmed == "@action"
            {
                return Err(StageMdxError::new(
                    self.path,
                    line,
                    1,
                    STAGE_NARRATION_DIRECTIVE_FORBIDDEN,
                    format!(
                        "legacy Stage narration directive `{trimmed}` is forbidden; use `src/narration/**/*.track.mdx`"
                    ),
                ));
            }
            if trimmed.starts_with('@') {
                return Err(StageMdxError::new(
                    self.path,
                    line,
                    1,
                    codes::PARSE,
                    format!("unknown or malformed directive `{trimmed}`"),
                ));
            }
            return Err(StageMdxError::new(
                self.path,
                line,
                1,
                codes::PARSE,
                "cockpit body must use `@scene` / `@fill` directives",
            ));
        }
        Ok(())
    }
}

/// Normalize `@scene(use=…)` to a scene assembly hint (`scene/home` → `home`).
pub fn scene_id_from_use(use_path: &str) -> String {
    let normalized = use_path.replace('\\', "/");
    normalized
        .trim_start_matches("src/")
        .trim_start_matches("scene/")
        .trim_end_matches(".mei")
        .rsplit('/')
        .next()
        .unwrap_or(&normalized)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"---
stage_id: home
profile: cockpit
title: Mini
short_title: M
---

@scene(use="scene/home")
@fill(slot="mini-metric", content="mini-metric")
"#;

    #[test]
    fn parses_minimal_cockpit_stage() {
        let doc = parse_cockpit_stage_source(MINIMAL).expect("parse");
        assert_eq!(doc.frontmatter.stage_id, "home");
        assert_eq!(doc.frontmatter.short_title.as_deref(), Some("M"));
        assert_eq!(doc.scene_use, "scene/home");
        assert_eq!(doc.fills.len(), 1);
        assert_eq!(doc.fills[0].slot, "mini-metric");
        assert!(doc.steps.is_empty());
    }

    #[test]
    fn rejects_legacy_narration_step() {
        let source = format!(
            "{MINIMAL}\n@step(mini-metric)\n@caption\nHello\n@end\n@speaker_notes\nNotes\n@end\n@end\n"
        );
        let error = parse_cockpit_stage_source(&source).expect_err("legacy narration");
        assert_eq!(error.code, STAGE_NARRATION_DIRECTIVE_FORBIDDEN);
    }

    #[test]
    fn rejects_caption_as_legacy_narration_before_markdown_parse() {
        let source = format!("{MINIMAL}\n@step(x)\n@caption\n<div/>\n@end\n@end\n");
        let err = parse_cockpit_stage_source(&source).expect_err("legacy narration");
        assert_eq!(err.code, STAGE_NARRATION_DIRECTIVE_FORBIDDEN);
    }

    #[test]
    fn rejects_private_scene_path() {
        let source = r#"---
stage_id: home
profile: cockpit
---
@scene(use="scene/home/t1/r-main")
"#;
        let err = parse_cockpit_stage_source(source).expect_err("private");
        assert_eq!(err.code, codes::PRIVATE_PATH);
    }
}
