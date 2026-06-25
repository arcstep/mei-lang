use std::fs;
use std::path::{Path, PathBuf};

use super::io::load_workspace_config;
use super::build_store::{resolve_app_build_root_following_active, resolve_symlink_target};
use super::types::{
    WorkspaceConfig, APP_CONFIG_FILENAME, APP_BUILD_STORE_REL, APP_VAR_ACTIVE_REL,
    DEFAULT_APPS_REL, DEFAULT_APP_SRC_REL, DEFAULT_DEPLOY_REL, DEFAULT_RUNTIME_REL,
    DEFAULT_STOCK_COMPONENTS_REL, DEFAULT_STOCK_TEMPLATES_REL, DEFAULT_TOOLCHAIN_REL,
    WORKSPACE_CONFIG_FILENAME, WORKSPACE_PLATFORM_DIR_REL, WORKSPACE_RUNTIME_CACHE_REL,
    WORKSPACE_RUNTIME_LOGS_REL,
};

pub fn resolve_symlink_target_from_link(link: &Path) -> Option<PathBuf> {
    fs::read_link(link).ok().map(|target| {
        if target.is_absolute() {
            target
        } else {
            link.parent()
                .map(|parent| parent.join(&target))
                .unwrap_or(target)
        }
    })
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

fn normalize_app_rel_path(rel: &str) -> String {
    rel.trim().trim_start_matches("./").replace('\\', "/")
}

/// 将 app 内逻辑相对路径解析为磁盘路径（v2：`*.mei` 在 `src/` 下；config 在 app 根）。
pub fn resolve_app_mei_file_path(app_root: &Path, rel: &str) -> PathBuf {
    let rel = normalize_app_rel_path(rel);
    if rel.is_empty() {
        return resolve_app_src_root(app_root);
    }
    if Path::new(&rel).is_absolute() {
        return PathBuf::from(rel);
    }
    if rel.starts_with("src/") {
        return app_root.join(rel);
    }
    if rel == APP_CONFIG_FILENAME {
        return app_root.join(rel);
    }
    if rel.ends_with(".mei") {
        let under_src = resolve_app_src_root(app_root).join(&rel);
        if under_src.is_file() {
            return under_src;
        }
        let legacy = app_root.join(&rel);
        if legacy.is_file() {
            return legacy;
        }
        return under_src;
    }
    let under_src = resolve_app_src_root(app_root).join(&rel);
    if under_src.is_file() {
        return under_src;
    }
    app_root.join(&rel)
}

/// 逻辑路径是否指向 Mei 源码（watch set / dependency 用逻辑名，open 用 [`resolve_app_mei_file_path`]）。
pub fn is_app_mei_source_rel(rel: &str) -> bool {
    let rel = normalize_app_rel_path(rel);
    rel.ends_with(".mei") || rel.starts_with("scenes/") || rel == "main.mei"
}

/// App AOT 读路径：`apps/{appId}/build/active/`（symlink 指向 `build/store/{buildId}/`）。
pub fn resolve_app_build_root(app_root: &Path) -> PathBuf {
    resolve_app_build_root_following_active(app_root)
}

/// App build store 根：`apps/{appId}/build/store/{buildId}/`。
pub fn resolve_app_build_store_root(app_root: &Path, build_id: &str) -> PathBuf {
    app_root.join(APP_BUILD_STORE_REL).join(build_id.trim())
}

/// App 运行时写路径：`apps/{appId}/var/active/`（symlink 指向 `var/store/{buildId}/`）。
pub fn resolve_app_var_root(app_root: &Path) -> PathBuf {
    let active = app_root.join(APP_VAR_ACTIVE_REL);
    if active.is_symlink() {
        if let Some(target) = resolve_symlink_target(&active) {
            return target;
        }
    }
    if active.is_dir() {
        return active;
    }
    active
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

    #[test]
    fn resolve_app_mei_file_path_v2_layout() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let app = tmp.path().join("apps/hello");
        fs::create_dir_all(app.join("src/scenes")).expect("mkdir");
        fs::write(app.join("src/main.mei"), "app(id=hello)").expect("write");
        fs::write(app.join("src/scenes/home.mei"), "scene(id=home)").expect("write");
        assert!(resolve_app_mei_file_path(&app, "main.mei").is_file());
        assert!(resolve_app_mei_file_path(&app, "scenes/home.mei").is_file());
    }

    fn resolve_app_main_path(app_root: &Path) -> PathBuf {
        super::super::io::resolve_app_main_path(app_root)
    }
}
