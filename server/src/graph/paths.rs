use std::path::{Path, PathBuf};

use mei_lang_kernel::{resolve_app_registry_root, resolve_app_root, resolve_workspace_graph_root};

/// Canonical MRG/MCG graph root: `apps/{app}/env/current/build/registry/`.
pub fn resolve_graph_root(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_app_registry_root(&resolve_app_root(source_root, app_id))
}

/// Legacy workspace graphs path (read-only migration source).
pub fn legacy_workspace_graph_root(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_workspace_graph_root(source_root, app_id)
}

pub fn mcg_registry_path(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_graph_root(source_root, app_id).join("mcg-registry.json")
}

pub fn mrg_registry_path(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_graph_root(source_root, app_id).join("mrg-registry.json")
}

pub fn bridge_path(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_graph_root(source_root, app_id).join("bridge.json")
}
