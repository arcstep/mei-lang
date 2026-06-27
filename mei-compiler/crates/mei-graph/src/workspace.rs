use std::path::{Path, PathBuf};

use mei_syntax::v2::parse_v2_source_file;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use walkdir::WalkDir;

use crate::expand::expand_v2_file;
use crate::lower::{lower_v2_file, GraphBlock, GraphOutcome};
use crate::registry::MacroRegistry;

#[derive(Debug, Error)]
pub enum CompileAppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error in {path}: {error}")]
    Parse {
        path: PathBuf,
        error: mei_syntax::V2ParseError,
    },
    #[error("expand error in {path}: {error}")]
    Expand {
        path: PathBuf,
        error: crate::ExpandError,
    },
    #[error("lower error in {path}: {error}")]
    Lower {
        path: PathBuf,
        error: crate::LowerGraphError,
    },
    #[error("{0}")]
    Config(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileOutcome {
    pub app_id: String,
    pub syntax_version: String,
    pub files: Vec<GraphOutcome>,
    pub blocks: Vec<GraphBlock>,
}

pub fn compile_app(workspace: &Path, app_id: &str) -> Result<CompileOutcome, CompileAppError> {
    let workspace_json = workspace.join("workspace.json");
    let ws_config: WorkspaceJson = serde_json::from_str(&std::fs::read_to_string(&workspace_json)?)
        .map_err(|e| CompileAppError::Config(format!("workspace.json: {e}")))?;

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

    let syntax_version = read_syntax_version(&app_root).unwrap_or_else(|| "2.0.0".to_string());

    let registry = MacroRegistry::load_dir(&stock_templates)?;

    let mut files = Vec::new();
    let mut blocks = Vec::new();

    for entry in WalkDir::new(&src_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "mei"))
    {
        let path = entry.path();
        let rel = path
            .strip_prefix(&src_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        let parsed = parse_v2_source_file(path).map_err(|error| CompileAppError::Parse {
            path: path.to_path_buf(),
            error,
        })?;
        let expanded = expand_v2_file(&parsed, &registry, &stock_templates).map_err(|error| {
            CompileAppError::Expand {
                path: path.to_path_buf(),
                error,
            }
        })?;
        let outcome = lower_v2_file(&rel, &expanded).map_err(|error| CompileAppError::Lower {
            path: path.to_path_buf(),
            error,
        })?;
        blocks.extend(outcome.blocks.clone());
        files.push(outcome);
    }

    files.sort_by(|a, b| a.source_file.cmp(&b.source_file));
    blocks.sort_by(|a, b| a.block_id.cmp(&b.block_id));

    Ok(CompileOutcome {
        app_id: app_id.to_string(),
        syntax_version,
        files,
        blocks,
    })
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
