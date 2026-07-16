use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::mei_config::io::load_workspace_config;
use crate::mei_config::types::{
    APP_ENV_BUILD_REL, APP_ENV_CURRENT_REL, APP_ENV_REL, APP_ENV_VAR_REL, BUILD_MANIFEST_FILENAME,
};
use crate::mei_config::workspace_paths::{resolve_app_root, resolve_symlink_target_from_link};

use super::build_generation::{
    is_build_generation_tag, require_build_generation_tag, resolve_build_generation_config,
    resolve_build_generation_for_prebuild, resolve_version_display_identity_for_app,
};
use super::types::{is_dev_toolchain_alias, DEV_TOOLCHAIN_ALIAS, DEV_TOOLCHAIN_VERSION};

const APP_RUNTIME_APP_ID_ENV: &str = "MEI_APP_RUNTIME_APP_ID";
const APP_RUNTIME_GENERATION_ENV: &str = "MEI_APP_RUNTIME_GENERATION";

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
    resolve_build_generation_config(source_root).date
}

pub fn resolve_env_generation_id(source_root: &Path) -> String {
    resolve_build_generation_config(source_root).tag
}

pub fn resolve_env_generation_id_for_prebuild(source_root: &Path) -> String {
    resolve_build_generation_for_prebuild(source_root).tag
}

pub fn generate_build_id(source_root: &Path) -> String {
    resolve_env_generation_id(source_root)
}

pub fn normalize_env_generation_id(source_root: &Path, raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(resolve_env_generation_id(source_root));
    }
    Ok(require_build_generation_tag(trimmed)?.tag)
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

pub fn app_env_current_link(app_root: &Path) -> PathBuf {
    app_root.join(APP_ENV_CURRENT_REL)
}

pub fn app_env_root(app_root: &Path) -> PathBuf {
    app_root.join(APP_ENV_REL)
}

pub fn build_manifest_path(env_dir: &Path) -> PathBuf {
    env_dir.join(BUILD_MANIFEST_FILENAME)
}

pub fn env_generation_from_env_dir(env_dir: &Path) -> Option<String> {
    let name = env_dir.file_name()?.to_str()?.trim();
    if is_build_generation_tag(name) {
        Some(name.to_string())
    } else {
        None
    }
}

/// Follow `env/current` symlink to the active env directory (`…/env/{id}`).
pub fn resolve_app_env_dir_following_current(app_root: &Path) -> Option<PathBuf> {
    if let Some(pinned) = app_runtime_pinned_env_dir(app_root) {
        return Some(pinned);
    }
    let current = app_env_current_link(app_root);
    if current.is_symlink() {
        return resolve_symlink_target_from_link(&current);
    }
    #[cfg(not(unix))]
    if current.is_dir() {
        let marker = current.join(".mei-build-target");
        if marker.is_file() {
            if let Ok(raw) = std::fs::read_to_string(marker) {
                let target = PathBuf::from(raw.trim());
                if target.is_absolute() {
                    return Some(target);
                }
                return current.parent().map(|parent| parent.join(target));
            }
        }
    }
    None
}

fn app_runtime_pinned_env_dir(app_root: &Path) -> Option<PathBuf> {
    let expected_app = std::env::var(APP_RUNTIME_APP_ID_ENV).ok()?;
    let actual_app = app_root.file_name()?.to_str()?;
    if expected_app.trim() != actual_app {
        return None;
    }
    let generation = std::env::var(APP_RUNTIME_GENERATION_ENV).ok()?;
    let generation = generation.trim();
    if !is_build_generation_tag(generation) {
        return None;
    }
    let pinned = app_env_dir(app_root, generation);
    pinned.is_dir().then_some(pinned)
}

pub fn require_app_env_dir_following_current(app_root: &Path) -> Result<PathBuf> {
    resolve_app_env_dir_following_current(app_root).ok_or_else(|| {
        anyhow::anyhow!(
            "missing env/current for app {} (run build prepare / promote first)",
            app_root.display()
        )
    })
}

