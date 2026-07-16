//! Cockpit Stage MDX profile (`*.stage.mdx`).
//!
//! Provisional directives (Phase 4 freeze):
//! - `@scene(use="scene/home")`
//! - `@fill(slot="…", content="…")`
//! - shared narration: `@step(target)` / `@caption` / `@speaker_notes` with `@end`

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::core::{
    check_markdown_line, codes, find_frontmatter_end, looks_like_private_scene_path,
    markdown_from_lines, parse_directive_arg, parse_frontmatter_map, parse_named_directive_args,
    validate_id_token, StageMarkdown, StageMdxError,
};

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
    let allowed = ["stage_id", "profile", "title", "theme"];
    let values = parse_frontmatter_map(path, &lines[1..fm_end], 2, &allowed)?;
    let stage_id = values
        .get("stage_id")
        .map(|(v, _)| v.clone())
        .ok_or_else(|| {
            StageMdxError::new(path, 1, 1, codes::PARSE, "missing required frontmatter field `stage_id`")
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
                if !validate_id_token(target) {
                    return Err(StageMdxError::new(
                        self.path,
                        line,
                        1,
                        codes::PARSE,
                        format!("invalid @step target `{target}`"),
                    ));
                }
                self.index += 1;
                let step = self.collect_step(target, line, self.steps.len())?;
                self.steps.push(step);
                continue;
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
                "cockpit body must use `@scene` / `@fill` / `@step` directives",
            ));
        }
        Ok(())
    }

    fn collect_step(
        &mut self,
        target: &str,
        directive_line: usize,
        index: usize,
    ) -> Result<CockpitNarrationStep, StageMdxError> {
        let mut caption = None;
        let mut speaker_notes = None;
        while self.index < self.lines.len() {
            let trimmed = self.lines[self.index].trim();
            let line = self.index + 1;
            if trimmed == "@end" {
                self.index += 1;
                return Ok(CockpitNarrationStep {
                    id: format!("step-{index}-{target}"),
                    target: target.to_string(),
                    caption,
                    speaker_notes,
                    line: directive_line,
                });
            }
            if trimmed == "@caption" {
                if caption.is_some() {
                    return Err(StageMdxError::new(
                        self.path,
                        line,
                        1,
                        codes::PARSE,
                        "@caption may appear once per @step",
                    ));
                }
                self.index += 1;
                caption = Some(self.collect_markdown_block("@caption", line)?);
                continue;
            }
            if trimmed == "@speaker_notes" {
                if speaker_notes.is_some() {
                    return Err(StageMdxError::new(
                        self.path,
                        line,
                        1,
                        codes::PARSE,
                        "@speaker_notes may appear once per @step",
                    ));
                }
                self.index += 1;
                speaker_notes = Some(self.collect_markdown_block("@speaker_notes", line)?);
                continue;
            }
            if trimmed.is_empty() {
                self.index += 1;
                continue;
            }
            return Err(StageMdxError::new(
                self.path,
                line,
                1,
                codes::PARSE,
                format!("unexpected content inside @step; got `{trimmed}`"),
            ));
        }
        Err(StageMdxError::new(
            self.path,
            directive_line,
            1,
            codes::PARSE,
            "`@step` block is missing `@end`",
        ))
    }

    fn collect_markdown_block(
        &mut self,
        directive: &str,
        directive_line: usize,
    ) -> Result<StageMarkdown, StageMdxError> {
        let start = self.index;
        while self.index < self.lines.len() {
            let raw = self.lines[self.index];
            if raw.trim() == "@end" {
                for (offset, line) in self.lines[start..self.index].iter().enumerate() {
                    if let Err(kind) = check_markdown_line(line) {
                        return Err(StageMdxError::new(
                            self.path,
                            start + offset + 1,
                            1,
                            kind.code(),
                            kind.message(),
                        ));
                    }
                }
                let md = markdown_from_lines(&self.lines[start..self.index]);
                self.index += 1;
                return Ok(md);
            }
            self.index += 1;
        }
        Err(StageMdxError::new(
            self.path,
            directive_line,
            1,
            codes::PARSE,
            format!("`{directive}` block is missing `@end`"),
        ))
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
---

@scene(use="scene/home")
@fill(slot="mini-metric", content="mini-metric")
"#;

    #[test]
    fn parses_minimal_cockpit_stage() {
        let doc = parse_cockpit_stage_source(MINIMAL).expect("parse");
        assert_eq!(doc.frontmatter.stage_id, "home");
        assert_eq!(doc.scene_use, "scene/home");
        assert_eq!(doc.fills.len(), 1);
        assert_eq!(doc.fills[0].slot, "mini-metric");
        assert!(doc.steps.is_empty());
    }

    #[test]
    fn parses_narration_step() {
        let source = format!(
            "{MINIMAL}\n@step(mini-metric)\n@caption\nHello\n@end\n@speaker_notes\nNotes\n@end\n@end\n"
        );
        let doc = parse_cockpit_stage_source(&source).expect("parse");
        assert_eq!(doc.steps.len(), 1);
        assert_eq!(doc.steps[0].target, "mini-metric");
        assert!(doc.steps[0]
            .caption
            .as_ref()
            .is_some_and(|c| c.markdown.contains("Hello")));
    }

    #[test]
    fn rejects_jsx_in_caption() {
        let source = format!("{MINIMAL}\n@step(x)\n@caption\n<div/>\n@end\n@end\n");
        let err = parse_cockpit_stage_source(&source).expect_err("jsx");
        assert_eq!(err.code, codes::JSX_FORBIDDEN);
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
