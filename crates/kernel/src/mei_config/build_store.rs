//! v2 build store: `build/store/{buildId}/`, workspace `deploy/state/links.json`, promote/rollback.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::io::load_workspace_config;
use super::types::{
    APP_BUILD_ACTIVE_REL, APP_BUILD_STORE_REL, APP_VAR_ACTIVE_REL, APP_VAR_STORE_REL,
    BUILD_MANIFEST_FILENAME, DEPLOY_LINKS_REL, TOOLCHAIN_ACTIVE_REL, TOOLCHAIN_STORE_REL,
    WORKSPACE_AGENT_LOCAL_DIR_REL, WORKSPACE_HOSTS_DIR_REL, WORKSPACE_PLATFORM_DIR_REL,
};
use super::workspace_paths::{
    resolve_app_root, resolve_apps_root, resolve_deploy_root, resolve_symlink_target_from_link,
    resolve_toolchain_root,
};

pub const LINKS_STATE_SCHEMA: &str = "mei-workspace-links-v1";
pub const BUILD_MANIFEST_SCHEMA: &str = "mei-build-manifest-v1";
pub const DEV_TOOLCHAIN_VERSION: &str = "0.0.0-dev-local";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolchainLinks {
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub previous: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildLinks {
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub candidate: Option<String>,
    #[serde(default)]
    pub previous: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinksState {
    #[serde(rename = "schemaVersion", default = "default_links_schema")]
    pub schema_version: String,
    #[serde(default, rename = "sourceRevision")]
    pub source_revision: Option<String>,
    #[serde(default, rename = "stockRevision")]
    pub stock_revision: Option<String>,
    #[serde(default)]
    pub toolchain: ToolchainLinks,
    #[serde(default)]
    pub build: BuildLinks,
}

fn default_links_schema() -> String {
    LINKS_STATE_SCHEMA.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "buildId")]
    pub build_id: String,
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(rename = "toolchainVersion")]
    pub toolchain_version: String,
    #[serde(default, rename = "sourceRevision")]
    pub source_revision: Option<String>,
    #[serde(default, rename = "stockRevision")]
    pub stock_revision: Option<String>,
    #[serde(rename = "finishedAt")]
    pub finished_at: String,
}

pub fn deploy_links_path(source_root: &Path) -> PathBuf {
    resolve_deploy_root(source_root).join(DEPLOY_LINKS_REL)
}

pub fn read_links_state(source_root: &Path) -> Result<LinksState> {
    let path = deploy_links_path(source_root);
    if !path.is_file() {
        return Ok(LinksState::default());
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read deploy links {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse deploy links {}", path.display()))
}

pub fn write_links_state(source_root: &Path, links: &LinksState) -> Result<()> {
    let path = deploy_links_path(source_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut value = links.clone();
    if value.schema_version.is_empty() {
        value.schema_version = LINKS_STATE_SCHEMA.to_string();
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&value)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

pub fn resolve_toolchain_version(source_root: &Path) -> String {
    let manifest = resolve_toolchain_root(source_root).join("MANIFEST.json");
    if manifest.is_file() {
        let path = manifest;
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(v) = json.get("version").and_then(|v| v.as_str()) {
                    let trimmed = v.trim();
                    if !trimmed.is_empty() {
                        return trimmed.to_string();
                    }
                }
            }
        }
    }
    let cfg = load_workspace_config(source_root);
    if let Some(pin) = cfg.toolchain.pin.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return pin.to_string();
    }
    DEV_TOOLCHAIN_VERSION.to_string()
}

pub fn generate_build_id(toolchain_version: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let utc = format_timestamp_utc(now);
    let version = toolchain_version.trim();
    if version.is_empty() {
        format!("{utc}-{DEV_TOOLCHAIN_VERSION}")
    } else {
        format!("{utc}-{version}")
    }
}

fn format_timestamp_utc(epoch_secs: u64) -> String {
    let days_since_epoch = epoch_secs / 86400;
    let time_of_day = epoch_secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    let (year, month, day) = civil_from_days(days_since_epoch as i64);
    format!(
        "{year:04}{month:02}{day:02}T{hours:02}{minutes:02}{seconds:02}"
    )
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m, d)
}

pub fn app_build_store_dir(app_root: &Path, build_id: &str) -> PathBuf {
    app_root.join(APP_BUILD_STORE_REL).join(build_id.trim())
}

pub fn app_var_store_dir(app_root: &Path, build_id: &str) -> PathBuf {
    app_root.join(APP_VAR_STORE_REL).join(build_id.trim())
}

pub fn app_build_active_link(app_root: &Path) -> PathBuf {
    app_root.join(APP_BUILD_ACTIVE_REL)
}

pub fn app_var_active_link(app_root: &Path) -> PathBuf {
    app_root.join(APP_VAR_ACTIVE_REL)
}

pub fn resolve_symlink_target(link: &Path) -> Option<PathBuf> {
    resolve_symlink_target_from_link(link)
}

