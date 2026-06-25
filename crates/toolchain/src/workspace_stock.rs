use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use mei_lang_kernel::{
    resolve_toolchain_root, resolve_workspace_runtime_root, stock_authoring_source,
    stock_components_source, stock_templates_source, workspace_config_path, write_workspace_config, APP_CONFIG_FILENAME,
    WorkspaceConfig, WorkspacePathsConfig, WorkspaceProfile, DEFAULT_APPS_REL,
    DEFAULT_STOCK_AUTHORING_REL, DEFAULT_STOCK_COMPONENTS_REL, DEFAULT_STOCK_TEMPLATES_REL,
    WORKSPACE_HOSTS_DIR_REL,
};
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct MaterializeReport {
    pub source_root: String,
    pub components: MaterializeDirReport,
    pub templates: MaterializeDirReport,
    pub authoring: MaterializeDirReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaterializeDirReport {
    pub from: String,
    pub to: String,
    pub copied_files: usize,
    pub skipped_files: usize,
    pub overwritten_files: usize,
}

pub fn materialize_workspace_stock(
    source_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<MaterializeReport> {
    fs::create_dir_all(source_root.join(WORKSPACE_HOSTS_DIR_REL))
        .context("create workspace host-state dir")?;
    let components = materialize_tree(
        &stock_components_source(package_root),
        &source_root.join(DEFAULT_STOCK_COMPONENTS_REL),
        force,
    )?;
    let templates = materialize_tree(
        &stock_templates_source(package_root),
        &source_root.join(DEFAULT_STOCK_TEMPLATES_REL),
        force,
    )?;
    let authoring = materialize_tree(
        &stock_authoring_source(package_root),
        &source_root.join(DEFAULT_STOCK_AUTHORING_REL),
        force,
    )?;
    Ok(MaterializeReport {
        source_root: source_root.display().to_string(),
        components,
        templates,
        authoring,
    })
}

/// Idempotent stock bootstrap: copy platform `stock/*` into the workspace when trees are missing.
/// Called from workspace init, runtime install, prebuild, and host startup — not a standalone user workflow.
pub fn ensure_workspace_stock_materialized(
    source_root: &Path,
    package_root: &Path,
) -> Result<Option<MaterializeReport>> {
    if !workspace_stock_needs_materialize(source_root) {
        return Ok(None);
    }
    Ok(Some(materialize_workspace_stock(
        source_root,
        package_root,
        false,
    )?))
}

fn workspace_stock_needs_materialize(source_root: &Path) -> bool {
    !stock_tree_ready(&source_root.join(DEFAULT_STOCK_AUTHORING_REL))
        || !stock_tree_ready(&source_root.join(DEFAULT_STOCK_COMPONENTS_REL))
        || !stock_tree_ready(&source_root.join(DEFAULT_STOCK_TEMPLATES_REL))
}

fn stock_tree_ready(path: &Path) -> bool {
    path.is_dir()
        && fs::read_dir(path)
            .ok()
            .and_then(|mut entries| entries.next())
            .is_some()
}

fn materialize_tree(from: &Path, to: &Path, force: bool) -> Result<MaterializeDirReport> {
    if !from.is_dir() {
        anyhow::bail!(
            "stock source `{}` is missing; ensure mei-lang ships stock/components and stock/templates",
            from.display()
        );
    }
    let mut copied_files = 0usize;
    let mut skipped_files = 0usize;
    let mut overwritten_files = 0usize;
    for entry in WalkDir::new(from).into_iter().filter_map(Result::ok) {
        let src = entry.path();
        let rel = src
            .strip_prefix(from)
            .context("strip stock prefix")?
            .to_path_buf();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = to.join(&rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest).with_context(|| format!("create dir {}", dest.display()))?;
            continue;
        }
        if dest.exists() && !force {
            skipped_files += 1;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create parent {}", parent.display()))?;
        }
        let existed = dest.exists();
        fs::copy(src, &dest)
            .with_context(|| format!("copy stock file {} -> {}", src.display(), dest.display()))?;
        copied_files += 1;
        if existed {
            overwritten_files += 1;
        }
    }
    Ok(MaterializeDirReport {
        from: from.display().to_string(),
        to: to.display().to_string(),
        copied_files,
        skipped_files,
        overwritten_files,
    })
}

pub fn init_workspace_profile(
    parent: &Path,
    profile_id: &str,
    label: Option<&str>,
    package_root: &Path,
) -> Result<PathBuf> {
    let profile_id = profile_id.trim();
    if profile_id.is_empty() || profile_id.starts_with('.') {
        anyhow::bail!("workspace profile id must be a non-hidden directory name");
    }
    let source_root = parent.join(profile_id);
    if source_root.exists() && !source_root.is_dir() {
        anyhow::bail!("`{}` exists and is not a directory", source_root.display());
    }
    fs::create_dir_all(&source_root)
        .with_context(|| format!("create workspace profile {}", source_root.display()))?;
    fs::create_dir_all(source_root.join(WORKSPACE_HOSTS_DIR_REL))
        .context("create workspace host-state dir")?;
    fs::create_dir_all(source_root.join("apps")).context("create workspace apps dir")?;
    fs::create_dir_all(source_root.join("deploy")).context("create workspace deploy dir")?;
    fs::create_dir_all(resolve_toolchain_root(&source_root).join("bin"))
        .context("create workspace toolchain bin dir")?;
    fs::create_dir_all(resolve_workspace_runtime_root(&source_root).join("platform"))
        .context("create workspace runtime platform dir")?;

    let config_path = workspace_config_path(&source_root);
    if !config_path.is_file() {
        let config = WorkspaceConfig {
            schema_version: 2,
            workspace: WorkspaceProfile {
                id: Some(profile_id.to_string()),
                label: label.map(str::to_string),
                deploy_host: None,
                default_app: None,
            },
            paths: WorkspacePathsConfig {
                apps: Some(DEFAULT_APPS_REL.to_string()),
                components: Some(DEFAULT_STOCK_COMPONENTS_REL.to_string()),
                templates: Some(DEFAULT_STOCK_TEMPLATES_REL.to_string()),
                authoring: Some(DEFAULT_STOCK_AUTHORING_REL.to_string()),
                ..WorkspacePathsConfig::default()
            },
            ..WorkspaceConfig::default()
        };
        write_workspace_config(&config_path, &config)?;
    }
    ensure_workspace_stock_materialized(&source_root, package_root)?;
    Ok(source_root)
}

pub fn create_app_skeleton(source_root: &Path, app_id: &str) -> Result<PathBuf> {
    let app_id = app_id.trim();
    if app_id.is_empty() || app_id.starts_with('.') || app_id.starts_with('_') {
        anyhow::bail!("app id must be a plain directory name");
    }
    let app_root = source_root.join("apps").join(app_id);
    if app_root.exists() {
        anyhow::bail!("app `{}` already exists", app_root.display());
    }
    fs::create_dir_all(app_root.join("src/scenes")).context("create app scenes dir")?;
    fs::create_dir_all(app_root.join("assets")).context("create app assets dir")?;
    fs::write(
        app_root.join("src/main.mei"),
        format!(
            r#"app(
    id = "{app_id}",
    title = "{app_id}",
    default_scene = "home",
    scene = scene_ref(scene_file = "scenes/home.mei"),
)
"#
        ),
    )?;
    fs::write(
        app_root.join("src/scenes/home.mei"),
        r#"scene(
    id = "home",
    world = "home_world",
    frame = "home_frame",
    profile = "page",
)

world(
    id = "home_world",
    resources = [],
)

frame(
    id = "home_frame",
    layout = flex(direction = "column", gap = "16px", padding = "20px"),
)

frame.add_panel(
    id = "main",
    area = "auto",
    blocks = [],
)
"#,
    )?;
    fs::write(
        app_root.join(APP_CONFIG_FILENAME),
        r#"{
  "schemaVersion": 1,
  "entry": { "main": "main.mei" }
}
"#,
    )?;
    Ok(app_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn ensure_materialize_fills_missing_authoring_tree() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp = std::env::temp_dir().join(format!(
            "mei-ensure-stock-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp workspace");
        let report = ensure_workspace_stock_materialized(temp.as_path(), package_root.as_path())
            .expect("ensure stock")
            .expect("should materialize");
        assert!(report.authoring.copied_files > 0);
        assert!(ensure_workspace_stock_materialized(temp.as_path(), package_root.as_path())
            .expect("ensure again")
            .is_none());
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn materialize_report_includes_authoring_tree() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp = std::env::temp_dir().join(format!(
            "mei-materialize-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("temp dir");
        let report =
            materialize_workspace_stock(temp.as_path(), package_root.as_path(), true).expect("materialize");
        assert!(
            temp.join("stock/authoring/examples/chart-baseline.mei").is_file(),
            "authoring examples should be copied"
        );
        assert_eq!(report.authoring.copied_files > 0, true);
        let json = serde_json::to_value(&report).expect("serialize");
        assert!(json.get("authoring").is_some(), "json must include authoring");
        let _ = fs::remove_dir_all(&temp);
    }
}
