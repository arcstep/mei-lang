use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use super::types::{MeiConfig, WorkspaceConfig, DEFAULT_APP_ENTRY_MAIN};
use super::workspace_paths::{app_mei_config_path, workspace_config_path};

/// Prefer `app.toml` when present (Wave D); else `app.config.json`.
pub fn resolve_mei_config_path(app_root: &Path, _source_root: Option<&Path>) -> PathBuf {
    let toml = app_root.join(super::types::APP_TOML_FILENAME);
    if toml.is_file() {
        return toml;
    }
    app_mei_config_path(app_root)
}

pub fn load_mei_config_for_app(app_root: &Path, _source_root: Option<&Path>) -> MeiConfig {
    super::load_app_manifest(app_root).to_mei_config()
}

/// 读取 `{workspace}/workspace.json`。
pub fn load_workspace_config(segment_root: &Path) -> WorkspaceConfig {
    let path = workspace_config_path(segment_root);
    if path.is_file() {
        return WorkspaceConfig::load_or_default(&path);
    }
    WorkspaceConfig::default()
}

pub fn resolve_app_entry_main(app_root: &Path) -> String {
    let config = load_mei_config_for_app(app_root, None);
    let has_toml = app_root
        .join(super::types::APP_TOML_FILENAME)
        .is_file();
    let configured = config.entry.main.trim();
    let entry = if configured.is_empty() {
        if has_toml {
            // Graph-native product apps: app.toml is the root; no Mei entry file.
            String::new()
        } else {
            DEFAULT_APP_ENTRY_MAIN.to_string()
        }
    } else {
        config.entry.main_rel()
    };
    if entry.is_empty() {
        return entry;
    }
    let path = resolve_app_main_path_for_entry(app_root, entry.as_str());
    if path.is_file() {
        entry
    } else if has_toml {
        // Stale entry.main pointing at a missing file — treat as graph-native.
        String::new()
    } else {
        entry
    }
}

pub fn resolve_app_main_path(app_root: &Path) -> PathBuf {
    let entry = resolve_app_entry_main(app_root);
    if entry.trim().is_empty() {
        // Sentinel path that does not exist; callers must check `is_file()` / IO errors.
        return super::workspace_paths::resolve_app_src_root(app_root).join("__no_app_entry__.mei");
    }
    resolve_app_main_path_for_entry(app_root, entry.as_str())
}

fn resolve_app_main_path_for_entry(app_root: &Path, entry: &str) -> PathBuf {
    let normalized = entry.trim().trim_start_matches("./").replace('\\', "/");
    // `app.config.json` may use app-root-relative (`src/app.mei`) or src-relative (`app.mei` / `main.mei`).
    let from_app_root = app_root.join(normalized.as_str());
    if from_app_root.is_file() {
        return from_app_root;
    }
    let under_src_rel = normalized
        .strip_prefix("src/")
        .unwrap_or(normalized.as_str());
    let from_src = super::workspace_paths::resolve_app_src_root(app_root).join(under_src_rel);
    if from_src.is_file() {
        return from_src;
    }
    // Prefer the historical src-relative join when neither candidate exists yet.
    from_src
}

pub fn write_mei_config(path: &Path, config: &MeiConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create mei config parent dir {}",
                parent.display()
            )
        })?;
    }
    let raw = serde_json::to_string_pretty(config).context("failed to serialize mei config")?;
    write_string_atomically(path, raw.as_str())
        .with_context(|| format!("failed to write mei config {}", path.display()))
}

pub fn write_workspace_config(path: &Path, config: &WorkspaceConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create workspace config parent dir {}",
                parent.display()
            )
        })?;
    }
    let raw =
        serde_json::to_string_pretty(config).context("failed to serialize workspace config")?;
    write_string_atomically(path, raw.as_str())
        .with_context(|| format!("failed to write workspace config {}", path.display()))
}

pub(crate) fn write_string_atomically(path: &Path, raw: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path {} has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.json");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_path = parent.join(format!(".{file_name}.tmp-{}-{stamp}", std::process::id()));
    fs::write(&tmp_path, raw)
        .with_context(|| format!("failed to write temporary file {}", tmp_path.display()))?;
    if let Err(error) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(error).with_context(|| {
            format!(
                "failed to atomically replace {} from {}",
                path.display(),
                tmp_path.display()
            )
        });
    }
    Ok(())
}
