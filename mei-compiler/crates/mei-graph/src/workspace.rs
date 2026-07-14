use std::path::{Path, PathBuf};

use mei_syntax::parse_deck_source_file;
use mei_syntax::v2::parse_v2_source_file;
use mei_syntax::StageMdxError;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use walkdir::WalkDir;

use crate::deck::{deck_to_v2, DeckBuildError};
use crate::expand::expand_v2_file;
use crate::lower::{lower_v2_file, GraphBlock, GraphOutcome};
use crate::registry::{MacroRegistry, TemplateRoots};
use crate::stage_mdx::compile_cockpit_stage_file;
use crate::world_expand::{expand_world_v2_file, WorldContextCatalog, WorldExpandError};

#[derive(Debug, Error)]
pub enum CompileAppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error in {path}: {error}")]
    Parse {
        path: PathBuf,
        error: mei_syntax::V2ParseError,
    },
    #[error("{error}")]
    DeckParse { error: mei_syntax::DeckParseError },
    #[error("deck compile error in {path}:{line}: {message}")]
    DeckBuild {
        path: PathBuf,
        line: usize,
        message: String,
    },
    #[error("expand error in {path}: {error}")]
    Expand {
        path: PathBuf,
        error: crate::ExpandError,
    },
    #[error("world expand error in {path}: {error}")]
    WorldExpand {
        path: PathBuf,
        error: WorldExpandError,
    },
    #[error("lower error in {path}: {error}")]
    Lower {
        path: PathBuf,
        error: crate::LowerGraphError,
    },
    #[error("{0}")]
    Config(String),
    #[error(
        "presentation_dual_source_forbidden: presentation stage has two authoring sources: deck `{deck}` conflicts with `{existing}`"
    )]
    DeckSourceConflict { deck: PathBuf, existing: PathBuf },
    #[error(
        "presentation_dual_source_forbidden: legacy presentation authoring is forbidden at `{path}`; use `src/presentation/{{stage}}/{{stage}}.deck.mdx`"
    )]
    LegacyPresentationForbidden { path: PathBuf },
    #[error("{error}")]
    StageMdxParse { error: StageMdxError },
    #[error(
        "narration_aot_session_dual_source: cockpit stage `{stage_id}` has both `src/stage/{{id}}.stage.mdx` and default AOT `*.scene.mdx` at `{scene_mdx}`"
    )]
    NarrationAotSessionDualSource {
        stage_id: String,
        scene_mdx: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileOutcome {
    pub app_id: String,
    pub syntax_version: String,
    pub files: Vec<GraphOutcome>,
    pub blocks: Vec<GraphBlock>,
}

pub fn compile_app(workspace: &Path, app_id: &str) -> Result<CompileOutcome, CompileAppError> {
    let workspace_json = resolve_workspace_config_path(workspace);
    let ws_config: WorkspaceJson = serde_json::from_str(
        &std::fs::read_to_string(&workspace_json)
            .map_err(|e| CompileAppError::Config(format!("{}: {e}", workspace_json.display())))?,
    )
    .map_err(|e| CompileAppError::Config(format!("{}: {e}", workspace_json.display())))?;

    let templates_rel = ws_config
        .paths
        .and_then(|p| p.templates)
        .unwrap_or_else(|| "stock/templates".to_string());
    let stock_templates = workspace.join(&templates_rel);
    let app_root = workspace.join("apps").join(app_id);
    let src_root = app_root.join("src");
    if !src_root.is_dir() {
        return Err(CompileAppError::Config(format!(
            "missing app src: {}",
            src_root.display()
        )));
    }

    let syntax_version =
        read_syntax_version(&app_root).unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    let roots = TemplateRoots::from_app_and_stock(&app_root, stock_templates);
    let registry = MacroRegistry::load_layered(&roots)?;

    let mut files = Vec::new();
    let mut blocks = Vec::new();

    let mut mei_paths: Vec<PathBuf> = WalkDir::new(&src_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "mei"))
        .map(|entry| entry.into_path())
        .collect();
    mei_paths.sort();
    let mut deck_paths: Vec<PathBuf> = WalkDir::new(&src_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".deck.mdx"))
        })
        .map(|entry| entry.into_path())
        .collect();
    deck_paths.sort();

    let mut stage_mdx_paths: Vec<PathBuf> = WalkDir::new(&src_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".stage.mdx"))
        })
        .map(|entry| entry.into_path())
        .collect();
    stage_mdx_paths.sort();

    reject_legacy_presentation_authoring(&src_root)?;

    for path in &deck_paths {
        validate_deck_location(&src_root, path)?;
        reject_dual_presentation_source(path)?;
    }

    for path in &stage_mdx_paths {
        validate_stage_mdx_location(&src_root, path)?;
        reject_stage_mdx_scene_dual_source(&src_root, path)?;
    }

    // Old presentation deep trees are forbidden; never lower them as author input.
    mei_paths.retain(|path| !is_legacy_presentation_mei_path(&src_root, path));

    for path in &mei_paths {
        let rel = path
            .strip_prefix(&src_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let parsed = parse_v2_source_file(path).map_err(|error| CompileAppError::Parse {
            path: path.clone(),
            error,
        })?;
        let expanded = expand_v2_file(&parsed, &registry, &roots).map_err(|error| {
            CompileAppError::Expand {
                path: path.clone(),
                error,
            }
        })?;
        let expanded = if is_world_mei_path(path) {
            let catalog = WorldContextCatalog::load_from_app(&app_root);
            expand_world_v2_file(&expanded, &catalog).map_err(|error| {
                CompileAppError::WorldExpand {
                    path: path.clone(),
                    error,
                }
            })?
        } else {
            expanded
        };
        let outcome = lower_v2_file(&rel, &expanded).map_err(|error| CompileAppError::Lower {
            path: path.clone(),
            error,
        })?;
        blocks.extend(outcome.blocks.clone());
        files.push(outcome);
    }

    for path in &deck_paths {
        let rel = source_relative_path(&src_root, path);
        let deck =
            parse_deck_source_file(path).map_err(|error| CompileAppError::DeckParse { error })?;
        let parsed =
            deck_to_v2(app_id, &rel, &deck).map_err(|DeckBuildError { line, message }| {
                CompileAppError::DeckBuild {
                    path: path.clone(),
                    line,
                    message,
                }
            })?;
        let expanded = expand_v2_file(&parsed, &registry, &roots).map_err(|error| {
            CompileAppError::Expand {
                path: path.clone(),
                error,
            }
        })?;
        let outcome = lower_v2_file(&rel, &expanded).map_err(|error| CompileAppError::Lower {
            path: path.clone(),
            error,
        })?;
        blocks.extend(outcome.blocks.clone());
        files.push(outcome);
    }

    for path in &stage_mdx_paths {
        let rel = source_relative_path(&src_root, path);
        let outcome = compile_cockpit_stage_file(path, &rel)
            .map_err(|error| CompileAppError::StageMdxParse { error })?;
        blocks.extend(outcome.blocks.clone());
        files.push(outcome);
    }

    // 0119: synthesize access navigation from Stage MDX / Deck so authors need not
    // write sandwich navigation+assembly_ref for MCG graph closure.
    crate::stage_closure::synthesize_stage_access_navigation(&app_root, app_id, &mut blocks);

    files.sort_by(|a, b| a.source_file.cmp(&b.source_file));
    blocks.sort_by(|a, b| a.block_id.cmp(&b.block_id));

    Ok(CompileOutcome {
        app_id: app_id.to_string(),
        syntax_version,
        files,
        blocks,
    })
}

