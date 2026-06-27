use std::cell::RefCell;
use std::path::{Path, PathBuf};

use super::paths::app_var_store_dir;

#[derive(Debug, Clone)]
struct PrebuildStoreOverride {
    build: PathBuf,
    var: PathBuf,
}

thread_local! {
    static PREBUILD_OVERRIDE: RefCell<Option<PrebuildStoreOverride>> = const { RefCell::new(None) };
}

pub fn set_prebuild_build_root_override(app_root: &Path, store_dir: Option<&Path>) {
    PREBUILD_OVERRIDE.with(|cell| {
        *cell.borrow_mut() = store_dir.and_then(|build| {
            let build_id = build.file_name()?.to_str()?.trim();
            if build_id.is_empty() {
                return None;
            }
            Some(PrebuildStoreOverride {
                build: build.to_path_buf(),
                var: app_var_store_dir(app_root, build_id),
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
