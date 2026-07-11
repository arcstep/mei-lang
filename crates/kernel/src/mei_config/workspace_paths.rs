use std::fs;
use std::path::{Path, PathBuf};

use super::build_store::resolve_app_build_root_following_active;
use super::io::load_workspace_config;
use super::stock_catalog::normalize_stock_relative_path;
use super::types::{
    WorkspaceConfig, APP_CONFIG_FILENAME, DEFAULT_APPS_REL, DEFAULT_APP_SRC_REL,
    DEFAULT_DEPLOY_REL, DEFAULT_RUNTIME_REL, DEFAULT_STOCK_AUTHORING_REL,
    DEFAULT_STOCK_COMPONENTS_REL, DEFAULT_STOCK_TEMPLATES_REL, DEFAULT_TOOLCHAIN_REL,
    LEGACY_WORKSPACE_HOSTS_DIR_REL, LEGACY_WORKSPACE_PLATFORM_DIR_REL,
    LEGACY_WORKSPACE_RUNTIME_DIR_REL, WORKSPACE_CONFIG_FILENAME, WORKSPACE_HOSTS_DIR_REL,
    WORKSPACE_PLATFORM_DIR_REL, WORKSPACE_RUNTIME_CACHE_REL, WORKSPACE_RUNTIME_LOGS_REL,
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
    resolve_workspace_config_path(segment_root, None)
}

/// Resolve workspace config file: `MEI_WORKSPACE_CONFIG` (absolute or relative to segment root), else `workspace.json`.
pub fn resolve_workspace_config_path(segment_root: &Path, override_path: Option<&Path>) -> PathBuf {
    let candidates = override_path
        .map(|path| vec![path.to_path_buf()])
        .unwrap_or_else(|| {
            std::env::var("MEI_WORKSPACE_CONFIG")
                .ok()
                .map(|raw| vec![PathBuf::from(raw.trim())])
                .unwrap_or_default()
        });
    for candidate in candidates {
        if candidate.is_file() {
            return candidate;
        }
        if candidate.is_relative() {
            let joined = segment_root.join(&candidate);
            if joined.is_file() {
                return joined;
            }
        }
    }
    segment_root.join(WORKSPACE_CONFIG_FILENAME)
}

/// 从 app 根向上解析 workspace `--source-root`（v2：`apps/{id}/` → 含 `workspace.json` 的祖先）。
pub fn resolve_workspace_source_root_from_app_root(app_root: &Path) -> PathBuf {
    let mut cursor = app_root.to_path_buf();
    loop {
        if cursor.join(WORKSPACE_CONFIG_FILENAME).is_file() {
            return cursor;
        }
        if cursor.join("_components").is_dir() {
            return cursor;
        }
        if !cursor.pop() {
            break;
        }
    }
    app_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| app_root.to_path_buf())
}

/// 保留给 CLI 启动路径；workspace stock 不再回退到 package tree。
pub fn set_mei_package_root(_path: PathBuf) {}

pub fn resolve_workspace_path(source_root: &Path, rel: &str) -> PathBuf {
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
    let primary = resolve_workspace_path(source_root, rel);
    if primary.is_dir() {
        return primary;
    }
    let legacy = source_root.join(LEGACY_WORKSPACE_RUNTIME_DIR_REL);
    if legacy.is_dir() {
        return legacy;
    }
    primary
}

pub fn resolve_workspace_platform_root(source_root: &Path) -> PathBuf {
    let primary = resolve_workspace_path(source_root, WORKSPACE_PLATFORM_DIR_REL);
    if primary.is_dir() {
        return primary;
    }
    resolve_workspace_path(source_root, LEGACY_WORKSPACE_PLATFORM_DIR_REL)
}

pub fn resolve_workspace_hosts_root(source_root: &Path) -> PathBuf {
    let primary = resolve_workspace_path(source_root, WORKSPACE_HOSTS_DIR_REL);
    if primary.is_dir() {
        return primary;
    }
    resolve_workspace_path(source_root, LEGACY_WORKSPACE_HOSTS_DIR_REL)
}

pub fn resolve_workspace_logs_root(source_root: &Path) -> PathBuf {
    let primary = resolve_workspace_path(source_root, WORKSPACE_RUNTIME_LOGS_REL);
    if primary.is_dir() {
        return primary;
    }
    resolve_workspace_runtime_root(source_root).join("logs")
}

