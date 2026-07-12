//! Runtime identity carrying pinned generation + instance id (does not follow `env/current`).

use std::path::PathBuf;

use super::cache_partition::CachePartitionKey;
use super::paths::{
    instance_bootstrap_dir, instance_eval_cache_dir, instance_logs_dir, instance_meta_dir,
    instance_mrg_disk_dir, instance_mrg_memory_dir, instance_runtime_root, instance_var_dir,
    pinned_generation_root,
};
use super::InstanceSpec;
use crate::context::HostContext;

/// Future injection point for per-instance cache stores.
///
/// Today process-global maps remain behind `OnceLock`; callers must key them with
/// [`CachePartitionKey`] (via [`AppRuntimeState::partition`]) so dual-instance
/// embeds do not share entries. Replaceable store fields can be added later without
/// changing the partition / path surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRuntimeState {
    pub partition: CachePartitionKey,
    pub instance_id: String,
    pub workspace_root: PathBuf,
}

impl AppRuntimeState {
    pub fn new(
        workspace_root: impl Into<PathBuf>,
        instance_id: impl Into<String>,
        partition: CachePartitionKey,
    ) -> Self {
        Self {
            partition,
            instance_id: instance_id.into(),
            workspace_root: workspace_root.into(),
        }
    }

    pub fn from_instance_spec(
        workspace_root: impl Into<PathBuf>,
        spec: &InstanceSpec,
    ) -> Self {
        Self::new(
            workspace_root,
            spec.instance_id.as_str(),
            CachePartitionKey::from_instance_spec(spec),
        )
    }

    pub fn prefix_cache_key(&self, inner: &str) -> String {
        self.partition.prefix_key(inner)
    }

    pub fn instance_runtime_root(&self) -> PathBuf {
        instance_runtime_root(self.workspace_root.as_path(), self.instance_id.as_str())
    }

    pub fn instance_var_dir(&self) -> PathBuf {
        instance_var_dir(self.workspace_root.as_path(), self.instance_id.as_str())
    }

    pub fn instance_eval_cache_dir(&self) -> PathBuf {
        instance_eval_cache_dir(self.workspace_root.as_path(), self.instance_id.as_str())
    }

    pub fn instance_bootstrap_dir(&self) -> PathBuf {
        instance_bootstrap_dir(self.workspace_root.as_path(), self.instance_id.as_str())
    }

    pub fn instance_mrg_memory_dir(&self) -> PathBuf {
        instance_mrg_memory_dir(self.workspace_root.as_path(), self.instance_id.as_str())
    }

    pub fn instance_mrg_disk_dir(&self) -> PathBuf {
        instance_mrg_disk_dir(self.workspace_root.as_path(), self.instance_id.as_str())
    }

    pub fn instance_logs_dir(&self) -> PathBuf {
        instance_logs_dir(self.workspace_root.as_path(), self.instance_id.as_str())
    }

    pub fn instance_meta_dir(&self) -> PathBuf {
        instance_meta_dir(self.workspace_root.as_path(), self.instance_id.as_str())
    }

    pub fn pinned_generation_root(&self) -> PathBuf {
        pinned_generation_root(
            self.workspace_root.as_path(),
            self.partition.app_id.as_str(),
            self.partition.generation.as_str(),
        )
    }
}

/// Host request context pinned to a concrete App Runtime instance.
///
/// Prefer this over bare [`HostContext`] for runtime data-plane work so paths and
/// caches resolve against `generation` / `instance_id` instead of mutable `env/current`.
#[derive(Debug, Clone)]
pub struct RuntimeContext {
    pub host: HostContext,
    pub instance_id: String,
    pub generation: String,
    pub config_digest: String,
}

impl RuntimeContext {
    pub fn new(
        host: HostContext,
        instance_id: impl Into<String>,
        generation: impl Into<String>,
        config_digest: impl Into<String>,
    ) -> Self {
        Self {
            host,
            instance_id: instance_id.into(),
            generation: generation.into(),
            config_digest: config_digest.into(),
        }
    }

    pub fn from_instance_spec(workspace_root: impl Into<PathBuf>, spec: &InstanceSpec) -> Self {
        let workspace_root = workspace_root.into();
        let host = HostContext::new(workspace_root, spec.app_id.as_str());
        let config_digest = spec
            .bundle
            .config_digest
            .clone()
            .unwrap_or_else(|| spec.spec_digest());
        Self::new(
            host,
            spec.instance_id.as_str(),
            spec.bundle.generation.as_str(),
            config_digest,
        )
    }

    pub fn app_id(&self) -> &str {
        self.host.app_id.as_str()
    }

    pub fn workspace_root(&self) -> &std::path::Path {
        self.host.workspace_root.as_path()
    }

