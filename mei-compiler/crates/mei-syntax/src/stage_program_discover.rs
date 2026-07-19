//! Discover Stage Program MDX files (0119): Registry from enumeration, not navigation pointers.
//! Shared by mei-compiler (graph closure) and mei-host-graph (assemble).

use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::parse_cockpit_stage_file;

/// One discovered Stage Program author file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredStageProgram {
    pub stage_id: String,
    pub profile: StageProgramProfile,
    /// Path relative to app root (forward slashes).
    pub program_rel: String,
    pub title: Option<String>,
    pub short_title: Option<String>,
    /// Assembly key for host assemble (`stage@src/...`).
    pub assembly_key: String,
    /// Target file for routes (scene .mei or deck path).
    pub target_file: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageProgramProfile {
    Cockpit,
    Slides,
    Page,
}

impl StageProgramProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cockpit => "cockpit",
            Self::Slides => "slides",
            Self::Page => "page",
        }
    }

    pub fn route_kind(self) -> &'static str {
        match self {
            Self::Cockpit => "scene",
            Self::Slides => "presentation",
            // Product page Stage (not T2 page_instance). Wire as document for Registry.
            Self::Page => "document",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cockpit" => Some(Self::Cockpit),
            "slides" | "presentation" => Some(Self::Slides),
            "page" => Some(Self::Page),
            _ => None,
        }
    }
}

/// Enumerate valid Stage Program files under an app root.
///
/// Recruits: `home.mdx`, `home.stage.mdx`, `*.stage.mdx`, `*.deck.mdx`
/// under the app root or `src/` (depth-limited walk).
pub fn discover_stage_programs(app_root: &Path) -> Vec<DiscoveredStageProgram> {
    let mut found: Vec<(PathBuf, String)> = Vec::new();
    let roots = [app_root.to_path_buf(), app_root.join("src")];
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&root)
            .max_depth(6)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let name = entry.file_name().to_string_lossy();
            let lower = name.to_ascii_lowercase();
            let ok = lower == "home.mdx"
                || lower.ends_with(".stage.mdx")
                || lower.ends_with(".deck.mdx");
            if !ok {
                continue;
            }
            // Skip drafts / fragments under underscore dirs.
            let rel_check = entry.path().strip_prefix(app_root).unwrap_or(entry.path());
            if rel_check.components().any(|c| {
                c.as_os_str()
                    .to_str()
                    .is_some_and(|s| s.starts_with('_') || s == "node_modules")
            }) {
                continue;
            }
            let abs = entry.path().to_path_buf();
            let rel = abs
                .strip_prefix(app_root)
                .unwrap_or(&abs)
                .to_string_lossy()
                .replace('\\', "/");
            if found.iter().any(|(_, r)| r == &rel) {
                continue;
            }
            found.push((abs, rel));
        }
    }
    found.sort_by(|a, b| a.1.cmp(&b.1));

    let mut out = Vec::new();
    for (abs, rel) in found {
        if let Some(prog) = parse_discovered(&abs, &rel) {
            out.push(prog);
        }
    }
    // Prefer home as default ordering: stable unique by stage_id (first wins).
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|p| seen.insert(p.stage_id.clone()));
    out
}

