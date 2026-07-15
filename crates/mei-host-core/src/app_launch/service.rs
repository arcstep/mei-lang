use std::path::Path;

use mei_lang_kernel::{load_app_manifest, AppManifest};
use sha2::{Digest, Sha256};

use super::paths::{ensure_app_launch_dir, launch_json_path, resolve_launch_path};
use super::types::{AppLaunchConfig, AppLaunchDocument, AppLaunchSummary};

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

fn app_root(workspace: &Path, app_id: &str) -> std::path::PathBuf {
    workspace.join("apps").join(app_id)
}

fn document_from_manifest(
    workspace: &Path,
    app_id: &str,
    manifest: &AppManifest,
) -> Result<AppLaunchDocument, AppLaunchError> {
    let value = manifest.to_launch_json_value(app_id);
    let mut config: AppLaunchConfig =
        serde_json::from_value(value).map_err(|e| AppLaunchError::InvalidJson(e.to_string()))?;
    if config.app_id.trim().is_empty() {
        config.app_id = app_id.to_string();
    } else if config.app_id != app_id {
        return Err(AppLaunchError::Invalid(format!(
            "launch config appId `{}` does not match directory app `{}`",
            config.app_id, app_id
        )));
    }
    let raw = serde_json::to_string(&config).unwrap_or_default();
    let path = manifest
        .source_path
        .clone()
        .unwrap_or_else(|| AppManifest::app_toml_path(&app_root(workspace, app_id)));
    Ok(AppLaunchDocument {
        id: "launch".to_string(),
        path: rel_display(workspace, &path),
        revision: revision_hash(manifest.source_raw.as_deref().unwrap_or(raw.as_str())),
        config,
    })
}

/// Phase 8.5: at most one launch document per app (`launch.json` or `app.toml`).
pub fn list_launch_configs(
    workspace: &Path,
    app_id: &str,
) -> Result<Vec<AppLaunchSummary>, AppLaunchError> {
    let root = app_root(workspace, app_id);
    if AppManifest::has_app_toml(&root) {
        let manifest = load_app_manifest(&root);
        let doc = document_from_manifest(workspace, app_id, &manifest)?;
        return Ok(vec![AppLaunchSummary {
            id: "launch".to_string(),
            path: doc.path,
            revision: doc.revision,
            display_name: doc.config.display_name,
            is_default: true,
        }]);
    }
    let path = launch_json_path(workspace, app_id);
    if !path.is_file() {
        migrate_legacy_launch_if_needed(workspace, app_id)?;
    }
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    let display_name = serde_json::from_str::<AppLaunchConfig>(&raw)
        .ok()
        .and_then(|c| c.display_name);
    Ok(vec![AppLaunchSummary {
        id: "launch".to_string(),
        path: rel_display(workspace, &path),
        revision: revision_hash(&raw),
        display_name,
        is_default: true,
    }])
}