    pub fn partition(&self) -> CachePartitionKey {
        CachePartitionKey::new(
            self.host.app_id.as_str(),
            self.generation.as_str(),
            self.config_digest.as_str(),
        )
    }

    pub fn runtime_state(&self) -> AppRuntimeState {
        AppRuntimeState::new(
            self.host.workspace_root.clone(),
            self.instance_id.as_str(),
            self.partition(),
        )
    }

    pub fn instance_runtime_root(&self) -> PathBuf {
        instance_runtime_root(self.workspace_root(), self.instance_id.as_str())
    }

    pub fn instance_eval_cache_dir(&self) -> PathBuf {
        instance_eval_cache_dir(self.workspace_root(), self.instance_id.as_str())
    }

    pub fn instance_bootstrap_dir(&self) -> PathBuf {
        instance_bootstrap_dir(self.workspace_root(), self.instance_id.as_str())
    }

    pub fn instance_var_dir(&self) -> PathBuf {
        instance_var_dir(self.workspace_root(), self.instance_id.as_str())
    }

    /// Read-only generation tree; does not follow `env/current`.
    pub fn pinned_generation_root(&self) -> PathBuf {
        pinned_generation_root(
            self.workspace_root(),
            self.host.app_id.as_str(),
            self.generation.as_str(),
        )
    }

    /// Pinned registry root under the sealed generation (not `env/current`).
    pub fn pinned_registry_root(&self) -> PathBuf {
        self.pinned_generation_root()
            .join("build")
            .join("registry")
    }
}

impl HostContext {
    /// Attach pinned instance identity for runtime requests.
    pub fn with_runtime(
        &self,
        instance_id: impl Into<String>,
        generation: impl Into<String>,
        config_digest: impl Into<String>,
    ) -> RuntimeContext {
        RuntimeContext::new(self.clone(), instance_id, generation, config_digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_runtime::{BundleRef, ConfigSnapshot, InstanceSpec, SCHEMA_INSTANCE_SPEC_V1};
    use mei_lang_kernel::{RuntimeMode, RuntimePlan};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn sample_spec(instance_id: &str, config_digest: &str) -> InstanceSpec {
        InstanceSpec {
            schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
            instance_id: instance_id.to_string(),
            app_id: "mini-data".to_string(),
            bundle: BundleRef {
                generation: "WS-20260712.1".to_string(),
                bundle_path: "apps/mini-data/env/WS-20260712.1".to_string(),
                digest: None,
                toolchain_version: None,
                config_digest: Some(config_digest.to_string()),
            },
            config_snapshot: ConfigSnapshot {
                profile_id: "local".to_string(),
                profile_revision: "r1".to_string(),
                profile_file: "configs/local.json".to_string(),
                runtime_plan: RuntimePlan {
                    default_mode: RuntimeMode::Hot,
                    apps: BTreeMap::new(),
                },
                default_app: None,
            },
            runtime_abi: "2.4".to_string(),
            data_mode_ceiling: None,
        }
    }

    #[test]
    fn runtime_context_pins_generation_and_instance_paths() {
        let ws = PathBuf::from("/tmp/ws");
        let ctx = RuntimeContext::from_instance_spec(&ws, &sample_spec("inst-a", "cfg-a"));
        assert_eq!(ctx.instance_id, "inst-a");
        assert_eq!(ctx.generation, "WS-20260712.1");
        assert_eq!(
            ctx.pinned_generation_root(),
            PathBuf::from("/tmp/ws/apps/mini-data/env/WS-20260712.1")
        );
        assert_eq!(
            ctx.instance_runtime_root(),
            PathBuf::from("/tmp/ws/deploy/runtime/instances/inst-a")
        );
        assert!(!ctx
            .pinned_generation_root()
            .to_string_lossy()
            .contains("env/current"));
    }

    #[test]
    fn dual_instance_partitions_do_not_share_cache_keys() {
        let ws = PathBuf::from("/tmp/ws");
        let a = RuntimeContext::from_instance_spec(&ws, &sample_spec("inst-a", "cfg-scoped"));
        let b = RuntimeContext::from_instance_spec(&ws, &sample_spec("inst-b", "cfg-full"));
        let inner = r#"{"app_id":"mini-data","scene":"home"}"#;
        assert_ne!(
            a.partition().prefix_key(inner),
            b.partition().prefix_key(inner)
        );
        assert_ne!(a.runtime_state().instance_runtime_root(), b.runtime_state().instance_runtime_root());
    }

    #[test]
    fn host_context_with_runtime_preserves_app_id() {
        let host = HostContext::new("/tmp/ws", "mini-data");
        let rt = host.with_runtime("inst-1", "WS-1", "digest");
        assert_eq!(rt.app_id(), "mini-data");
        assert_eq!(rt.partition().config_digest, "digest");
    }
}
