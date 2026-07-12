//! App-private runtime paths under `deploy/runtime/apps/{appId}/`.
//!
//! Generation trees (`apps/{app}/env/WS-*`) are **read-only build artifacts**. Mutable
//! runtime state (eval cache, MRG tiers, bootstrap, logs, meta) must live under the
//! ephemeral app root resolved by these helpers.
//!
//! Legacy layout `deploy/runtime/instances/{instanceId}/` is read for one
//! compatibility round; new writes only use the app root.

use std::path::{Path, PathBuf};

use mei_lang_kernel::resolve_app_root;

use crate::app_launch::app_runtime_root;

/// Ephemeral root for an app: `{workspace}/deploy/runtime/apps/{app_id}/`.
pub fn instance_runtime_root(workspace: &Path, app_id: &str) -> PathBuf {
    app_runtime_root(workspace, app_id)
}

/// Legacy instance-private root: `{workspace}/deploy/runtime/instances/{instance_id}/`.
pub fn legacy_instance_runtime_root(workspace: &Path, instance_id: &str) -> PathBuf {
    workspace
        .join("deploy/runtime/instances")
        .join(sanitize_segment(instance_id))
}

/// `{app_root}/var/` — general mutable runtime directory.
pub fn instance_var_dir(workspace: &Path, app_id: &str) -> PathBuf {
    instance_runtime_root(workspace, app_id).join("var")
}

/// `{app_root}/var/eval-cache/`
pub fn instance_eval_cache_dir(workspace: &Path, app_id: &str) -> PathBuf {
    instance_var_dir(workspace, app_id).join("eval-cache")
}

/// `{app_root}/var/client-bootstrap/`
pub fn instance_bootstrap_dir(workspace: &Path, app_id: &str) -> PathBuf {
    instance_var_dir(workspace, app_id).join("client-bootstrap")
}

/// `{app_root}/var/mrg/memory/`
pub fn instance_mrg_memory_dir(workspace: &Path, app_id: &str) -> PathBuf {
    instance_var_dir(workspace, app_id)
        .join("mrg")
        .join("memory")
}

/// `{app_root}/var/mrg/disk/`
pub fn instance_mrg_disk_dir(workspace: &Path, app_id: &str) -> PathBuf {
    instance_var_dir(workspace, app_id).join("mrg").join("disk")
}

/// `{app_root}/logs/`
pub fn instance_logs_dir(workspace: &Path, app_id: &str) -> PathBuf {
    instance_runtime_root(workspace, app_id).join("logs")
}

/// `{app_root}/meta/`
pub fn instance_meta_dir(workspace: &Path, app_id: &str) -> PathBuf {
    instance_runtime_root(workspace, app_id).join("meta")
}

/// Pinned generation root: `apps/{app}/env/{generation}/` (does **not** follow `env/current`).
///
/// Treat as read-only; never write eval/bootstrap/cache here.
pub fn pinned_generation_root(workspace: &Path, app_id: &str, generation: &str) -> PathBuf {
    resolve_app_root(workspace, app_id)
        .join("env")
        .join(generation.trim())
}

fn sanitize_segment(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "_invalid"
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn instance_path_helpers_nest_under_deploy_runtime_apps() {
        let ws = PathBuf::from("/tmp/ws");
        let root = instance_runtime_root(&ws, "mini-data");
        assert_eq!(root, PathBuf::from("/tmp/ws/deploy/runtime/apps/mini-data"));
        assert_eq!(
            instance_eval_cache_dir(&ws, "mini-data"),
            root.join("var/eval-cache")
        );
        assert_eq!(
            instance_bootstrap_dir(&ws, "mini-data"),
            root.join("var/client-bootstrap")
        );
        assert_eq!(instance_var_dir(&ws, "mini-data"), root.join("var"));
        assert_eq!(
            instance_mrg_memory_dir(&ws, "mini-data"),
            root.join("var/mrg/memory")
        );
        assert_eq!(
            instance_mrg_disk_dir(&ws, "mini-data"),
            root.join("var/mrg/disk")
        );
        assert_eq!(instance_logs_dir(&ws, "mini-data"), root.join("logs"));
        assert_eq!(instance_meta_dir(&ws, "mini-data"), root.join("meta"));
        assert_eq!(
            legacy_instance_runtime_root(&ws, "inst-a"),
            PathBuf::from("/tmp/ws/deploy/runtime/instances/inst-a")
        );
    }

    #[test]
    fn pinned_generation_root_does_not_use_env_current() {
        let ws = PathBuf::from("/tmp/ws");
        let gen = pinned_generation_root(&ws, "mini-data", "WS-20260712.1");
        assert_eq!(
            gen,
            PathBuf::from("/tmp/ws/apps/mini-data/env/WS-20260712.1")
        );
        assert!(!gen.to_string_lossy().contains("env/current"));
    }

    #[test]
    fn distinct_app_ids_yield_distinct_roots() {
        let ws = PathBuf::from("/tmp/ws");
        assert_ne!(
            instance_runtime_root(&ws, "app-a"),
            instance_runtime_root(&ws, "app-b")
        );
    }
}
