use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use mei_lang_kernel::{
    stock_components_source, stock_templates_source, workspace_config_path, write_workspace_config,
    WorkspaceConfig, WorkspacePathsConfig, WorkspaceProfile, DEFAULT_STOCK_AUTHORING_REL,
    DEFAULT_STOCK_COMPONENTS_REL,
    DEFAULT_STOCK_TEMPLATES_REL, WORKSPACE_HOSTS_DIR_REL,
};
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct MaterializeReport {
    pub source_root: String,
    pub components: MaterializeDirReport,
    pub templates: MaterializeDirReport,
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
    Ok(MaterializeReport {
        source_root: source_root.display().to_string(),
        components,
        templates,
    })
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
    materialize: bool,
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
    fs::create_dir_all(source_root.join(".mei")).context("create workspace .mei runtime dir")?;
    fs::create_dir_all(source_root.join(WORKSPACE_HOSTS_DIR_REL))
        .context("create workspace host-state dir")?;

    let config_path = workspace_config_path(&source_root);
    if !config_path.is_file() {
        let config = WorkspaceConfig {
            schema_version: 1,
            workspace: WorkspaceProfile {
                id: Some(profile_id.to_string()),
                label: label.map(str::to_string),
                deploy_host: None,
            },
            paths: WorkspacePathsConfig {
                components: Some(DEFAULT_STOCK_COMPONENTS_REL.to_string()),
                templates: Some(DEFAULT_STOCK_TEMPLATES_REL.to_string()),
                authoring: Some(DEFAULT_STOCK_AUTHORING_REL.to_string()),
            },
            ..WorkspaceConfig::default()
        };
        write_workspace_config(&config_path, &config)?;
    }
    if materialize {
        materialize_workspace_stock(&source_root, package_root, false)?;
    }
    Ok(source_root)
}

pub fn create_app_skeleton(source_root: &Path, app_id: &str) -> Result<PathBuf> {
    let app_id = app_id.trim();
    if app_id.is_empty() || app_id.starts_with('.') || app_id.starts_with('_') {
        anyhow::bail!("app id must be a plain directory name");
    }
    let app_root = source_root.join(app_id);
    if app_root.exists() {
        anyhow::bail!("app `{}` already exists", app_root.display());
    }
    fs::create_dir_all(app_root.join("scenes")).context("create app scenes dir")?;
    fs::write(
        app_root.join("main.mei"),
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
        app_root.join("scenes/home.mei"),
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
        app_root.join(".mei-config.json"),
        r#"{
  "schemaVersion": 1,
  "entry": { "main": "main.mei" }
}
"#,
    )?;
    Ok(app_root)
}
