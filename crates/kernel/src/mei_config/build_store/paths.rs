use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::mei_config::io::load_workspace_config;
use crate::mei_config::types::{
    APP_BUILD_ACTIVE_REL, APP_BUILD_STORE_REL, APP_VAR_ACTIVE_REL, APP_VAR_STORE_REL,
    BUILD_MANIFEST_FILENAME,
};
use crate::mei_config::workspace_paths::{
    resolve_app_root, resolve_symlink_target_from_link, resolve_toolchain_root,
};

use super::prebuild_override::{prebuild_build_root_override, prebuild_var_root_override};
use super::types::{read_links_state, BuildManifest, DEV_TOOLCHAIN_VERSION};

use std::fs;

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
    let embedded = env!("CARGO_PKG_VERSION").trim();
    if !embedded.is_empty() {
        return embedded.to_string();
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
