use super::build_generation::is_build_generation_tag;
use super::env_paths::app_env_var_dir;

use std::cell::RefCell;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct PrebuildStoreOverride {
    build: PathBuf,
    var: PathBuf,
}

thread_local! {
    static PREBUILD_OVERRIDE: RefCell<Option<PrebuildStoreOverride>> = const { RefCell::new(None) };
}

fn resolve_var_dir_for_build_root(app_root: &Path, build: &Path) -> Option<PathBuf> {
    if build.file_name().and_then(|n| n.to_str()) == Some("build") {
        if let Some(env_dir) = build.parent() {
            if let Some(ver) = env_dir
                .file_name()
                .and_then(|n| n.to_str())
                .filter(|s| is_build_generation_tag(s))
            {
                return Some(app_env_var_dir(app_root, ver));
            }
            return Some(env_dir.join("var"));
        }
    }
    let ver = build
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty() && is_build_generation_tag(s))?;
    Some(app_env_var_dir(app_root, ver))
}

pub fn set_prebuild_build_root_override(app_root: &Path, store_dir: Option<&Path>) {
    PREBUILD_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = store_dir.and_then(|build| {
            let var = resolve_var_dir_for_build_root(app_root, build)?;
            Some(PrebuildStoreOverride {
                build: build.to_path_buf(),
                var,
            })
        });
    });
}

pub fn clear_prebuild_build_root_override() {
    PREBUILD_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Copy the current thread's prebuild store override (for worker threads).
pub fn snapshot_prebuild_build_root_override() -> Option<PrebuildStoreOverrideSnapshot> {
    PREBUILD_OVERRIDE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|value| PrebuildStoreOverrideSnapshot {
                build: value.build.clone(),
                var: value.var.clone(),
            })
    })
}

#[derive(Debug, Clone)]
pub struct PrebuildStoreOverrideSnapshot {
    pub build: PathBuf,
    pub var: PathBuf,
}

/// Restore a snapshot on a worker thread before touching build/var store paths.
pub fn restore_prebuild_build_root_override(snapshot: Option<PrebuildStoreOverrideSnapshot>) {
    PREBUILD_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = snapshot.map(|value| PrebuildStoreOverride {
            build: value.build,
            var: value.var,
        });
    });
}

pub(super) fn prebuild_build_root_override() -> Option<PathBuf> {
    PREBUILD_OVERRIDE.with(|cell| cell.borrow().as_ref().map(|value| value.build.clone()))
}

pub(crate) fn prebuild_var_root_override() -> Option<PathBuf> {
    PREBUILD_OVERRIDE.with(|cell| cell.borrow().as_ref().map(|value| value.var.clone()))
}
