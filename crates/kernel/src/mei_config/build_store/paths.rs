use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::mei_config::io::load_workspace_config;
use crate::mei_config::workspace_paths::{
    resolve_app_root, resolve_symlink_target_from_link, resolve_toolchain_root,
};

use super::prebuild_override::{prebuild_build_root_override, prebuild_var_root_override};
use super::types::{
    read_links_state, BuildManifest, DEV_TOOLCHAIN_ALIAS,
};

use std::fs;

use crate::mei_config::types::TOOLCHAIN_ACTIVE_REL;

use super::env_paths::{
    app_build_active_link, app_var_active_link, build_manifest_path, env_version_from_build_root,
};

fn read_manifest_version(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let raw = fs::read_to_string(path).ok()?;
    let json = serde_json::from_str::<serde_json::Value>(&raw).ok()?;
    let trimmed = json.get("version")?.as_str()?.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn active_toolchain_manifest_path(source_root: &Path) -> Option<PathBuf> {
    let toolchain_root = resolve_toolchain_root(source_root);
    let active = toolchain_root.join(TOOLCHAIN_ACTIVE_REL);
    if active.is_symlink() {
        return resolve_symlink_target(&active).map(|dir| dir.join("MANIFEST.json"));
    }
    if active.is_dir() {
        return Some(active.join("MANIFEST.json"));
    }
    None
}

/// Toolchain version for env ver: active store MANIFEST → links.active → workspace pin → flat MANIFEST → `latest`.
pub fn resolve_toolchain_version(source_root: &Path) -> String {
    if let Some(manifest) = active_toolchain_manifest_path(source_root) {
        if let Some(version) = read_manifest_version(&manifest) {
            return version;
        }
    }
    if let Ok(links) = read_links_state(source_root) {
        if let Some(active) = links
            .toolchain
            .active
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return active.to_string();
        }
    }
    let cfg = load_workspace_config(source_root);
    if let Some(pin) = cfg.toolchain.pin.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return pin.to_string();
    }
    let flat_manifest = resolve_toolchain_root(source_root).join("MANIFEST.json");
    if let Some(version) = read_manifest_version(&flat_manifest) {
        return version;
    }
    DEV_TOOLCHAIN_ALIAS.to_string()
}

pub fn resolve_dev_toolchain_version() -> &'static str {
    DEV_TOOLCHAIN_ALIAS
}

pub(super) fn civil_from_days(z: i64) -> (i64, i64, i64) {
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

pub fn resolve_app_var_root_following_active(app_root: &Path) -> PathBuf {
    if let Some(override_root) = prebuild_var_root_override() {
        return override_root;
    }
    let active = app_var_active_link(app_root);
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
        if let Some(target) = resolve_symlink_target(&active) {
            if let Some(ver) = env_version_from_build_root(target.as_path()) {
                return Some(ver);
            }
        }
        return active
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
    }
    None
}

pub fn write_build_manifest(env_dir: &Path, manifest: &BuildManifest) -> Result<()> {
    fs::create_dir_all(env_dir)?;
    let path = build_manifest_path(env_dir);
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(manifest)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn read_build_manifest(env_dir: &Path) -> Result<Option<BuildManifest>> {
    let path = build_manifest_path(env_dir);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}
