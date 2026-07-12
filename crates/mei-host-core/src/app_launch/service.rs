use std::path::Path;

use sha2::{Digest, Sha256};

use super::paths::{
    app_launch_dir, default_launch_path, ensure_app_launch_dir, launch_config_path,
    resolve_launch_path,
};
use super::types::{AppLaunchConfig, AppLaunchDocument, AppLaunchSummary};
use crate::config::load_app_config;

#[derive(Debug, thiserror::Error)]
pub enum AppLaunchError {
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    InvalidJson(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
}

pub fn list_launch_configs(
    workspace: &Path,
    app_id: &str,
) -> Result<Vec<AppLaunchSummary>, AppLaunchError> {
    let dir = app_launch_dir(workspace, app_id);
    let default_id = load_default_launch_id(workspace, app_id);
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    let entries = std::fs::read_dir(&dir).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| AppLaunchError::Io(e.to_string()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() || !valid_launch_id(&id) {
            continue;
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| AppLaunchError::Io(e.to_string()))?;
        let revision = revision_hash(&raw);
        let display_name = serde_json::from_str::<AppLaunchConfig>(&raw)
            .ok()
            .and_then(|c| c.display_name);
        out.push(AppLaunchSummary {
            id: id.clone(),
            path: rel_display(workspace, &path),
            revision,
            display_name,
            is_default: default_id.as_deref() == Some(id.as_str()),
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub fn read_launch_config(
    workspace: &Path,
    app_id: &str,
    config_ref: &str,
) -> Result<AppLaunchDocument, AppLaunchError> {
    let path = resolve_launch_path(workspace, app_id, config_ref);
    if !path.is_file() {
        return Err(AppLaunchError::NotFound(format!(
            "launch config not found: {}",
            path.display()
        )));
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    let mut config: AppLaunchConfig =
        serde_json::from_str(&raw).map_err(|e| AppLaunchError::InvalidJson(e.to_string()))?;
    if config.app_id.trim().is_empty() {
        config.app_id = app_id.to_string();
    } else if config.app_id != app_id {
        return Err(AppLaunchError::Invalid(format!(
            "launch config appId `{}` does not match directory app `{}`",
            config.app_id, app_id
        )));
    }
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("default")
        .to_string();
    Ok(AppLaunchDocument {
        id,
        path: rel_display(workspace, &path),
        revision: revision_hash(&raw),
        config,
    })
}

/// Resolve the launch document used when starting without an explicit `--config`.
pub fn resolve_default_launch(
    workspace: &Path,
    app_id: &str,
) -> Result<AppLaunchDocument, AppLaunchError> {
    if let Some(id) = load_default_launch_id(workspace, app_id) {
        return read_launch_config(workspace, app_id, &id);
    }
    ensure_default_launch_config(workspace, app_id)
}

/// Materialize `launch/default.json` if missing, then read it.
pub fn ensure_default_launch_config(
    workspace: &Path,
    app_id: &str,
) -> Result<AppLaunchDocument, AppLaunchError> {
    ensure_app_launch_dir(workspace, app_id).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    let path = default_launch_path(workspace, app_id);
    if !path.is_file() {
        let cfg = AppLaunchConfig::default_for_app(app_id);
        write_launch_config(workspace, app_id, "default", &cfg)?;
    }
    read_launch_config(workspace, app_id, "default")
}

pub fn write_launch_config(
    workspace: &Path,
    app_id: &str,
    name: &str,
    config: &AppLaunchConfig,
) -> Result<AppLaunchDocument, AppLaunchError> {
    if !valid_launch_id(name) {
        return Err(AppLaunchError::Invalid(format!(
            "invalid launch config name: {name}"
        )));
    }
    if config.app_id != app_id {
        return Err(AppLaunchError::Invalid(
            "launch config appId must match app directory".to_string(),
        ));
    }
    ensure_app_launch_dir(workspace, app_id).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    let path = launch_config_path(workspace, app_id, name);
    let raw = serde_json::to_string_pretty(config)
        .map_err(|e| AppLaunchError::InvalidJson(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &raw).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    std::fs::rename(&tmp, &path).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    Ok(AppLaunchDocument {
        id: name.to_string(),
        path: rel_display(workspace, &path),
        revision: revision_hash(&raw),
        config: config.clone(),
    })
}

fn load_default_launch_id(workspace: &Path, app_id: &str) -> Option<String> {
    let app_root = mei_lang_kernel::resolve_app_root(workspace, app_id);
    let cfg = load_app_config(&app_root).ok()?;
    cfg.default_launch
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
}

fn valid_launch_id(id: &str) -> bool {
    let trimmed = id.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn revision_hash(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn rel_display(workspace: &Path, path: &Path) -> String {
    path.strip_prefix(workspace)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_and_list_default_launch() {
        let tmp = tempfile::tempdir().expect("tmp");
        let ws = tmp.path();
        std::fs::create_dir_all(ws.join("apps/demo")).expect("app");
        std::fs::write(
            ws.join("apps/demo/app.config.json"),
            r#"{"schemaVersion":1,"defaultLaunch":"default"}"#,
        )
        .expect("cfg");
        let doc = ensure_default_launch_config(ws, "demo").expect("default");
        assert_eq!(doc.id, "default");
        assert_eq!(doc.config.app_id, "demo");
        let list = list_launch_configs(ws, "demo").expect("list");
        assert_eq!(list.len(), 1);
        assert!(list[0].is_default);
    }
}