pub fn resolve_app_build_root_following_active(app_root: &Path) -> PathBuf {
    if let Some(override_root) = prebuild_build_root_override() {
        return override_root;
    }
    let active = app_build_active_link(app_root);
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

pub fn resolve_active_build_id(source_root: &Path, app_id: &str) -> Option<String> {
    if let Ok(links) = read_links_state(source_root) {
        if let Some(id) = links
            .build
            .active
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(id.to_string());
        }
    }
    let app_root = resolve_app_root(source_root, app_id);
    let active = app_build_active_link(&app_root);
    if active.is_symlink() {
        return active
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
    }
    None
}

pub fn write_build_manifest(
    store_dir: &Path,
    manifest: &BuildManifest,
) -> Result<()> {
    fs::create_dir_all(store_dir)?;
    let path = store_dir.join(BUILD_MANIFEST_FILENAME);
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(manifest)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn read_build_manifest(store_dir: &Path) -> Result<Option<BuildManifest>> {
    let path = store_dir.join(BUILD_MANIFEST_FILENAME);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

pub struct PrebuildGeneration {
    pub build_id: String,
    pub toolchain_version: String,
    pub store_dirs: BTreeMap<String, PathBuf>,
}

pub fn begin_prebuild_generation(source_root: &Path, app_ids: &[String]) -> Result<PrebuildGeneration> {
    let toolchain_version = resolve_toolchain_version(source_root);
    let build_id = generate_build_id(&toolchain_version);
    let mut store_dirs = BTreeMap::new();
    for app_id in app_ids {
        let app_root = resolve_app_root(source_root, app_id);
        let store_dir = app_build_store_dir(&app_root, &build_id);
        fs::create_dir_all(&store_dir)?;
        let var_store = app_var_store_dir(&app_root, &build_id);
        fs::create_dir_all(var_store.join("cache"))?;
        fs::create_dir_all(var_store.join("eval-results"))?;
        store_dirs.insert(app_id.clone(), store_dir);
    }
    Ok(PrebuildGeneration {
        build_id,
        toolchain_version,
        store_dirs,
    })
}

pub fn finish_prebuild_generation(
    source_root: &Path,
    generation: &PrebuildGeneration,
    app_ids: &[String],
    source_revision: Option<&str>,
    stock_revision: Option<&str>,
) -> Result<()> {
    let finished_at = chrono_like_rfc3339();
    for app_id in app_ids {
        let store_dir = generation
            .store_dirs
            .get(app_id)
            .cloned()
            .unwrap_or_else(|| app_build_store_dir(&resolve_app_root(source_root, app_id), &generation.build_id));
        write_build_manifest(
            &store_dir,
            &BuildManifest {
                schema_version: BUILD_MANIFEST_SCHEMA.to_string(),
                build_id: generation.build_id.clone(),
                app_id: app_id.clone(),
                toolchain_version: generation.toolchain_version.clone(),
                source_revision: source_revision.map(str::to_string),
                stock_revision: stock_revision.map(str::to_string),
                finished_at: finished_at.clone(),
            },
        )?;
    }
    let mut links = read_links_state(source_root).unwrap_or_default();
    links.build.candidate = Some(generation.build_id.clone());
    if links
        .toolchain
        .active
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        links.toolchain.active = Some(generation.toolchain_version.clone());
    }
    if source_revision.is_some() {
        links.source_revision = source_revision.map(str::to_string);
    }
    if stock_revision.is_some() {
        links.stock_revision = stock_revision.map(str::to_string);
    }
    write_links_state(source_root, &links)?;
    Ok(())
}

fn chrono_like_rfc3339() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d) = civil_from_days((now / 86400) as i64);
    let tod = now % 86400;
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

pub fn promote_build(source_root: &Path, build_id: Option<&str>) -> Result<String> {
    let mut links = read_links_state(source_root)?;
    let target = build_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| links.build.candidate.clone())
        .ok_or_else(|| anyhow::anyhow!("no build candidate to promote"))?;
    if links.build.active.as_deref() == Some(target.as_str()) {
        return Ok(target);
    }
    links.build.previous = links.build.active.take();
    links.build.active = Some(target.clone());
    links.build.candidate = None;
    apply_build_symlinks_for_all_apps(source_root, &target)?;
    write_links_state(source_root, &links)?;
    Ok(target)
}

pub fn rollback_build(source_root: &Path) -> Result<String> {
    let mut links = read_links_state(source_root)?;
    let target = links
        .build
        .previous
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no previous build to rollback"))?;
    links.build.active = Some(target.clone());
    apply_build_symlinks_for_all_apps(source_root, &target)?;
    write_links_state(source_root, &links)?;
    Ok(target)
}

fn apply_build_symlinks_for_all_apps(source_root: &Path, build_id: &str) -> Result<()> {
    let apps_root = resolve_apps_root(source_root);
    if !apps_root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&apps_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let app_root = entry.path();
        set_active_symlink(&app_build_active_link(&app_root), &app_build_store_dir(&app_root, build_id))?;
        set_active_symlink(&app_var_active_link(&app_root), &app_var_store_dir(&app_root, build_id))?;
    }
    Ok(())
}

