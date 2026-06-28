use std::path::{Path, PathBuf};

use crate::mei_config::io::load_workspace_config;
use crate::mei_config::types::{
    APP_BUILD_ACTIVE_REL, APP_ENV_BUILD_REL, APP_ENV_REL, APP_ENV_VAR_REL, APP_VAR_ACTIVE_REL,
    BUILD_MANIFEST_FILENAME,
};

use super::paths::resolve_toolchain_version;
use super::types::{DEV_TOOLCHAIN_ALIAS, DEV_TOOLCHAIN_VERSION, is_dev_toolchain_alias};

const ENV_COMPOSITE_SEP: &str = "-ws";

/// Toolchain segment only (dev alias → `latest`).
pub fn resolve_toolchain_segment(toolchain_version: &str) -> String {
    let version = toolchain_version.trim();
    if version.is_empty() {
        return DEV_TOOLCHAIN_VERSION.to_string();
    }
    if is_dev_toolchain_alias(version) {
        return DEV_TOOLCHAIN_ALIAS.to_string();
    }
    version.to_string()
}

/// Legacy alias for toolchain segment.
pub fn resolve_env_version(toolchain_version: &str) -> String {
    resolve_toolchain_segment(toolchain_version)
}

pub fn resolve_workspace_version(source_root: &Path) -> String {
    let cfg = load_workspace_config(source_root);
    cfg.workspace
        .version_trimmed()
        .map(normalize_workspace_version_segment)
        .unwrap_or_else(|| "dev".to_string())
}

pub fn normalize_workspace_version_segment(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix("WS")
        .or_else(|| trimmed.strip_prefix("ws"))
        .unwrap_or(trimmed)
        .trim();
    if stripped.is_empty() {
        "dev".to_string()
    } else {
        stripped.to_string()
    }
}

/// Composite env directory id: `{toolchain}-ws{workspace}` (e.g. `2.0.1-ws20260228`).
pub fn format_env_generation_id(toolchain_version: &str, workspace_version: &str) -> String {
    format!(
        "{}-ws{}",
        resolve_toolchain_segment(toolchain_version),
        normalize_workspace_version_segment(workspace_version)
    )
}

pub fn resolve_env_generation_id(source_root: &Path) -> String {
    format_env_generation_id(
        resolve_toolchain_version(source_root).as_str(),
        resolve_workspace_version(source_root).as_str(),
    )
}

/// Script-compat: returns composite env generation id for the workspace.
pub fn generate_build_id(source_root: &Path) -> String {
    resolve_env_generation_id(source_root)
}

pub fn is_composite_env_generation_id(raw: &str) -> bool {
    raw.contains(ENV_COMPOSITE_SEP)
}

pub fn parse_composite_env_generation_id(raw: &str) -> Option<(String, String)> {
    let trimmed = raw.trim();
    let idx = trimmed.rfind(ENV_COMPOSITE_SEP)?;
    let toolchain = trimmed[..idx].trim();
    let ws = normalize_workspace_version_segment(trimmed[idx + ENV_COMPOSITE_SEP.len()..].trim());
    if toolchain.is_empty() || ws.is_empty() {
        return None;
    }
    Some((toolchain.to_string(), ws))
}

pub fn normalize_env_generation_id(source_root: &Path, raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return resolve_env_generation_id(source_root);
    }
    if trimmed.contains("-wsws") {
        return normalize_env_generation_id(source_root, trimmed.replace("-wsws", "-ws").as_str());
    }
    if is_composite_env_generation_id(trimmed) {
        return trimmed.to_string();
    }
    if let Some(legacy) = parse_ver_from_legacy_build_id(trimmed) {
        return format_env_generation_id(
            legacy.as_str(),
            resolve_workspace_version(source_root).as_str(),
        );
    }
    format_env_generation_id(trimmed, resolve_workspace_version(source_root).as_str())
}

pub fn app_env_dir(app_root: &Path, env_version: &str) -> PathBuf {
    app_root.join(APP_ENV_REL).join(env_version.trim())
}

pub fn app_env_build_dir(app_root: &Path, env_version: &str) -> PathBuf {
    app_env_dir(app_root, env_version).join(APP_ENV_BUILD_REL)
}

pub fn app_env_var_dir(app_root: &Path, env_version: &str) -> PathBuf {
    app_env_dir(app_root, env_version).join(APP_ENV_VAR_REL)
}

pub fn app_build_store_dir(app_root: &Path, env_version: &str) -> PathBuf {
    app_env_build_dir(app_root, env_version)
}

pub fn app_var_store_dir(app_root: &Path, env_version: &str) -> PathBuf {
    app_env_var_dir(app_root, env_version)
}

pub fn app_build_active_link(app_root: &Path) -> PathBuf {
    app_root.join(APP_BUILD_ACTIVE_REL)
}

pub fn app_var_active_link(app_root: &Path) -> PathBuf {
    app_root.join(APP_VAR_ACTIVE_REL)
}

pub fn app_env_root(app_root: &Path) -> PathBuf {
    app_root.join(APP_ENV_REL)
}

pub fn build_manifest_path(env_dir: &Path) -> PathBuf {
    env_dir.join(BUILD_MANIFEST_FILENAME)
}

/// Parse composite `env/{id}` from a resolved `build/active` target (`…/env/{id}/build`).
pub fn env_version_from_build_root(build_root: &Path) -> Option<String> {
    if build_root.file_name()?.to_str()? != APP_ENV_BUILD_REL {
        return None;
    }
    let env_dir = build_root.parent()?;
    if env_dir.parent()?.file_name()?.to_str()? != APP_ENV_REL {
        return None;
    }
    env_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
}

/// Parse ver suffix from legacy store dir name `YYYYMMDDTHHMMSS-{ver}`.
pub fn parse_ver_from_legacy_build_id(legacy_build_id: &str) -> Option<String> {
    let trimmed = legacy_build_id.trim();
    let t_pos = trimmed.find('T')?;
    let rest = &trimmed[t_pos + 1..];
    let dash = rest.find('-')?;
    let ver = rest[dash + 1..].trim();
    if ver.is_empty() {
        None
    } else {
        Some(ver.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveBuildIdentity {
    pub toolchain_version: String,
    pub workspace_version: String,
    pub env_generation_id: String,
}

pub fn resolve_active_build_identity(source_root: &Path) -> ActiveBuildIdentity {
    let toolchain_version = resolve_toolchain_version(source_root);
    let workspace_version = resolve_workspace_version(source_root);
    let env_generation_id = format_env_generation_id(
        toolchain_version.as_str(),
        workspace_version.as_str(),
    );
    ActiveBuildIdentity {
        toolchain_version,
        workspace_version,
        env_generation_id,
    }
}

pub fn format_build_identity_display(identity: &ActiveBuildIdentity) -> String {
    format!(
        "MeiLang {} · WS {} · build {}",
        identity.toolchain_version, identity.workspace_version, identity.env_generation_id
    )
}
