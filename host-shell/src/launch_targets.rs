//! Resolve which apps to autostart for `serve --app` / `--mode` / `--launch`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mei_host_core::{
    ensure_default_launch_config, launch_json_path, list_launch_configs, read_launch_config,
    resolve_default_launch, AppLaunchDocument, AppLaunchError,
};

use crate::cli::LaunchMode;
use crate::landing;

#[derive(Debug, Clone)]
pub struct LaunchTarget {
    pub app_id: String,
    pub document: AppLaunchDocument,
    /// When set, write ephemeral overlay `defaultMode` before start (unified mode).
    pub mode_override: Option<String>,
    /// When true (plain `--app` / `--launch`), clear any stale overlay before start.
    pub clear_overlay: bool,
}

/// Product CLI: one `--app` plus optional `--mode`.
pub fn collect_single_app_target(
    workspace: &Path,
    app_id: &str,
    mode: Option<&str>,
) -> anyhow::Result<LaunchTarget> {
    let app_id = normalize_app_id(app_id);
    anyhow::ensure!(!app_id.is_empty(), "--app requires a non-empty app id");
    let mode = mode
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(|m| m.to_ascii_lowercase());
    if let Some(mode) = mode.as_deref() {
        anyhow::ensure!(
            matches!(mode, "hot" | "lazy" | "frozen"),
            "--mode must be hot|lazy|frozen"
        );
    }
    let doc = load_or_ensure_launch(workspace, &app_id)?;
    let (mode_override, clear_overlay) = match mode {
        Some(mode) => (Some(mode), false),
        None => (None, true),
    };
    Ok(LaunchTarget {
        app_id,
        document: doc,
        mode_override,
        clear_overlay,
    })
}

fn normalize_app_id(raw: &str) -> String {
    raw.trim().trim_matches('/').to_string()
}

fn load_or_ensure_launch(workspace: &Path, app_id: &str) -> anyhow::Result<AppLaunchDocument> {
    match resolve_default_launch(workspace, app_id) {
        Ok(doc) => Ok(doc),
        Err(AppLaunchError::NotFound(_)) => {
            ensure_default_launch_config(workspace, app_id).map_err(|e| anyhow::anyhow!("{e}"))
        }
        Err(error) => Err(anyhow::anyhow!("{error}")),
    }
}

pub fn collect_serve_launch_targets(
    workspace: &Path,
    mode: LaunchMode,
    app_configs: &[PathBuf],
) -> anyhow::Result<Vec<LaunchTarget>> {
    if !app_configs.is_empty() {
        tracing::warn!(
            "serve --app-config is legacy; prefer --app [--mode] or --launch (single launch.json)"
        );
        let mut out = Vec::new();
        for path in app_configs {
            let abs = if path.is_absolute() {
                path.clone()
            } else {
                workspace.join(path)
            };
            let app_id = infer_app_id_from_launch_path(workspace, &abs)?;
            let rel = abs
                .strip_prefix(workspace)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| abs.display().to_string());
            let doc =
                read_launch_config(workspace, &app_id, &rel).map_err(|e| anyhow::anyhow!("{e}"))?;
            out.push(LaunchTarget {
                app_id,
                document: doc,
                mode_override: None,
                clear_overlay: false,
            });
        }
        return Ok(dedupe_by_app(out));
    }

    match mode {
        LaunchMode::None => Ok(Vec::new()),
        LaunchMode::All => {
            let apps = landing::discover_workspace_apps(workspace)?;
            let mut out = Vec::new();
            for app in apps {
                let doc = load_or_ensure_launch(workspace, &app.id)?;
                out.push(LaunchTarget {
                    app_id: app.id,
                    document: doc,
                    mode_override: None,
                    clear_overlay: true,
                });
            }
            Ok(out)
        }
    }
}

/// Merge CLI `--app` target over `--launch` / `--app-config` (CLI wins per app).
pub fn merge_launch_targets(base: Vec<LaunchTarget>, cli: Vec<LaunchTarget>) -> Vec<LaunchTarget> {
    if cli.is_empty() {
        return dedupe_by_app(base);
    }
    if base.is_empty() {
        return dedupe_by_app(cli);
    }
    let mut by_app: BTreeMap<String, LaunchTarget> = BTreeMap::new();
    for target in base {
        by_app.insert(target.app_id.clone(), target);
    }
    for target in cli {
        by_app.insert(target.app_id.clone(), target);
    }
    by_app.into_values().collect()
}

pub fn list_app_launch_rows(workspace: &Path) -> anyhow::Result<Vec<serde_json::Value>> {
    let apps = landing::discover_workspace_apps(workspace)?;
    let mut rows = Vec::new();
    for app in apps {
        let launches = list_launch_configs(workspace, &app.id).unwrap_or_default();
        rows.push(serde_json::json!({
            "appId": app.id,
            "launchPath": launch_json_path(workspace, &app.id)
                .strip_prefix(workspace)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| format!("apps/{}/launch.json", app.id)),
            "launches": launches,
        }));
    }
    Ok(rows)
}

fn infer_app_id_from_launch_path(workspace: &Path, path: &Path) -> anyhow::Result<String> {
    let rel = path.strip_prefix(workspace).unwrap_or(path).to_path_buf();
    let parts: Vec<_> = rel
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_string()))
        .collect();
    if parts.len() == 3 && parts[0] == "apps" && parts[2] == "launch.json" {
        return Ok(parts[1].clone());
    }
    if parts.len() >= 4 && parts[0] == "apps" && parts[2] == "launch" {
        return Ok(parts[1].clone());
    }
    let raw = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&raw)?;
    value
        .get("appId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("cannot infer app id from launch path {}", path.display()))
}

fn dedupe_by_app(targets: Vec<LaunchTarget>) -> Vec<LaunchTarget> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for target in targets {
        if seen.insert(target.app_id.clone()) {
            out.push(target);
        } else {
            tracing::warn!(
                app = %target.app_id,
                "duplicate autostart for same app; keeping first"
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn single_app_mode_override() {
        let dir = tempdir().unwrap();
        let workspace = dir.path();
        let app_root = workspace.join("apps/mini-data");
        std::fs::create_dir_all(app_root.join("src")).unwrap();
        std::fs::write(
            app_root.join("app.config.json"),
            r#"{"schemaVersion":"mei-app-config-v1","id":"mini-data"}"#,
        )
        .unwrap();
        std::fs::write(
            app_root.join("launch.json"),
            r#"{
              "schemaVersion":"mei-app-launch-v1",
              "appId":"mini-data",
              "runtimePlan":{"defaultMode":"frozen","apps":{}}
            }"#,
        )
        .unwrap();

        let follow = collect_single_app_target(workspace, "mini-data", None).unwrap();
        assert!(follow.clear_overlay);
        assert!(follow.mode_override.is_none());

        let lazy = collect_single_app_target(workspace, "mini-data", Some("lazy")).unwrap();
        assert_eq!(lazy.mode_override.as_deref(), Some("lazy"));
        assert!(!lazy.clear_overlay);
    }
}
