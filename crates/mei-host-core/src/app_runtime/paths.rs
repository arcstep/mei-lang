//! Instance-private runtime paths under `deploy/runtime/instances/{instanceId}/`.
//!
//! Generation trees (`apps/{app}/env/WS-*`) are **read-only build artifacts**. Mutable
//! runtime state (eval cache, MRG tiers, bootstrap, logs, meta) must live under the
//! instance root resolved by these helpers.

use std::path::{Path, PathBuf};

use mei_lang_kernel::resolve_app_root;

/// Workspace-relative root for all instance-private mutable state.
///
/// `{workspace}/deploy/runtime/instances/{instance_id}/`
pub fn instance_runtime_root(workspace: &Path, instance_id: &str) -> PathBuf {
    workspace
        .join("deploy/runtime/instances")
        .join(sanitize_instance_id(instance_id))
}

/// `{instance_root}/var/` — general mutable runtime directory.
pub fn instance_var_dir(workspace: &Path, instance_id: &str) -> PathBuf {
    instance_runtime_root(workspace, instance_id).join("var")
}

/// `{instance_root}/var/eval-cache/`
pub fn instance_eval_cache_dir(workspace: &Path, instance_id: &str) -> PathBuf {
    instance_var_dir(workspace, instance_id).join("eval-cache")
}

/// `{instance_root}/var/client-bootstrap/`
pub fn instance_bootstrap_dir(workspace: &Path, instance_id: &str) -> PathBuf {
    instance_var_dir(workspace, instance_id).join("client-bootstrap")
}

/// `{instance_root}/var/mrg/memory/` — MRG in-memory tier spill / pin metadata.
pub fn instance_mrg_memory_dir(workspace: &Path, instance_id: &str) -> PathBuf {
    instance_var_dir(workspace, instance_id)
        .join("mrg")
        .join("memory")
}

/// `{instance_root}/var/mrg/disk/` — MRG disk tier artifacts.
pub fn instance_mrg_disk_dir(workspace: &Path, instance_id: &str) -> PathBuf {
    instance_var_dir(workspace, instance_id).join("mrg").join("disk")
}

/// `{instance_root}/logs/`
pub fn instance_logs_dir(workspace: &Path, instance_id: &str) -> PathBuf {
    instance_runtime_root(workspace, instance_id).join("logs")
}

/// `{instance_root}/meta/` — observed revisions, readiness, and other run metadata.
pub fn instance_meta_dir(workspace: &Path, instance_id: &str) -> PathBuf {
    instance_runtime_root(workspace, instance_id).join("meta")
}

/// Pinned generation root: `apps/{app}/env/{generation}/` (does **not** follow `env/current`).
///
/// Treat as read-only; never write eval/bootstrap/cache here.
pub fn pinned_generation_root(workspace: &Path, app_id: &str, generation: &str) -> PathBuf {
    resolve_app_root(workspace, app_id)
        .join("env")
        .join(generation.trim())
}

fn sanitize_instance_id(instance_id: &str) -> &str {
    let trimmed = instance_id.trim();
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
    fn instance_path_helpers_nest_under_deploy_runtime_instances() {
        let ws = PathBuf::from("/tmp/ws");
        let root = instance_runtime_root(&ws, "inst-a");
        assert_eq!(
            root,
            PathBuf::from("/tmp/ws/deploy/runtime/instances/inst-a")
        );
        assert_eq!(
            instance_eval_cache_dir(&ws, "inst-a"),
            root.join("var/eval-cache")
        );
        assert_eq!(
            instance_bootstrap_dir(&ws, "inst-a"),
            root.join("var/client-bootstrap")
        );
        assert_eq!(instance_var_dir(&ws, "inst-a"), root.join("var"));
        assert_eq!(
            instance_mrg_memory_dir(&ws, "inst-a"),
            root.join("var/mrg/memory")
        );
        assert_eq!(
            instance_mrg_disk_dir(&ws, "inst-a"),
            root.join("var/mrg/disk")
        );
        assert_eq!(instance_logs_dir(&ws, "inst-a"), root.join("logs"));
        assert_eq!(instance_meta_dir(&ws, "inst-a"), root.join("meta"));
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
    fn distinct_instance_ids_yield_distinct_roots() {
        let ws = PathBuf::from("/tmp/ws");
        assert_ne!(
            instance_runtime_root(&ws, "inst-a"),
            instance_runtime_root(&ws, "inst-b")
        );
    }
}