fn source_relative_path(src_root: &Path, path: &Path) -> String {
    path.strip_prefix(src_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn validate_deck_location(src_root: &Path, path: &Path) -> Result<(), CompileAppError> {
    let rel = path.strip_prefix(src_root).unwrap_or(path);
    let components: Vec<_> = rel.components().collect();
    let valid = components.len() == 3
        && components[0].as_os_str() == "presentation"
        && components[1].as_os_str().to_str().is_some_and(|stage| {
            components[2].as_os_str().to_str() == Some(&format!("{stage}.deck.mdx"))
        });
    if valid {
        Ok(())
    } else {
        Err(CompileAppError::Config(format!(
            "{}:1:1: deck path must be `src/presentation/{{stage}}/{{stage}}.deck.mdx`",
            path.display()
        )))
    }
}

/// Cockpit Stage MDX must live at `src/stage/{stage_id}.stage.mdx`.
fn validate_stage_mdx_location(src_root: &Path, path: &Path) -> Result<(), CompileAppError> {
    let rel = path.strip_prefix(src_root).unwrap_or(path);
    let components: Vec<_> = rel.components().collect();
    let valid = components.len() == 2
        && components[0].as_os_str() == "stage"
        && components[1]
            .as_os_str()
            .to_str()
            .is_some_and(|name| name.ends_with(".stage.mdx"));
    if valid {
        Ok(())
    } else {
        Err(CompileAppError::Config(format!(
            "{}:1:1: cockpit stage mdx path must be `src/stage/{{stage_id}}.stage.mdx`",
            path.display()
        )))
    }
}

/// When AOT Stage MDX exists, a same-stage `*.scene.mdx` with `default_for_stage: true` is dual-source.
fn reject_stage_mdx_scene_dual_source(
    src_root: &Path,
    stage_mdx_path: &Path,
) -> Result<(), CompileAppError> {
    let file_stem = stage_mdx_path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".stage.mdx"))
        .unwrap_or("");
    if file_stem.is_empty() {
        return Ok(());
    }
    let scene_mdx = src_root.join("scene").join(format!("{file_stem}.scene.mdx"));
    if !scene_mdx.is_file() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&scene_mdx).unwrap_or_default();
    let is_default_aot = raw.lines().any(|line| {
        let t = line.trim();
        t == "default_for_stage: true"
            || t == "default_for_stage:true"
            || t.eq_ignore_ascii_case("default_for_stage: yes")
    });
    if is_default_aot {
        return Err(CompileAppError::NarrationAotSessionDualSource {
            stage_id: file_stem.to_string(),
            scene_mdx,
        });
    }
    Ok(())
}