pub fn resolve_workspace_cache_root(source_root: &Path) -> PathBuf {
    let primary = resolve_workspace_path(source_root, WORKSPACE_RUNTIME_CACHE_REL);
    if primary.is_dir() {
        return primary;
    }
    resolve_workspace_runtime_root(source_root).join("cache")
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

pub fn resolve_stock_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    let rel = cfg
        .paths
        .stock
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("stock");
    resolve_workspace_path(source_root, rel)
}

fn resolve_stock_subdir(
    source_root: &Path,
    configured: Option<&str>,
    default_rel: &str,
) -> PathBuf {
    if let Some(rel) = configured.filter(|value| !value.trim().is_empty()) {
        let trimmed = rel.trim();
        if trimmed.contains('/') {
            let candidate = resolve_workspace_path(source_root, trimmed);
            if candidate.is_dir() {
                return candidate;
            }
        } else {
            let under_stock = resolve_stock_root(source_root).join(trimmed);
            if under_stock.is_dir() {
                return under_stock;
            }
        }
    }
    resolve_workspace_path(source_root, default_rel)
}

/// 解析组件根：`paths.components` → `{stock}/components`。
pub fn resolve_components_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    let configured = configured_components_rel(&cfg);
    resolve_stock_subdir(source_root, configured, DEFAULT_STOCK_COMPONENTS_REL)
}

/// 解析模板根：`paths.templates` → `{stock}/templates`。
pub fn resolve_templates_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    resolve_stock_subdir(
        source_root,
        configured_templates_rel(&cfg),
        DEFAULT_STOCK_TEMPLATES_REL,
    )
}

pub fn stock_components_source(package_root: &Path) -> PathBuf {
    package_root.join("stock/components")
}

pub fn stock_templates_source(package_root: &Path) -> PathBuf {
    package_root.join("stock/templates")
}

pub fn stock_authoring_source(package_root: &Path) -> PathBuf {
    package_root.join("stock/authoring")
}

pub fn resolve_authoring_root(source_root: &Path) -> PathBuf {
    let cfg = load_workspace_config(source_root);
    if let Some(rel) = cfg
        .paths
        .authoring
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let candidate = resolve_workspace_path(source_root, rel);
        if candidate.is_dir() {
            return candidate;
        }
    }
    resolve_workspace_path(source_root, DEFAULT_STOCK_AUTHORING_REL)
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

/// Paths that must not receive an automatic `src/` prefix (stock imports, escapes).
pub fn is_external_or_stock_mei_rel(rel: &str) -> bool {
    let rel = normalize_app_rel_path(rel);
    rel.starts_with("stock/")
        || rel.starts_with(".stock/")
        || rel.contains("/../")
        || rel.starts_with("../")
}

/// Canonical app-relative path for authored sources under `apps/{id}/src/`.
///
/// v2 SSOT: logical paths in MCG/MRG/scope gate/prebuild use `src/…` even when
/// authors or legacy configs still write `scenes/foo.mei`.
pub fn canonical_app_source_rel_path(rel: &str) -> String {
    let rel = normalize_app_rel_path(rel);
    if rel.is_empty() || Path::new(&rel).is_absolute() {
        return rel;
    }
    if is_external_or_stock_mei_rel(rel.as_str()) {
        return rel;
    }
    if rel.starts_with("src/") {
        return rel;
    }
    if rel.ends_with(".mei") {
        return format!("src/{rel}");
    }
    rel
}

/// Lookup keys for registry rows keyed by target file (canonical first, then legacy).
pub fn app_source_rel_path_lookup_keys(rel: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut push = |value: String| {
        if !value.is_empty() && !keys.iter().any(|existing| existing == &value) {
            keys.push(value);
        }
    };
    push(canonical_app_source_rel_path(rel));
    let normalized = normalize_app_rel_path(rel);
    push(normalized.clone());
    if let Some(stripped) = normalized.strip_prefix("src/") {
        push(stripped.to_string());
    } else if normalized.ends_with(".mei") {
        push(format!("src/{normalized}"));
    }
    keys
}