fn set_active_symlink(link: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    if link.is_symlink() || link.exists() {
        fs::remove_file(link).or_else(|_| fs::remove_dir_all(link))?;
    }
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(link)?;
        let marker = link.join(".mei-build-target");
        fs::write(marker, target.to_string_lossy().as_bytes())?;
    }
    Ok(())
}

thread_local! {
    static PREBUILD_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub fn set_prebuild_build_root_override(_app_root: &Path, store_dir: Option<&Path>) {
    PREBUILD_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = store_dir.map(|dir| dir.to_path_buf());
    });
}

pub fn clear_prebuild_build_root_override() {
    PREBUILD_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

fn prebuild_build_root_override() -> Option<PathBuf> {
    PREBUILD_OVERRIDE.with(|cell| cell.borrow().clone())
}

pub fn migrate_legacy_app_mei(app_root: &Path) -> Result<()> {
    let legacy = app_root.join(".mei");
    if !legacy.is_dir() {
        return Ok(());
    }
    let build_root = resolve_app_build_root_following_active(app_root);
    for sub in ["prebuild", "graph"] {
        let from = legacy.join(sub);
        if !from.is_dir() {
            continue;
        }
        let to = build_root.join(sub);
        merge_dir_recursive(&from, &to)?;
    }
    fs::remove_dir_all(&legacy).ok();
    Ok(())
}

/// 一次性迁移工作区根级 legacy `.mei/` → `runtime/hosts`、`runtime/agent`、`runtime/platform/`。
pub fn migrate_legacy_workspace_mei(source_root: &Path) -> Result<()> {
    let legacy = source_root.join(".mei");
    if !legacy.is_dir() {
        return Ok(());
    }
    let mappings = [
        ("local/hosts", WORKSPACE_HOSTS_DIR_REL),
        ("local/agent", WORKSPACE_AGENT_LOCAL_DIR_REL),
    ];
    for (from_suffix, to_rel) in mappings {
        let from = legacy.join(from_suffix);
        if !from.is_dir() {
            continue;
        }
        merge_dir_recursive(&from, &source_root.join(to_rel))?;
    }
    let legacy_runtime = legacy.join("runtime");
    if legacy_runtime.is_dir() {
        merge_dir_recursive(&legacy_runtime, &source_root.join(WORKSPACE_PLATFORM_DIR_REL))?;
    }
    let remaining: Vec<_> = fs::read_dir(&legacy)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    if remaining.is_empty() {
        fs::remove_dir_all(&legacy).ok();
    }
    Ok(())
}

pub fn toolchain_store_dir(source_root: &Path, toolchain_version: &str) -> PathBuf {
    resolve_toolchain_root(source_root)
        .join(TOOLCHAIN_STORE_REL)
        .join(toolchain_version.trim())
}

pub fn apply_toolchain_store_symlinks(source_root: &Path, toolchain_version: &str) -> Result<()> {
    let toolchain_root = resolve_toolchain_root(source_root);
    let store_dir = toolchain_store_dir(source_root, toolchain_version);
    let store_bin = store_dir.join("bin");
    fs::create_dir_all(&store_bin)?;
    migrate_flat_toolchain_bin_to_store(&toolchain_root, &store_bin)?;
    set_active_symlink(&toolchain_root.join(TOOLCHAIN_ACTIVE_REL), &store_dir)?;
    set_active_symlink(&toolchain_root.join("bin"), &store_bin)?;
    Ok(())
}

pub fn record_toolchain_install_links(source_root: &Path, toolchain_version: &str) -> Result<()> {
    let version = toolchain_version.trim();
    if version.is_empty() {
        return Ok(());
    }
    let mut links = read_links_state(source_root).unwrap_or_default();
    if links.toolchain.active.as_deref() == Some(version) {
        return Ok(());
    }
    links.toolchain.previous = links.toolchain.active.take();
    links.toolchain.active = Some(version.to_string());
    write_links_state(source_root, &links)
}

fn migrate_flat_toolchain_bin_to_store(toolchain_root: &Path, store_bin: &Path) -> Result<()> {
    let flat_bin = toolchain_root.join("bin");
    if flat_bin.is_symlink() {
        return Ok(());
    }
    if !flat_bin.is_dir() {
        return Ok(());
    }
    merge_dir_recursive(&flat_bin, store_bin)?;
    fs::remove_dir_all(&flat_bin)
        .with_context(|| format!("remove flat toolchain bin {}", flat_bin.display()))?;
    Ok(())
}

fn merge_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            merge_dir_recursive(&entry.path(), &dest)?;
        } else if !dest.exists() {
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_id_contains_toolchain_version() {
        let id = generate_build_id("2026.6.1-abc1234");
        assert!(id.ends_with("2026.6.1-abc1234"));
        assert!(id.contains('T'));
    }

    #[test]
    fn links_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("deploy/state")).expect("mkdir");
        fs::write(ws.join("workspace.json"), r#"{"schemaVersion":2}"#).expect("write");
        let mut links = LinksState::default();
        links.build.candidate = Some("20260625T120000-dev".into());
        write_links_state(ws, &links).expect("write");
        let loaded = read_links_state(ws).expect("read");
        assert_eq!(loaded.build.candidate.as_deref(), Some("20260625T120000-dev"));
    }
}