fn reject_legacy_presentation_authoring(src_root: &Path) -> Result<(), CompileAppError> {
    let presentation_root = src_root.join("presentation");
    if !presentation_root.is_dir() {
        return Ok(());
    }

    let mut stage_dirs: Vec<PathBuf> = fs_read_dir_dirs(&presentation_root)?;
    stage_dirs.sort();

    for stage_dir in stage_dirs {
        let Some(stage_id) = stage_dir
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if stage_id.starts_with('.') {
            continue;
        }

        let expected_deck = stage_dir.join(format!("{stage_id}.deck.mdx"));
        let stage_decks: Vec<PathBuf> = WalkDir::new(&stage_dir)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(".deck.mdx"))
            })
            .map(|entry| entry.into_path())
            .collect();

        if stage_decks.len() > 1 {
            let mut sorted = stage_decks;
            sorted.sort();
            return Err(CompileAppError::DeckSourceConflict {
                deck: sorted[0].clone(),
                existing: sorted[1].clone(),
            });
        }

        if let Some(deck) = stage_decks.first() {
            if deck != &expected_deck {
                return Err(CompileAppError::Config(format!(
                    "{}:1:1: deck path must be `src/presentation/{{stage}}/{{stage}}.deck.mdx`",
                    deck.display()
                )));
            }
            continue;
        }

        // No deck: legacy authoring alone is forbidden.
        if let Some(legacy) = find_legacy_presentation_artifact(&stage_dir) {
            return Err(CompileAppError::LegacyPresentationForbidden { path: legacy });
        }
    }
    Ok(())
}