/// Map legacy app-relative `.stock/...` paths onto workspace `stock/...` (post v2 migration).
fn resolve_legacy_workspace_stock_mei_path(app_root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.replace('\\', "/");
    let dot_stock = rel.find(".stock/")?;
    let stock_tail = rel[dot_stock + ".stock/".len()..].trim_start_matches('/');
    if stock_tail.is_empty() {
        return None;
    }
    let source_root = resolve_workspace_source_root_from_app_root(app_root);
    let under_stock = source_root.join(normalize_stock_relative_path(&format!(
        ".stock/{stock_tail}"
    )));
    if under_stock.is_file() {
        return Some(under_stock);
    }
    let under_templates = resolve_templates_root(&source_root)
        .join(stock_tail.strip_prefix("templates/").unwrap_or(stock_tail));
    if under_templates.is_file() {
        return Some(under_templates);
    }
    None
}

fn resolve_app_mei_file_path_primary(app_root: &Path, rel: &str) -> PathBuf {
    if rel.ends_with(".mei") {
        let under_src = resolve_app_src_root(app_root).join(rel);
        if under_src.is_file() {
            return under_src;
        }
        let legacy = app_root.join(rel);
        if legacy.is_file() {
            return legacy;
        }
        return under_src;
    }
    let under_src = resolve_app_src_root(app_root).join(rel);
    if under_src.is_file() {
        return under_src;
    }
    app_root.join(rel)
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
    let primary = resolve_app_mei_file_path_primary(app_root, rel.as_str());
    if primary.is_file() {
        return primary;
    }
    resolve_legacy_workspace_stock_mei_path(app_root, rel.as_str()).unwrap_or(primary)
}

/// 逻辑路径是否指向 Mei 源码（watch set / dependency 用逻辑名，open 用 [`resolve_app_mei_file_path`]）。
pub fn is_app_mei_source_rel(rel: &str) -> bool {
    let rel = normalize_app_rel_path(rel);
    rel.ends_with(".mei")
        || rel.starts_with("scenes/")
        || rel.starts_with("src/scenes/")
        || rel == "main.mei"
        || rel == "src/main.mei"
}

/// App AOT 读路径：`apps/{appId}/env/current/build/`（经 `env/current` 解析）。
pub fn resolve_app_build_root(app_root: &Path) -> PathBuf {
    resolve_app_build_root_following_active(app_root)
}

/// App env build 根：`apps/{appId}/env/{ver}/build/`。
pub fn resolve_app_build_store_root(app_root: &Path, env_version: &str) -> PathBuf {
    crate::mei_config::build_store::app_env_build_dir(app_root, env_version)
}

/// App 运行时写路径：`apps/{appId}/env/current/var/`（经 `env/current` 解析）。
pub fn resolve_app_var_root(app_root: &Path) -> PathBuf {
    crate::mei_config::build_store::resolve_app_var_root_following_active(app_root)
}

/// 求值物化缓存根：`apps/{appId}/var/active/eval-cache/`。
pub fn resolve_app_eval_cache_root(app_root: &Path) -> PathBuf {
    resolve_app_var_root(app_root).join("eval-cache")
}

/// xlsx parquet 快照根：`apps/{appId}/var/active/data-snapshots/`。
pub fn resolve_app_data_snapshot_root(app_root: &Path) -> PathBuf {
    resolve_app_var_root(app_root).join("data-snapshots")
}

/// MCG/MRG registry 根：`apps/{appId}/env/current/build/registry/`。
pub fn resolve_app_registry_root(app_root: &Path) -> PathBuf {
    resolve_app_build_root(app_root).join("registry")
}

/// AOT artifact store = active env build root。
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

/// Scene 结构入口：优先 `src/scene/{id}.mei`，回落 `src/scene/{id}/assembly.mei`。
/// 返回带 `src/` 前缀的逻辑相对路径（与 `assembly_ref` / navigation 一致）。
pub fn resolve_scene_assembly_rel(app_root: &Path, scene_id: &str) -> String {
    let scene_id = scene_id.trim().trim_matches('/');
    if scene_id.is_empty() {
        return "src/scene/home.mei".to_string();
    }
    let modern = format!("src/scene/{scene_id}.mei");
    let legacy = format!("src/scene/{scene_id}/assembly.mei");
    if resolve_app_mei_file_path(app_root, &modern).is_file() {
        modern
    } else if resolve_app_mei_file_path(app_root, &legacy).is_file() {
        legacy
    } else {
        modern
    }
}

/// `{scene_id}@{resolve_scene_assembly_rel(...)}`
pub fn default_scene_assembly_key(app_root: &Path, scene_id: &str) -> String {
    let scene_id = scene_id.trim();
    let scene_id = if scene_id.is_empty() { "home" } else { scene_id };
    format!("{scene_id}@{}", resolve_scene_assembly_rel(app_root, scene_id))
}

