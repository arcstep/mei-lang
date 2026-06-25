use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::Utc;
use mei_lang_kernel::{
    resolve_authoring_root, resolve_components_root, resolve_stock_root, resolve_templates_root,
    resolve_toolchain_root, resolve_workspace_runtime_root, stock_authoring_source,
    stock_components_source, stock_templates_source, workspace_config_path, write_workspace_config,
    APP_CONFIG_FILENAME, WorkspaceConfig, WorkspacePathsConfig, WorkspaceProfile,
    WorkspaceStockBootstrapConfig, WorkspaceStockCatalogConfig, WorkspaceStockCatalogKindConfig,
    WorkspaceStockConfig, WorkspaceStockPreviewConfig, DEFAULT_APPS_REL, DEFAULT_STOCK_AUTHORING_REL,
    DEFAULT_STOCK_COMPONENTS_REL, DEFAULT_STOCK_TEMPLATES_REL, WORKSPACE_HOSTS_DIR_REL,
};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

const STOCK_MANIFEST_FILENAME: &str = "STOCK.json";
const STOCK_MANIFEST_SCHEMA_VERSION: u32 = 1;
const BOOTSTRAP_SOURCE_PLATFORM_DEFAULT: &str = "platform-default";
const LEGACY_STOCK_DIR: &str = ".stock";

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockTreeFingerprint {
    #[serde(rename = "fileCount")]
    pub file_count: usize,
    #[serde(skip_serializing_if = "Option::is_none", rename = "pathsHash")]
    pub paths_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "materializedAt")]
    pub materialized_at: String,
    #[serde(rename = "bootstrapSource")]
    pub bootstrap_source: String,
    #[serde(rename = "packageRoot")]
    pub package_root: String,
    pub components: StockTreeFingerprint,
    pub templates: StockTreeFingerprint,
    pub authoring: StockTreeFingerprint,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockDoctorReport {
    pub ok: bool,
    pub missing_trees: Vec<String>,
    pub orphan_paths: Vec<String>,
    pub manifest_drift: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrateWorkspaceStockPathsReport {
    pub renamed_legacy_stock: bool,
    pub updated_example_files: Vec<String>,
}

pub fn materialize_workspace_stock(
    source_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<MaterializeReport> {
    fs::create_dir_all(source_root.join(WORKSPACE_HOSTS_DIR_REL))
        .context("create workspace host-state dir")?;
    let components_dest = resolve_components_root(source_root);
    let templates_dest = resolve_templates_root(source_root);
    let authoring_dest = resolve_authoring_root(source_root);
    let components = materialize_tree(
        &stock_components_source(package_root),
        &components_dest,
        force,
    )?;
    let templates = materialize_tree(
        &stock_templates_source(package_root),
        &templates_dest,
        force,
    )?;
    let authoring = materialize_tree(
        &stock_authoring_source(package_root),
        &authoring_dest,
        force,
    )?;
    write_stock_manifest(
        source_root,
        package_root,
        &components_dest,
        &templates_dest,
        &authoring_dest,
    )?;
    Ok(MaterializeReport {
        source_root: source_root.display().to_string(),
        components,
        templates,
        authoring,
    })
}

/// Force-sync workspace stock trees from the platform package (alias for materialize).
pub fn sync_workspace_stock(
    source_root: &Path,
    package_root: &Path,
    force: bool,
) -> Result<MaterializeReport> {
    materialize_workspace_stock(source_root, package_root, force)
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

pub fn doctor_workspace_stock(
    source_root: &Path,
    package_root: &Path,
) -> Result<StockDoctorReport> {
    let mut missing_trees = Vec::new();
    let mut orphan_paths = Vec::new();
    let mut manifest_drift = Vec::new();
    let mut warnings = Vec::new();

    let components_root = resolve_components_root(source_root);
    let templates_root = resolve_templates_root(source_root);
    let authoring_root = resolve_authoring_root(source_root);
    for (label, path) in [
        ("components", &components_root),
        ("templates", &templates_root),
        ("authoring", &authoring_root),
    ] {
        if !stock_tree_ready(path) {
            missing_trees.push(format!("{label}: {}", path.display()));
        }
    }

    let legacy_stock = source_root.join(LEGACY_STOCK_DIR);
    if legacy_stock.is_dir() {
        orphan_paths.push(legacy_stock.display().to_string());
    }
    let root_authoring = source_root.join("authoring");
    if root_authoring.is_dir() {
        orphan_paths.push(root_authoring.display().to_string());
    }
    let nested_authoring = resolve_stock_root(source_root).join("authoring/authoring");
    if nested_authoring.is_dir() {
        orphan_paths.push(nested_authoring.display().to_string());
    }

    let manifest_path = resolve_stock_root(source_root).join(STOCK_MANIFEST_FILENAME);
    if manifest_path.is_file() {
        let raw = fs::read_to_string(&manifest_path)
            .with_context(|| format!("read {}", manifest_path.display()))?;
        match serde_json::from_str::<StockManifest>(&raw) {
            Ok(manifest) => {
                if manifest.bootstrap_source != BOOTSTRAP_SOURCE_PLATFORM_DEFAULT {
                    manifest_drift.push(format!(
                        "bootstrapSource={} (expected {BOOTSTRAP_SOURCE_PLATFORM_DEFAULT})",
                        manifest.bootstrap_source
                    ));
                }
                let expected_package_root = package_root
                    .canonicalize()
                    .unwrap_or_else(|_| package_root.to_path_buf());
                let recorded = PathBuf::from(&manifest.package_root);
                let recorded_canonical = recorded.canonicalize().unwrap_or(recorded);
                if recorded_canonical != expected_package_root {
                    manifest_drift.push(format!(
                        "packageRoot={} (expected {})",
                        manifest.package_root,
                        expected_package_root.display()
                    ));
                }
            }
            Err(error) => warnings.push(format!(
                "failed to parse {}: {error}",
                manifest_path.display()
            )),
        }
    } else if stock_tree_ready(&components_root)
        || stock_tree_ready(&templates_root)
        || stock_tree_ready(&authoring_root)
    {
        warnings.push(format!(
            "missing {} under {}",
            STOCK_MANIFEST_FILENAME,
            resolve_stock_root(source_root).display()
        ));
    }

    let ok = missing_trees.is_empty() && orphan_paths.is_empty() && manifest_drift.is_empty();
    Ok(StockDoctorReport {
        ok,
        missing_trees,
        orphan_paths,
        manifest_drift,
        warnings,
    })
}

pub fn migrate_workspace_stock_paths(source_root: &Path) -> Result<MigrateWorkspaceStockPathsReport> {
    let legacy_stock = source_root.join(LEGACY_STOCK_DIR);
    let stock_root = resolve_stock_root(source_root);
    let mut renamed_legacy_stock = false;
    if legacy_stock.is_dir() {
        if stock_root.exists() {
            anyhow::bail!(
                "both `{}` and `{}` exist; merge or remove legacy `.stock` manually before migrate",
                legacy_stock.display(),
                stock_root.display()
            );
        }
        fs::rename(&legacy_stock, &stock_root).with_context(|| {
            format!(
                "rename legacy stock {} -> {}",
                legacy_stock.display(),
                stock_root.display()
            )
        })?;
        renamed_legacy_stock = true;
    }

    let examples_dir = resolve_authoring_root(source_root).join("examples");
    let mut updated_example_files = Vec::new();
    if examples_dir.is_dir() {
        for entry in fs::read_dir(&examples_dir).with_context(|| {
            format!("read authoring examples dir {}", examples_dir.display())
        })? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("mei") {
                continue;
            }
            let original = fs::read_to_string(&path)
                .with_context(|| format!("read example mei {}", path.display()))?;
            let migrated = migrate_example_mei_stock_paths(&original);
            if migrated != original {
                fs::write(&path, migrated)
                    .with_context(|| format!("write migrated example {}", path.display()))?;
                updated_example_files.push(path.display().to_string());
            }
        }
    }

    Ok(MigrateWorkspaceStockPathsReport {
        renamed_legacy_stock,
        updated_example_files,
    })
}

fn migrate_example_mei_stock_paths(content: &str) -> String {
    content
        .replace("../.stock/", "../../stock/")
        .replace(".stock/", "stock/")
}

fn workspace_stock_needs_materialize(source_root: &Path) -> bool {
    !stock_tree_ready(&resolve_authoring_root(source_root))
        || !stock_tree_ready(&resolve_components_root(source_root))
        || !stock_tree_ready(&resolve_templates_root(source_root))
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

/// Stable revision string for promote gates (`stockRevision` in links state).
pub fn workspace_stock_revision(source_root: &Path) -> Option<String> {
    let manifest_path = resolve_stock_root(source_root).join(STOCK_MANIFEST_FILENAME);
    let raw = fs::read_to_string(manifest_path).ok()?;
    let manifest: StockManifest = serde_json::from_str(&raw).ok()?;
    let mut hasher = DefaultHasher::new();
    manifest.components.paths_hash.hash(&mut hasher);
    manifest.templates.paths_hash.hash(&mut hasher);
    manifest.authoring.paths_hash.hash(&mut hasher);
    Some(format!(
        "stock-v{STOCK_MANIFEST_SCHEMA_VERSION}-{:016x}",
        hasher.finish()
    ))
}

fn write_stock_manifest(
    source_root: &Path,
    package_root: &Path,
    components_root: &Path,
    templates_root: &Path,
    authoring_root: &Path,
) -> Result<()> {
    let stock_root = resolve_stock_root(source_root);
    fs::create_dir_all(&stock_root).context("create stock root for manifest")?;
    let manifest = StockManifest {
        schema_version: STOCK_MANIFEST_SCHEMA_VERSION,
        materialized_at: Utc::now().to_rfc3339(),
        bootstrap_source: BOOTSTRAP_SOURCE_PLATFORM_DEFAULT.to_string(),
        package_root: package_root.display().to_string(),
        components: fingerprint_tree(components_root)?,
        templates: fingerprint_tree(templates_root)?,
        authoring: fingerprint_tree(authoring_root)?,
    };
    let manifest_path = stock_root.join(STOCK_MANIFEST_FILENAME);
    let json = serde_json::to_string_pretty(&manifest).context("serialize STOCK.json")?;
    fs::write(&manifest_path, format!("{json}\n"))
        .with_context(|| format!("write {}", manifest_path.display()))?;
    Ok(())
}

fn fingerprint_tree(root: &Path) -> Result<StockTreeFingerprint> {
    if !root.is_dir() {
        return Ok(StockTreeFingerprint {
            file_count: 0,
            paths_hash: None,
        });
    }
    let mut rel_paths = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .context("strip fingerprint prefix")?
            .to_string_lossy()
            .replace('\\', "/");
        rel_paths.push(rel);
    }
    rel_paths.sort();
    let file_count = rel_paths.len();
    let paths_hash = if file_count == 0 {
        None
    } else {
        let mut hasher = DefaultHasher::new();
        rel_paths.join("\n").hash(&mut hasher);
        Some(format!("{:016x}", hasher.finish()))
    };
    Ok(StockTreeFingerprint {
        file_count,
        paths_hash,
    })
}

fn default_workspace_stock_config() -> WorkspaceStockConfig {
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
        assert!(
            temp.join("stock/STOCK.json").is_file(),
            "STOCK.json manifest should be written"
        );
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn doctor_detects_missing_tree() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp = std::env::temp_dir().join(format!(
            "mei-doctor-stock-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp workspace");
        let report = doctor_workspace_stock(temp.as_path(), package_root.as_path()).expect("doctor");
        assert!(!report.ok, "empty workspace should not pass doctor");
        assert_eq!(report.missing_trees.len(), 3);
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn workspace_stock_revision_reads_manifest_fingerprint() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp = std::env::temp_dir().join(format!(
            "mei-stock-revision-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp workspace");
        materialize_workspace_stock(temp.as_path(), package_root.as_path(), false)
            .expect("materialize");
        let revision = workspace_stock_revision(temp.as_path()).expect("revision");
        assert!(
            revision.starts_with("stock-v"),
            "unexpected revision format: {revision}"
        );
        let _ = fs::remove_dir_all(&temp);
    }
}
