use super::prelude::*;
use super::*;

pub(crate) fn default_workspace_stock_config() -> WorkspaceStockConfig {
    WorkspaceStockConfig {
        bootstrap: WorkspaceStockBootstrapConfig {
            source: Some(BOOTSTRAP_SOURCE_PLATFORM_DEFAULT.to_string()),
        },
        catalog: WorkspaceStockCatalogConfig {
            components: WorkspaceStockCatalogKindConfig {
                enabled: true,
                exclude: Vec::new(),
            },
            templates: WorkspaceStockCatalogKindConfig {
                enabled: true,
                exclude: Vec::new(),
            },
            authoring: WorkspaceStockCatalogKindConfig {
                enabled: true,
                exclude: Vec::new(),
            },
        },
        preview: WorkspaceStockPreviewConfig {
            workspace_only: true,
            ..WorkspaceStockPreviewConfig::default()
        },
        catalog_app: WorkspaceStockCatalogAppConfig::default(),
        sources: Vec::new(),
    }
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
                version: None,
            },
            paths: WorkspacePathsConfig {
                apps: Some(DEFAULT_APPS_REL.to_string()),
                components: Some(DEFAULT_STOCK_COMPONENTS_REL.to_string()),
                templates: Some(DEFAULT_STOCK_TEMPLATES_REL.to_string()),
                authoring: Some(DEFAULT_STOCK_AUTHORING_REL.to_string()),
                ..WorkspacePathsConfig::default()
            },
            stock: default_workspace_stock_config(),
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
