use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use super::types::{MeiConfig, WorkspaceConfig, DEFAULT_APP_ENTRY_MAIN};
use super::workspace_paths::{app_mei_config_path, workspace_config_path};

/// 仅认 app 根目录的 `.mei-config.json`，不再向上/向 segment 回退。
pub fn resolve_mei_config_path(app_root: &Path, _source_root: Option<&Path>) -> PathBuf {
    app_mei_config_path(app_root)
}

pub fn load_mei_config_for_app(app_root: &Path, source_root: Option<&Path>) -> MeiConfig {
    let path = resolve_mei_config_path(app_root, source_root);
    let mut config = MeiConfig::load_or_default(&path);
    config.apply_profile_runtime_defaults();
    config
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
    let path = app_mei_config_path(app_root);
    if path.is_file() {
        MeiConfig::load_or_default(&path).entry.main_rel()
    } else {
        DEFAULT_APP_ENTRY_MAIN.to_string()
    }
}

pub fn resolve_app_main_path(app_root: &Path) -> PathBuf {
    let entry = resolve_app_entry_main(app_root);
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
