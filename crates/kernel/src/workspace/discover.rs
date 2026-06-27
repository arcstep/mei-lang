use std::{
    collections::HashSet,
    fs,
    path::Path,
};

use anyhow::{bail, Context, Result};

use crate::mei_config::{
    is_v2_app_root, load_workspace_config, resolve_apps_root,
};
use crate::model::WorkspaceAppMeta;

fn segment_discover_skip_dirs(segment_root: &Path) -> HashSet<String> {
    let mut out: HashSet<String> = ["node_modules", ".git", "target", "dist"]
        .into_iter()
        .map(str::to_string)
        .collect();
    let cfg = load_workspace_config(segment_root);
    for d in cfg.discover_skip_directories() {
        out.insert(d);
    }
    out
}

fn push_discovered_app(
    app_root: &Path,
    _source_root: &Path,
    apps: &mut Vec<WorkspaceAppMeta>,
) -> Result<()> {
    let id = app_root
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("discover_apps: app root has no directory name"))?
        .to_string();
    apps.push(WorkspaceAppMeta {
        id: id.clone(),
        title: id,
        root: app_root.to_string_lossy().to_string(),
    });
    Ok(())
}

/// 在 `root` 下发现 mei 应用（v2：`app.config.json` 或 `src/main.mei`）。
fn discover_apps_under(
    root: &Path,
    source_root: &Path,
    skip_dirs: &HashSet<String>,
    apps: &mut Vec<WorkspaceAppMeta>,
) -> Result<()> {
    if is_v2_app_root(root) {
        push_discovered_app(root, source_root, apps)?;
        return Ok(());
    }
    if !root.is_dir() {
        return Ok(());
    }
    for child in
        fs::read_dir(root).with_context(|| format!("discover_apps: read_dir {}", root.display()))?
    {
        let child = child.context("discover_apps: read_dir entry")?;
        if !child
            .file_type()
            .context("discover_apps: file_type")?
            .is_dir()
        {
            continue;
        }
        let name = child.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with('_') || skip_dirs.contains(&name) {
            continue;
        }
        discover_apps_under(&child.path(), source_root, skip_dirs, apps)?;
    }
    Ok(())
}

/// 在 `{workspace}/apps/` 下发现应用。
pub fn discover_apps(source_root: &Path) -> Result<Vec<WorkspaceAppMeta>> {
    let mut apps = Vec::new();
    if !source_root.is_dir() {
        bail!(
            "discover_apps: source_root `{}` is not a directory",
            source_root.display()
        );
    }
    let apps_root = resolve_apps_root(source_root);
    if !apps_root.is_dir() {
        return Ok(apps);
    }
    let skip_dirs = segment_discover_skip_dirs(source_root);
    for child in fs::read_dir(&apps_root)
        .with_context(|| format!("discover_apps: read_dir {}", apps_root.display()))?
    {
        let child = child.context("discover_apps: read_dir entry")?;
        if !child
            .file_type()
            .context("discover_apps: file_type")?
            .is_dir()
        {
            continue;
        }
        let name = child.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with('_') || skip_dirs.contains(&name) {
            continue;
        }
        discover_apps_under(&child.path(), source_root, &skip_dirs, &mut apps)?;
    }
    apps.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(apps)
}

fn stock_catalog_app_meta(source_root: &Path) -> Option<WorkspaceAppMeta> {
    use crate::catalog_app::stock_catalog_app_root;
    use crate::mei_config::stock_catalog_app_config;

    let app_root = stock_catalog_app_root(source_root);
    if !is_v2_app_root(app_root.as_path()) {
        return None;
    }
    let cfg = stock_catalog_app_config(source_root);
    Some(WorkspaceAppMeta {
        id: cfg.id,
        title: cfg.title,
        root: app_root.to_string_lossy().to_string(),
    })
}

fn perf_lab_app_meta(source_root: &Path) -> Option<WorkspaceAppMeta> {
    let app_root = crate::mei_config::resolve_app_root(source_root, "_perf-lab");
    if !is_v2_app_root(app_root.as_path()) {
        return None;
    }
    Some(WorkspaceAppMeta {
        id: "_perf-lab".to_string(),
        title: "Perf Lab".to_string(),
        root: app_root.to_string_lossy().to_string(),
    })
}

/// Discover apps for Build/manage surfaces, including hidden `_stock-catalog` when present.
pub fn discover_build_apps(source_root: &Path) -> Result<Vec<WorkspaceAppMeta>> {
    let mut apps = discover_apps(source_root)?;
    for hidden in [stock_catalog_app_meta(source_root), perf_lab_app_meta(source_root)]
        .into_iter()
        .flatten()
    {
        if !apps.iter().any(|app| app.id == hidden.id) {
            apps.push(hidden);
        }
    }
    apps.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(apps)
}
