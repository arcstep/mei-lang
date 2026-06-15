use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use super::io::load_workspace_config;
use super::authoring_policy::validate_authoring_policy;
use super::types::DEFAULT_STOCK_AUTHORING_REL;
use super::workspace_paths::resolve_workspace_path;

/// Workspace-local authoring helpers concatenated after kernel prelude.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthoringHelpers {
    pub prelude_suffix: String,
    pub fingerprint: String,
    pub public_functions: Vec<String>,
    pub source_files: Vec<String>,
}

fn configured_authoring_rel(cfg: &super::types::WorkspaceConfig) -> Option<&str> {
    cfg.paths
        .authoring
        .as_deref()
        .filter(|value| !value.trim().is_empty())
}

pub fn resolve_authoring_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    if let Some(rel) = configured_authoring_rel(&cfg) {
        let candidate = resolve_workspace_path(source_root, rel);
        if candidate.is_dir() {
            return candidate;
        }
    }
    resolve_workspace_path(source_root, DEFAULT_STOCK_AUTHORING_REL)
}

fn public_functions_from_source(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed.strip_prefix("def ")?;
            let name = rest.split('(').next()?.trim();
            if name.starts_with('_') || name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

fn collect_star_files(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("star") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn ensure_under_root(source_root: &Path, file: &Path) -> Result<()> {
    let source_root = source_root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", source_root.display()))?;
    let file = file
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", file.display()))?;
    if !file.starts_with(&source_root) {
        bail!(
            "authoring helper {} is outside workspace root {}",
            file.display(),
            source_root.display()
        );
    }
    Ok(())
}

/// Load workspace authoring helpers from `paths.authoring` or `.stock/authoring`.
pub fn resolve_authoring_helpers(source_root: &Path) -> Result<AuthoringHelpers> {
    let root = resolve_authoring_root(source_root);
    let files = collect_star_files(&root)?;
    if files.is_empty() {
        return Ok(AuthoringHelpers::default());
    }

    let mut prelude_suffix = String::new();
    let mut fingerprint_parts = Vec::new();
    let mut public_functions = Vec::new();
    let mut source_files = Vec::new();

    for file in files {
        ensure_under_root(source_root, &file)?;
        let source = fs::read_to_string(&file)
            .with_context(|| format!("failed to read authoring helper {}", file.display()))?;
        validate_authoring_policy(&source)
            .with_context(|| format!("authoring helper policy violation in {}", file.display()))?;
        if !prelude_suffix.is_empty() {
            prelude_suffix.push_str("\n\n");
        }
        prelude_suffix.push_str(source.trim());
        public_functions.extend(public_functions_from_source(&source));
        let rel = file
            .strip_prefix(source_root)
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| file.to_string_lossy().into_owned());
        source_files.push(rel.clone());
        let meta = fs::metadata(&file)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|value| value.as_millis())
            .unwrap_or(0);
        fingerprint_parts.push(format!("{rel}|{mtime}|{}", meta.len()));
    }

    public_functions.sort();
    public_functions.dedup();

    Ok(AuthoringHelpers {
        prelude_suffix,
        fingerprint: fingerprint_parts.join(";"),
        public_functions,
        source_files,
    })
}