pub fn read_launch_config(
    workspace: &Path,
    app_id: &str,
    config_ref: &str,
) -> Result<AppLaunchDocument, AppLaunchError> {
    let root = app_root(workspace, app_id);
    if AppManifest::has_app_toml(&root) {
        let manifest = load_app_manifest(&root);
        return document_from_manifest(workspace, app_id, &manifest);
    }
    migrate_legacy_launch_if_needed(workspace, app_id)?;
    let path = resolve_launch_path(workspace, app_id, config_ref);
    if !path.is_file() {
        return Err(AppLaunchError::NotFound(format!(
            "launch config not found: {}",
            path.display()
        )));
    }
    // Reject attempts to start a non-canonical named config that still exists as a file.
    let canonical = launch_json_path(workspace, app_id);
    if path != canonical {
        let trimmed = config_ref.trim();
        if !trimmed.is_empty()
            && trimmed != "default"
            && trimmed != "launch"
            && !trimmed.contains('/')
            && !trimmed.ends_with(".json")
        {
            return Err(AppLaunchError::Invalid(format!(
                "Phase 8.5: only apps/{{app}}/launch.json is allowed (got `{trimmed}`)"
            )));
        }
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
    Ok(AppLaunchDocument {
        id: "launch".to_string(),
        path: rel_display(workspace, &path),
        revision: revision_hash(&raw),
        config,
    })
}

/// Resolve the sole launch document (create `launch.json` if missing).
pub fn resolve_default_launch(
    workspace: &Path,
    app_id: &str,
) -> Result<AppLaunchDocument, AppLaunchError> {
    ensure_default_launch_config(workspace, app_id)
}

/// Materialize `launch.json` if missing (migrating from `launch/default.json` when present).
/// When `app.toml` exists, never write `launch.json` — project from the toml manifest.
pub fn ensure_default_launch_config(
    workspace: &Path,
    app_id: &str,
) -> Result<AppLaunchDocument, AppLaunchError> {
    let root = app_root(workspace, app_id);
    if AppManifest::has_app_toml(&root) {
        let manifest = load_app_manifest(&root);
        return document_from_manifest(workspace, app_id, &manifest);
    }
    ensure_app_launch_dir(workspace, app_id).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    migrate_legacy_launch_if_needed(workspace, app_id)?;
    let path = launch_json_path(workspace, app_id);
    if !path.is_file() {
        let cfg = AppLaunchConfig::default_for_app(app_id);
        write_launch_config(workspace, app_id, "launch", &cfg)?;
    }
    read_launch_config(workspace, app_id, "launch")
}

pub fn write_launch_config(
    workspace: &Path,
    app_id: &str,
    _name: &str,
    config: &AppLaunchConfig,
) -> Result<AppLaunchDocument, AppLaunchError> {
    if config.app_id != app_id {
        return Err(AppLaunchError::Invalid(
            "launch config appId must match app directory".to_string(),
        ));
    }
    ensure_app_launch_dir(workspace, app_id).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    let path = launch_json_path(workspace, app_id);
    let raw = serde_json::to_string_pretty(config)
        .map_err(|e| AppLaunchError::InvalidJson(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &raw).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    std::fs::rename(&tmp, &path).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    Ok(AppLaunchDocument {
        id: "launch".to_string(),
        path: rel_display(workspace, &path),
        revision: revision_hash(&raw),
        config: config.clone(),
    })
}

fn migrate_legacy_launch_if_needed(workspace: &Path, app_id: &str) -> Result<(), AppLaunchError> {
    let canonical = launch_json_path(workspace, app_id);
    if canonical.is_file() {
        return Ok(());
    }
    let legacy_default = resolve_app_root(workspace, app_id)
        .join("launch")
        .join("default.json");
    if !legacy_default.is_file() {
        return Ok(());
    }
    ensure_app_launch_dir(workspace, app_id).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    std::fs::copy(&legacy_default, &canonical).map_err(|e| AppLaunchError::Io(e.to_string()))?;
    Ok(())
}

fn resolve_app_root(workspace: &Path, app_id: &str) -> std::path::PathBuf {
    mei_lang_kernel::resolve_app_root(workspace, app_id)
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
    fn ensure_and_list_single_launch_json() {
        let tmp = tempfile::tempdir().expect("tmp");
        let ws = tmp.path();
        std::fs::create_dir_all(ws.join("apps/demo")).expect("app");
        std::fs::write(
            ws.join("apps/demo/app.config.json"),
            r#"{"schemaVersion":1}"#,
        )
        .expect("cfg");
        let doc = ensure_default_launch_config(ws, "demo").expect("launch");
        assert_eq!(doc.id, "launch");
        assert_eq!(doc.config.app_id, "demo");
        assert!(doc.path.ends_with("apps/demo/launch.json"));
        let list = list_launch_configs(ws, "demo").expect("list");
        assert_eq!(list.len(), 1);
        assert!(list[0].is_default);
    }

    #[test]
    fn migrates_legacy_launch_default_json() {
        let tmp = tempfile::tempdir().expect("tmp");
        let ws = tmp.path();
        let app = ws.join("apps/demo");
        std::fs::create_dir_all(app.join("launch")).expect("dir");
        let cfg = AppLaunchConfig::default_for_app("demo");
        std::fs::write(
            app.join("launch/default.json"),
            serde_json::to_string_pretty(&cfg).unwrap(),
        )
        .expect("legacy");
        let doc = ensure_default_launch_config(ws, "demo").expect("migrate");
        assert!(app.join("launch.json").is_file());
        assert_eq!(doc.config.app_id, "demo");
    }
}
