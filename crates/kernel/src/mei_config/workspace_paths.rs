use std::path::{Path, PathBuf};

use super::io::load_workspace_config;
use super::types::{
    WorkspaceConfig, APP_CONFIG_FILENAME, APP_BUILD_ACTIVE_REL, APP_VAR_ACTIVE_REL,
    DEFAULT_APPS_REL, DEFAULT_APP_SRC_REL, DEFAULT_DEPLOY_REL, DEFAULT_RUNTIME_REL,
    DEFAULT_STOCK_COMPONENTS_REL, DEFAULT_STOCK_TEMPLATES_REL, DEFAULT_TOOLCHAIN_REL,
    WORKSPACE_CONFIG_FILENAME,     WORKSPACE_PLATFORM_DIR_REL, WORKSPACE_RUNTIME_CACHE_REL,
    WORKSPACE_RUNTIME_LOGS_REL,
};

pub const MEI_BUNDLE_SNAPSHOT_ROOT_ENV: &str = "MEI_BUNDLE_SNAPSHOT_ROOT";

/// When set, app build stores resolve under `{MEI_BUNDLE_SNAPSHOT_ROOT}/{app}/build/active/`.
pub fn bundle_snapshot_root_from_env() -> Option<PathBuf> {
    std::env::var(MEI_BUNDLE_SNAPSHOT_ROOT_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn is_app_config_root(dir: &Path) -> bool {
    dir.join(APP_CONFIG_FILENAME).is_file()
}

pub fn is_v2_app_root(dir: &Path) -> bool {
    if is_app_config_root(dir) {
        return true;
    }
    resolve_app_src_root(dir).join("main.mei").is_file()
}

pub fn app_mei_config_path(app_root: &Path) -> PathBuf {
    app_root.join(APP_CONFIG_FILENAME)
}

pub fn workspace_config_path(segment_root: &Path) -> PathBuf {
    segment_root.join(WORKSPACE_CONFIG_FILENAME)
}

/// 保留给 CLI 启动路径；workspace stock 不再回退到 package tree。
pub fn set_mei_package_root(_path: PathBuf) {}

pub(crate) fn resolve_workspace_path(source_root: &Path, rel: &str) -> PathBuf {
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

pub fn resolve_apps_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    let rel = cfg
        .paths
        .apps
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(DEFAULT_APPS_REL);
    resolve_workspace_path(source_root, rel)
}

pub fn resolve_toolchain_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    let rel = cfg
        .paths
        .toolchain
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(DEFAULT_TOOLCHAIN_REL);
    resolve_workspace_path(source_root, rel)
}

pub fn resolve_workspace_runtime_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    let rel = cfg
        .paths
        .runtime
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(DEFAULT_RUNTIME_REL);
    resolve_workspace_path(source_root, rel)
}

pub fn resolve_workspace_logs_root(source_root: &Path) -> PathBuf {
    resolve_workspace_path(source_root, WORKSPACE_RUNTIME_LOGS_REL)
}

pub fn resolve_workspace_cache_root(source_root: &Path) -> PathBuf {
    resolve_workspace_path(source_root, WORKSPACE_RUNTIME_CACHE_REL)
}

pub fn resolve_workspace_platform_root(source_root: &Path) -> PathBuf {
    resolve_workspace_path(source_root, WORKSPACE_PLATFORM_DIR_REL)
}

pub fn resolve_deploy_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    let rel = cfg
        .paths
        .deploy
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(DEFAULT_DEPLOY_REL);
    resolve_workspace_path(source_root, rel)
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

/// 解析组件根：`paths.components` → `stock/components`。
pub fn resolve_components_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    if let Some(rel) = configured_components_rel(&cfg) {
        let candidate = resolve_workspace_path(source_root, rel);
        if candidate.is_dir() {
            return candidate;
        }
    }
    resolve_workspace_path(source_root, DEFAULT_STOCK_COMPONENTS_REL)
}

/// 解析模板根：`paths.templates` → `stock/templates`。
pub fn resolve_templates_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    if let Some(rel) = configured_templates_rel(&cfg) {
        let candidate = resolve_workspace_path(source_root, rel);
        if candidate.is_dir() {
            return candidate;
        }
    }
    resolve_workspace_path(source_root, DEFAULT_STOCK_TEMPLATES_REL)
}

pub fn stock_components_source(package_root: &Path) -> PathBuf {
    package_root.join("stock/components")
}

pub fn stock_templates_source(package_root: &Path) -> PathBuf {
    package_root.join("stock/templates")
}

pub fn resolve_authoring_root(source_root: &Path) -> PathBuf {
    super::authoring_helpers::resolve_authoring_root(source_root)
}

fn resolve_canonical_app_dir_name(source_root: &Path, app_id: &str) -> String {
    let app_id = app_id.trim();
    let cfg = load_workspace_config(source_root);
    if let Some(alias) = cfg.discover.app_aliases.get(app_id) {
        let target = alias.trim();
        if !target.is_empty() {
            return target.to_string();
        }
    }
    app_id.to_string()
}

/// App 根：`apps/{appId}/`。
pub fn resolve_app_root(source_root: &Path, app_id: &str) -> PathBuf {
    let name = resolve_canonical_app_dir_name(source_root, app_id);
    resolve_apps_root(source_root).join(name)
}

/// App 源码根：`apps/{appId}/src/`。
pub fn resolve_app_src_root(app_root: &Path) -> PathBuf {
    app_root.join(DEFAULT_APP_SRC_REL)
}

/// App AOT 读路径：`apps/{appId}/build/active/`。
pub fn resolve_app_build_root(app_root: &Path) -> PathBuf {
    if let Some(snapshot_root) = bundle_snapshot_root_from_env() {
        if let Some(app_name) = app_root.file_name() {
            return snapshot_root
                .join(app_name)
                .join("build")
                .join("active");
        }
    }
    app_root.join(APP_BUILD_ACTIVE_REL)
}

/// App 运行时写路径：`apps/{appId}/var/active/`。
pub fn resolve_app_var_root(app_root: &Path) -> PathBuf {
    app_root.join(APP_VAR_ACTIVE_REL)
}

/// 兼容旧名：AOT artifact store = `build/active/`。
pub fn resolve_app_mei_store_root(app_root: &Path) -> PathBuf {
    resolve_app_build_root(app_root)
}

/// Workspace 级 graph registry：`runtime/platform/graphs/{appId}/`。
pub fn resolve_workspace_graph_root(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_workspace_platform_root(source_root)
        .join("graphs")
        .join(app_id.trim())
}

/// 将 URL/CLI 中的 app 标识解析为 canonical app id（`apps/` 下目录名）。
pub fn resolve_app_id(source_root: &Path, app_id: &str) -> String {
    resolve_canonical_app_dir_name(source_root, app_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn v2_app_layout_paths() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::write(ws.join("workspace.json"), r#"{"schemaVersion":2}"#).expect("write");
        let app = ws.join("apps/zhifa");
        fs::create_dir_all(app.join("src")).expect("mkdir");
        fs::write(app.join("src/main.mei"), "app(id=zhifa)").expect("write");
        assert!(is_v2_app_root(&app));
        assert_eq!(
            resolve_app_root(ws, "zhifa"),
            app
        );
        assert_eq!(
            resolve_app_main_path(&app),
            app.join("src/main.mei")
        );
        assert_eq!(
            resolve_app_build_root(&app),
            app.join("build/active")
        );
    }

    fn resolve_app_main_path(app_root: &Path) -> PathBuf {
        super::super::io::resolve_app_main_path(app_root)
    }
}