fn reject_dual_presentation_source(deck_path: &Path) -> Result<(), CompileAppError> {
    let Some(stage_dir) = deck_path.parent() else {
        return Ok(());
    };
    for name in ["presentation.mei", "p.mei"] {
        let candidate = stage_dir.join(name);
        if candidate.is_file() {
            return Err(CompileAppError::DeckSourceConflict {
                deck: deck_path.to_path_buf(),
                existing: candidate,
            });
        }
    }
    if let Some(existing) = find_legacy_slide_tree(&stage_dir.join("p")) {
        return Err(CompileAppError::DeckSourceConflict {
            deck: deck_path.to_path_buf(),
            existing,
        });
    }
    if let Some(existing) = find_stage_presentation_mdx(stage_dir) {
        return Err(CompileAppError::DeckSourceConflict {
            deck: deck_path.to_path_buf(),
            existing,
        });
    }
    Ok(())
}

fn find_legacy_presentation_artifact(stage_dir: &Path) -> Option<PathBuf> {
    for name in ["presentation.mei", "p.mei"] {
        let candidate = stage_dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Some(slide) = find_legacy_slide_tree(&stage_dir.join("p")) {
        return Some(slide);
    }
    find_stage_presentation_mdx(stage_dir)
}

fn find_legacy_slide_tree(p_root: &Path) -> Option<PathBuf> {
    if !p_root.is_dir() {
        return None;
    }
    WalkDir::new(p_root)
        .min_depth(1)
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .path()
                .strip_prefix(p_root)
                .ok()
                .and_then(|path| path.components().next())
                .and_then(|component| component.as_os_str().to_str())
                .is_some_and(|name| name.starts_with("slide"))
        })
        .map(|entry| entry.into_path())
}

fn find_stage_presentation_mdx(stage_dir: &Path) -> Option<PathBuf> {
    WalkDir::new(stage_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".presentation.mdx"))
        })
        .map(|entry| entry.into_path())
}

fn is_legacy_presentation_mei_path(src_root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(src_root) else {
        return false;
    };
    let components: Vec<_> = rel.components().collect();
    if components.len() < 2 || components[0].as_os_str() != "presentation" {
        return false;
    }
    // Allow optional custom/*.mei under a presentation stage.
    if components
        .get(2)
        .and_then(|component| component.as_os_str().to_str())
        == Some("custom")
    {
        return false;
    }
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
    if file_name == "presentation.mei" || file_name == "p.mei" {
        return true;
    }
    components
        .get(2)
        .and_then(|component| component.as_os_str().to_str())
        .is_some_and(|name| name == "p")
}

fn fs_read_dir_dirs(root: &Path) -> Result<Vec<PathBuf>, CompileAppError> {
    let mut dirs = Vec::new();
    let read_dir = std::fs::read_dir(root)?;
    for entry in read_dir {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    Ok(dirs)
}

pub fn resolve_workspace_config_path(workspace: &Path) -> PathBuf {
    if let Ok(raw) = std::env::var("MEI_WORKSPACE_CONFIG") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if candidate.is_file() {
                return candidate;
            }
            let joined = workspace.join(&candidate);
            if joined.is_file() {
                return joined;
            }
        }
    }
    workspace.join("workspace.json")
}

fn is_world_mei_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".world.mei"))
}

fn read_syntax_version(app_root: &Path) -> Option<String> {
    let mei_lang = app_root.join("mei.lang.json");
    let raw = std::fs::read_to_string(mei_lang).ok()?;
    let parsed: MeiLangJson = serde_json::from_str(&raw).ok()?;
    parsed.syntax_version
}

#[derive(Deserialize)]
struct MeiLangJson {
    #[serde(rename = "syntaxVersion")]
    syntax_version: Option<String>,
}

#[derive(Deserialize)]
struct WorkspaceJson {
    paths: Option<PathsLite>,
}

#[derive(Deserialize)]
struct PathsLite {
    templates: Option<String>,
}
