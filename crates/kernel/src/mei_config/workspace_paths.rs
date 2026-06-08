use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::io::load_workspace_config;
use super::types::{
    WorkspaceConfig, DEFAULT_STOCK_COMPONENTS_REL, DEFAULT_STOCK_TEMPLATES_REL,
    MEI_CONFIG_FILENAME, MEI_WORKSPACE_CONFIG_FILENAME,
};

pub fn is_app_config_root(dir: &Path) -> bool {
    dir.join(MEI_CONFIG_FILENAME).is_file()
}

pub fn app_mei_config_path(app_root: &Path) -> PathBuf {
    app_root.join(MEI_CONFIG_FILENAME)
}

pub fn workspace_config_path(segment_root: &Path) -> PathBuf {
    segment_root.join(MEI_WORKSPACE_CONFIG_FILENAME)
}

static MEI_PACKAGE_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// 由 `mei-lang-server` 启动时注入；供 stock 回退路径解析。
pub fn set_mei_package_root(path: PathBuf) {
    let _ = MEI_PACKAGE_ROOT.set(path);
}

fn mei_package_root() -> Option<&'static Path> {
    MEI_PACKAGE_ROOT.get().map(|path| path.as_path())
}

fn resolve_workspace_path(source_root: &Path, rel: &str) -> PathBuf {
    let trimmed = rel.trim();
    if trimmed.is_empty() {
        return source_root.to_path_buf();
    }
    if Path::new(trimmed).is_absolute() {
        PathBuf::from(trimmed)
    } else {
        source_root.join(trimmed)
    }
}

fn configured_components_rel(cfg: &WorkspaceConfig) -> Option<&str> {
    cfg.paths
        .components
        .as_deref()
        .or(cfg.discover.components_root.as_deref())
        .filter(|value| !value.trim().is_empty())
}

fn configured_templates_rel(cfg: &WorkspaceConfig) -> Option<&str> {
    cfg.paths
        .templates
        .as_deref()
        .filter(|value| !value.trim().is_empty())
}

/// 解析组件根：`paths.components` → 物化目录 → `mei-lang/stock/components`。
pub fn resolve_components_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    if let Some(rel) = configured_components_rel(&cfg) {
        let candidate = resolve_workspace_path(source_root, rel);
        if candidate.is_dir() {
            return candidate;
        }
    }
    let materialized = resolve_workspace_path(source_root, DEFAULT_STOCK_COMPONENTS_REL);
    if materialized.is_dir() {
        return materialized;
    }
    if let Some(package_root) = mei_package_root() {
        let stock = package_root.join("stock/components");
        if stock.is_dir() {
            return stock;
        }
    }
    materialized
}

/// 解析模板根：`paths.templates` → 物化目录 → `mei-lang/stock/templates`。
pub fn resolve_templates_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    if let Some(rel) = configured_templates_rel(&cfg) {
        let candidate = resolve_workspace_path(source_root, rel);
        if candidate.is_dir() {
            return candidate;
        }
    }
    let materialized = resolve_workspace_path(source_root, DEFAULT_STOCK_TEMPLATES_REL);
    if materialized.is_dir() {
        return materialized;
    }
    if let Some(package_root) = mei_package_root() {
        let stock = package_root.join("stock/templates");
        if stock.is_dir() {
            return stock;
        }
    }
    materialized
}

pub fn stock_components_source(package_root: &Path) -> PathBuf {
    package_root.join("stock/components")
}

pub fn stock_templates_source(package_root: &Path) -> PathBuf {
    package_root.join("stock/templates")
}

/// 将 CLI/URL 中的 `app_id` 解析为应用目录（支持 `discover.appAliases`）。
pub fn resolve_app_root(source_root: &Path, app_id: &str) -> PathBuf {
    let app_id = app_id.trim();
    let direct = source_root.join(app_id);
    if direct.exists() {
        return direct;
    }
    let cfg = load_workspace_config(source_root);
    if let Some(alias) = cfg.discover.app_aliases.get(app_id) {
        let target = alias.trim();
        if !target.is_empty() {
            let aliased = source_root.join(target);
            if aliased.exists() {
                return aliased;
            }
        }
    }
    direct
}