/// Active build generation for one app, from `env/current` only.
pub fn resolve_app_build_generation_from_current(app_root: &Path) -> Result<String> {
    let env_dir = require_app_env_dir_following_current(app_root)?;
    env_generation_from_env_dir(env_dir.as_path()).ok_or_else(|| {
        anyhow::anyhow!(
            "invalid env/current target for app {} (expected WS-yyyymmdd.fixver)",
            app_root.display()
        )
    })
}

pub fn resolve_workspace_default_app_id(source_root: &Path) -> Option<String> {
    let cfg = load_workspace_config(source_root);
    cfg.workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn resolve_workspace_app_build_generations(
    source_root: &Path,
    app_ids: &[String],
) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for app_id in app_ids {
        let app_root = resolve_app_root(source_root, app_id);
        let generation = resolve_app_build_generation_from_current(app_root.as_path())?;
        out.insert(app_id.clone(), generation);
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveBuildIdentity {
    pub meilang_version: String,
    pub build_generation: String,
    pub workspace_version: String,
    pub env_generation_id: String,
}

pub fn resolve_active_build_identity(source_root: &Path) -> ActiveBuildIdentity {
    resolve_active_build_identity_for_app(source_root, None, None)
        .unwrap_or_else(|err| panic!("{err}"))
}

pub fn resolve_active_build_identity_with_hint(
    source_root: &Path,
    meilang_hint: Option<&str>,
) -> Result<ActiveBuildIdentity> {
    resolve_active_build_identity_for_app(source_root, None, meilang_hint)
}

pub fn resolve_active_build_identity_for_app(
    source_root: &Path,
    app_id: Option<&str>,
    meilang_hint: Option<&str>,
) -> Result<ActiveBuildIdentity> {
    let display = resolve_version_display_identity_for_app(source_root, app_id, meilang_hint)?;
    let date = require_build_generation_tag(display.build_generation.as_str())?.date;
    Ok(ActiveBuildIdentity {
        meilang_version: display.meilang_version,
        build_generation: display.build_generation.clone(),
        workspace_version: date,
        env_generation_id: display.env_generation_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn app_runtime_generation_override_pins_env_without_moving_current() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let tmp = tempfile::tempdir().expect("tempdir");
        let app_root = tmp.path().join("apps/demo");
        let old = app_env_dir(app_root.as_path(), "WS-20260714.0");
        let candidate = app_env_dir(app_root.as_path(), "WS-20260715.0");
        std::fs::create_dir_all(&old).expect("old");
        std::fs::create_dir_all(&candidate).expect("candidate");
        #[cfg(unix)]
        std::os::unix::fs::symlink("WS-20260714.0", app_env_current_link(&app_root))
            .expect("current");

        std::env::set_var(APP_RUNTIME_APP_ID_ENV, "demo");
        std::env::set_var(APP_RUNTIME_GENERATION_ENV, "WS-20260715.0");
        let instance_var = tmp.path().join("runtime/demo/instances/inst-a/var");
        std::env::set_var("MEI_APP_RUNTIME_VAR_ROOT", &instance_var);
        assert_eq!(
            resolve_app_env_dir_following_current(app_root.as_path()),
            Some(candidate.clone())
        );
        assert_eq!(
            super::super::paths::resolve_app_var_root_following_active(app_root.as_path()),
            instance_var
        );
        let generation_var = candidate.join(APP_ENV_VAR_REL);
        assert_eq!(
            super::super::paths::resolve_app_build_var_root_following_active(app_root.as_path()),
            generation_var
        );
        assert_eq!(
            crate::mei_config::resolve_app_data_snapshot_root(app_root.as_path()),
            generation_var.join("data-snapshots")
        );
        std::env::remove_var(APP_RUNTIME_APP_ID_ENV);
        std::env::remove_var(APP_RUNTIME_GENERATION_ENV);
        std::env::remove_var("MEI_APP_RUNTIME_VAR_ROOT");
    }
}
