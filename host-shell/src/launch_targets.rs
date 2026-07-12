//! Resolve which app launch configs to start for `serve --launch` / `--app-config`.

use std::path::{Path, PathBuf};

use mei_host_core::{
    ensure_default_launch_config, list_launch_configs, load_app_config, read_launch_config,
    resolve_default_launch, AppLaunchDocument, AppLaunchError,
};
use mei_lang_kernel::resolve_app_root;

use crate::cli::LaunchMode;
use crate::landing;

#[derive(Debug, Clone)]
pub struct LaunchTarget {
    pub app_id: String,
    pub document: AppLaunchDocument,
}

pub fn collect_serve_launch_targets(
    workspace: &Path,
    mode: LaunchMode,
    app_configs: &[PathBuf],
) -> anyhow::Result<Vec<LaunchTarget>> {
    if !app_configs.is_empty() {
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
            });
        }
        return Ok(dedupe_by_app(out));
    }

    match mode {
        LaunchMode::None => Ok(Vec::new()),
        LaunchMode::Defaults => {
            let apps = landing::discover_workspace_apps(workspace)?;
            let mut out = Vec::new();
            for app in apps {
                let app_root = resolve_app_root(workspace, &app.id);
                let cfg = load_app_config(&app_root).unwrap_or_default();
                let Some(name) = cfg.default_launch.filter(|s| !s.trim().is_empty()) else {
                    continue;
                };
                match read_launch_config(workspace, &app.id, name.trim()) {
                    Ok(doc) => out.push(LaunchTarget {
                        app_id: app.id,
                        document: doc,
                    }),
                    Err(AppLaunchError::NotFound(_)) => {
                        tracing::warn!(
                            app = %app.id,
                            launch = %name,
                            "defaultLaunch points to missing file; skip"
                        );
                    }
                    Err(error) => return Err(anyhow::anyhow!("{error}")),
                }
            }
            Ok(out)
        }
        LaunchMode::All => {
            let apps = landing::discover_workspace_apps(workspace)?;
            let mut out = Vec::new();
            for app in apps {
                let doc = match resolve_default_launch(workspace, &app.id) {
                    Ok(doc) => doc,
                    Err(AppLaunchError::NotFound(_)) => {
                        ensure_default_launch_config(workspace, &app.id)
                            .map_err(|e| anyhow::anyhow!("{e}"))?
                    }
                    Err(error) => return Err(anyhow::anyhow!("{error}")),
                };
                out.push(LaunchTarget {
                    app_id: app.id,
                    document: doc,
                });
            }
            Ok(out)
        }
    }
}

pub fn list_app_launch_rows(workspace: &Path) -> anyhow::Result<Vec<serde_json::Value>> {
    let apps = landing::discover_workspace_apps(workspace)?;
    let mut rows = Vec::new();
    for app in apps {
        let app_root = resolve_app_root(workspace, &app.id);
        let cfg = load_app_config(&app_root).unwrap_or_default();
        let launches = list_launch_configs(workspace, &app.id).unwrap_or_default();
        rows.push(serde_json::json!({
            "appId": app.id,
            "defaultLaunch": cfg.default_launch,
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
    // apps/{app}/launch/{name}.json
    if parts.len() >= 4 && parts[0] == "apps" && parts[2] == "launch" {
        return Ok(parts[1].clone());
    }
    // Fallback: parse appId from JSON
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
                "duplicate --app-config for same app; keeping first"
            );
        }
    }
    out
}
