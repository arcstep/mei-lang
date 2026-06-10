use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use super::types::{
    MeiConfig, WorkspaceAuthConfig, WorkspaceComplianceConfig, WorkspaceConfig,
    WorkspacePathsConfig, WorkspaceProfile, DEFAULT_APP_ENTRY_MAIN, MEI_CONFIG_FILENAME,
};
use super::workspace_paths::{app_mei_config_path, workspace_config_path};

/// 仅认 app 根目录的 `.mei-config.json`，不再向上/向 segment 回退。
pub fn resolve_mei_config_path(app_root: &Path, _source_root: Option<&Path>) -> PathBuf {
    app_mei_config_path(app_root)
}

pub fn load_mei_config_for_app(app_root: &Path, source_root: Option<&Path>) -> MeiConfig {
    let path = resolve_mei_config_path(app_root, source_root);
    MeiConfig::load_or_default(&path)
}

/// 迁移窗口：优先 `.mei-workspace.json`，否则回退读取 segment 级旧 `.mei-config.json`。
pub fn load_workspace_config(segment_root: &Path) -> WorkspaceConfig {
    let modern = workspace_config_path(segment_root);
    if modern.is_file() {
        return WorkspaceConfig::load_or_default(&modern);
    }
    let legacy = segment_root.join(MEI_CONFIG_FILENAME);
    if legacy.is_file() {
        let legacy_app = MeiConfig::load_or_default(&legacy);
        return WorkspaceConfig {
            schema_version: legacy_app.schema_version,
            workspace: WorkspaceProfile::default(),
            paths: WorkspacePathsConfig::default(),
            discover: legacy_app.discover,
            menu: legacy_app.menu,
            runtime: legacy_app.runtime,
            warmup: Default::default(),
            compliance: WorkspaceComplianceConfig::default(),
            auth: WorkspaceAuthConfig::default(),
        };
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
    app_root.join(resolve_app_entry_main(app_root))
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