fn parse_discovered(abs: &Path, rel: &str) -> Option<DiscoveredStageProgram> {
    let lower = rel.to_ascii_lowercase();
    if lower.ends_with(".deck.mdx") {
        let stage_id = stage_id_from_path(rel, "deck.mdx");
        let assembly_key = format!("{stage_id}@{rel}");
        return Some(DiscoveredStageProgram {
            stage_id,
            profile: StageProgramProfile::Slides,
            program_rel: rel.to_string(),
            title: frontmatter_title(abs),
            short_title: frontmatter_value(abs, "short_title"),
            assembly_key,
            target_file: rel.to_string(),
        });
    }

    // cockpit: home.mdx / *.stage.mdx
    let Ok(doc) = parse_cockpit_stage_file(abs) else {
        // home.mdx without full cockpit frontmatter: treat as L0 shell stage.
        if lower.ends_with("home.mdx") || lower.ends_with("home.stage.mdx") {
            let stage_id = "home".to_string();
            let scene_rel = "src/scene/home.mei";
            return Some(DiscoveredStageProgram {
                stage_id: stage_id.clone(),
                profile: StageProgramProfile::Cockpit,
                program_rel: rel.to_string(),
                title: frontmatter_title(abs),
                short_title: frontmatter_value(abs, "short_title"),
                assembly_key: format!("{stage_id}@{scene_rel}"),
                target_file: scene_rel.to_string(),
            });
        }
        return None;
    };
    let stage_id = if doc.frontmatter.stage_id.trim().is_empty() {
        stage_id_from_path(rel, "stage.mdx")
    } else {
        doc.frontmatter.stage_id.clone()
    };
    let profile = StageProgramProfile::parse(&doc.frontmatter.profile)
        .unwrap_or(StageProgramProfile::Cockpit);
    let scene_rel = scene_use_to_target(&doc.scene_use);
    let assembly_key = format!("{stage_id}@{scene_rel}");
    Some(DiscoveredStageProgram {
        stage_id,
        profile,
        program_rel: rel.to_string(),
        title: doc.frontmatter.title.or_else(|| frontmatter_title(abs)),
        short_title: doc.frontmatter.short_title,
        assembly_key,
        target_file: scene_rel,
    })
}

fn stage_id_from_path(rel: &str, suffix: &str) -> String {
    let name = Path::new(rel)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("home");
    let stem = name
        .strip_suffix(&format!(".{suffix}"))
        .or_else(|| name.strip_suffix(".mdx"))
        .unwrap_or(name);
    let stem = stem.strip_suffix(".stage").unwrap_or(stem);
    let stem = stem.strip_suffix(".deck").unwrap_or(stem);
    if stem.is_empty() {
        "home".to_string()
    } else {
        stem.to_string()
    }
}

/// `@scene(use="scene/home")` → `src/scene/home.mei`
pub fn scene_use_to_target(scene_use: &str) -> String {
    let trimmed = scene_use.trim().trim_matches('"').replace('\\', "/");
    let trimmed = trimmed.trim_start_matches('/');
    if trimmed.is_empty() {
        return "src/scene/home.mei".to_string();
    }
    if trimmed.ends_with(".mei") {
        if trimmed.starts_with("src/") {
            return trimmed.to_string();
        }
        return format!("src/{trimmed}");
    }
    if trimmed.starts_with("src/") {
        format!("{trimmed}.mei")
    } else {
        format!("src/{trimmed}.mei")
    }
}

fn frontmatter_title(path: &Path) -> Option<String> {
    frontmatter_value(path, "title")
}

fn frontmatter_value(path: &Path, key: &str) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let mut lines = raw.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        if k.trim() == key {
            let t = v.trim().trim_matches('"').trim_matches('\'');
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

pub fn discover_program_for_stage<'a>(
    programs: &'a [DiscoveredStageProgram],
    stage_id: &str,
) -> Option<&'a DiscoveredStageProgram> {
    programs.iter().find(|p| p.stage_id == stage_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scene_use_normalizes_to_src_mei() {
        assert_eq!(scene_use_to_target("scene/home"), "src/scene/home.mei");
        assert_eq!(
            scene_use_to_target("src/scene/home.mei"),
            "src/scene/home.mei"
        );
    }

    #[test]
    fn discovers_stage_mdx_and_deck() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path();
        fs::create_dir_all(app.join("src/stage")).unwrap();
        fs::create_dir_all(app.join("src/presentation/sup")).unwrap();
        fs::write(
            app.join("src/stage/home.stage.mdx"),
            r#"---
stage_id: home
profile: cockpit
title: Home
---
@scene(use="scene/home")
"#,
        )
        .unwrap();
        fs::write(
            app.join("src/presentation/sup/supervision.deck.mdx"),
            "---\ntitle: Sup\n---\n# Slide\n",
        )
        .unwrap();
        let programs = discover_stage_programs(app);
        assert_eq!(programs.len(), 2);
        let home = programs.iter().find(|p| p.stage_id == "home").unwrap();
        assert_eq!(home.target_file, "src/scene/home.mei");
        assert_eq!(home.assembly_key, "home@src/scene/home.mei");
        let deck = programs
            .iter()
            .find(|p| p.stage_id == "supervision")
            .unwrap();
        assert_eq!(deck.profile, StageProgramProfile::Slides);
    }
}