/// 结构节点文件：`.../r-foo.mei` / `.../s-bar.mei` / 遗留 `.../layout.mei`。
pub fn is_region_structure_mei_path(path: &str) -> bool {
    let raw = path.replace('\\', "/");
    if !raw.contains("/r-") {
        return false;
    }
    if raw.ends_with("/layout.mei") {
        return true;
    }
    Path::new(&raw)
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| name.starts_with("r-") && name.ends_with(".mei"))
}

pub fn is_section_structure_mei_path(path: &str) -> bool {
    let raw = path.replace('\\', "/");
    if !raw.contains("/s-") {
        return false;
    }
    if raw.ends_with("/layout.mei") {
        return true;
    }
    Path::new(&raw)
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| name.starts_with("s-") && name.ends_with(".mei"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn canonical_app_source_rel_path_adds_src_prefix() {
        assert_eq!(
            canonical_app_source_rel_path("scenes/home.mei"),
            "src/scenes/home.mei"
        );
        assert_eq!(
            canonical_app_source_rel_path("src/scenes/home.mei"),
            "src/scenes/home.mei"
        );
        assert_eq!(canonical_app_source_rel_path("main.mei"), "src/main.mei");
        assert_eq!(
            canonical_app_source_rel_path("../../stock/templates/cockpit/panel/x.mei"),
            "../../stock/templates/cockpit/panel/x.mei"
        );
    }

    #[test]
    fn app_source_rel_path_lookup_keys_includes_legacy() {
        let keys = app_source_rel_path_lookup_keys("scenes/home.mei");
        assert!(keys.contains(&"src/scenes/home.mei".to_string()));
        assert!(keys.contains(&"scenes/home.mei".to_string()));
    }

    #[test]
    fn v2_app_layout_paths() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::write(ws.join("workspace.json"), r#"{"schemaVersion":2}"#).expect("write");
        let app = ws.join("apps/zhifa");
        fs::create_dir_all(app.join("src")).expect("mkdir");
        fs::write(app.join("src/main.mei"), "app(id=zhifa)").expect("write");
        assert!(is_v2_app_root(&app));
        assert_eq!(resolve_app_root(ws, "zhifa"), app);
        assert_eq!(resolve_app_main_path(&app), app.join("src/main.mei"));
        let env_dir = app.join("env/WS-20260228.0");
        fs::create_dir_all(env_dir.join("build")).expect("mkdir build");
        fs::create_dir_all(env_dir.join("var")).expect("mkdir var");
        #[cfg(unix)]
        std::os::unix::fs::symlink("WS-20260228.0", app.join("env/current")).expect("symlink");
        assert_eq!(resolve_app_build_root(&app), env_dir.join("build"));
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

    #[test]
    fn resolve_app_mei_file_path_legacy_dot_stock_templates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::write(
            ws.join("workspace.json"),
            r#"{"schemaVersion":2,"paths":{"templates":"stock/templates"}}"#,
        )
        .expect("write");
        let tpl = ws.join("stock/templates/cockpit/panel/panel-screen-header.mei");
        fs::create_dir_all(tpl.parent().expect("parent")).expect("mkdir");
        fs::write(&tpl, "panel(id=screen_header_shell)").expect("write");
        let app = ws.join("apps/zhifa");
        fs::create_dir_all(app.join("src/scenes")).expect("mkdir");
        let resolved = resolve_app_mei_file_path(
            &app,
            "../.stock/templates/cockpit/panel/panel-screen-header.mei",
        );
        assert_eq!(resolved, tpl);
        assert!(resolved.is_file());
    }

    #[test]
    fn resolve_spbjw_legacy_panel_template_when_workspace_present() {
        let ws = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join("workspaces/ws-spbjw");
        let app = ws.join("apps/zhifa");
        let tpl = ws.join("stock/templates/cockpit/panel/panel-screen-header.mei");
        if !app.is_dir() || !tpl.is_file() {
            return;
        }
        let resolved = resolve_app_mei_file_path(
            &app,
            "../.stock/templates/cockpit/panel/panel-screen-header.mei",
        );
        assert_eq!(resolved, tpl);
    }

    fn resolve_app_main_path(app_root: &Path) -> PathBuf {
        super::super::io::resolve_app_main_path(app_root)
    }
}
