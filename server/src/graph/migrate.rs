//! Legacy graph layout migration for 1.3.0 clean rebuild.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use mei_lang_kernel::{resolve_app_build_root, resolve_app_root, resolve_app_var_root};

pub struct GraphMigrateOptions {
    pub source_root: PathBuf,
    pub app_id: Option<String>,
    pub clean: bool,
}

pub struct GraphMigrateReport {
    pub apps: Vec<String>,
    pub removed_paths: Vec<String>,
}

pub fn run_graph_migrate(options: GraphMigrateOptions) -> anyhow::Result<GraphMigrateReport> {
    let app_ids = resolve_app_ids(options.source_root.as_path(), options.app_id.as_deref())?;
    let mut removed_paths = Vec::new();
    for app_id in &app_ids {
        if options.clean {
            removed_paths.extend(remove_legacy_app_artifacts(
                options.source_root.as_path(),
                app_id.as_str(),
            )?);
        }
    }
    Ok(GraphMigrateReport {
        apps: app_ids,
        removed_paths,
    })
}

fn resolve_app_ids(source_root: &Path, app_filter: Option<&str>) -> anyhow::Result<Vec<String>> {
    if let Some(app_id) = app_filter.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(vec![app_id.to_string()]);
    }
    let apps_dir = source_root.join("apps");
    let mut apps = Vec::new();
    if apps_dir.is_dir() {
        for entry in fs::read_dir(&apps_dir).with_context(|| format!("read {}", apps_dir.display()))? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                apps.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    apps.sort();
    Ok(apps)
}

fn remove_legacy_app_artifacts(source_root: &Path, app_id: &str) -> anyhow::Result<Vec<String>> {
    let app_root = resolve_app_root(source_root, app_id);
    let mut removed = Vec::new();
    let candidates = [
        resolve_app_build_root(app_root.as_path()).join("artifacts/compiled_app"),
        resolve_app_var_root(app_root.as_path()).join("eval-results/metric-response-index.json"),
        resolve_app_var_root(app_root.as_path()).join("eval-results/.metric-response-index.json"),
        source_root
            .join("runtime/platform/graphs")
            .join(app_id)
            .join("mcg-registry.json"),
        source_root
            .join("runtime/platform/graphs")
            .join(app_id)
            .join("mrg-registry.json"),
        source_root
            .join("runtime/platform/graphs")
            .join(app_id)
            .join("bridge.json"),
    ];
    for path in candidates {
        if remove_path_recursive(&path)? {
            removed.push(path.display().to_string());
        }
    }
    Ok(removed)
}

fn remove_path_recursive(path: &Path) -> anyhow::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    if path.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("remove dir {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("remove file {}", path.display()))?;
    }
    Ok(true)
}
