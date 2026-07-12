//! Process-cache partition key so same-process multi-instance embeds do not share entries.

use serde::{Deserialize, Serialize};

use super::InstanceSpec;

/// Cache shard identity: `(app_id, generation, config_digest)`.
///
/// One-process-per-app runtimes are naturally isolated; tests and future in-process
/// embeds must pass this key (or [`crate::AppRuntimeState`]) so global `OnceLock`
/// maps do not cross-contaminate instances.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachePartitionKey {
    pub app_id: String,
    pub generation: String,
    pub config_digest: String,
}

impl CachePartitionKey {
    pub fn new(
        app_id: impl Into<String>,
        generation: impl Into<String>,
        config_digest: impl Into<String>,
    ) -> Self {
        Self {
            app_id: app_id.into(),
            generation: generation.into(),
            config_digest: config_digest.into(),
        }
    }

    /// Build from an [`InstanceSpec`], using `bundle.config_digest` when present,
    /// otherwise falling back to [`InstanceSpec::spec_digest`].
    pub fn from_instance_spec(spec: &InstanceSpec) -> Self {
        let config_digest = spec
            .bundle
            .config_digest
            .clone()
            .unwrap_or_else(|| spec.spec_digest());
        Self::new(
            spec.app_id.as_str(),
            spec.bundle.generation.as_str(),
            config_digest,
        )
    }

    /// Stable wire prefix shared with `mei-lang-datasets` partition helpers.
    ///
    /// Format: `part:{app_id}/{generation}/{config_digest}|`
    pub fn prefix(&self) -> String {
        format!(
            "part:{}/{}/{}|",
            self.app_id.trim(),
            self.generation.trim(),
            self.config_digest.trim()
        )
    }

    pub fn prefix_key(&self, inner: &str) -> String {
        format!("{}{inner}", self.prefix())
    }

    pub fn matches_key(&self, key: &str) -> bool {
        key.starts_with(self.prefix().as_str())
    }

    /// Strip this partition prefix when present; otherwise return `key` unchanged.
    pub fn strip_prefix<'a>(&self, key: &'a str) -> &'a str {
        let prefix = self.prefix();
        key.strip_prefix(prefix.as_str()).unwrap_or(key)
    }
}

/// Apply the shared partition wire format without constructing a full key struct.
pub fn partition_cache_key(
    app_id: &str,
    generation: &str,
    config_digest: &str,
    inner: &str,
) -> String {
    CachePartitionKey::new(app_id, generation, config_digest).prefix_key(inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_runtime::{BundleRef, ConfigSnapshot, InstanceSpec, SCHEMA_INSTANCE_SPEC_V1};
    use mei_lang_kernel::{RuntimeMode, RuntimePlan};
    use std::collections::BTreeMap;

    fn sample_spec(instance_id: &str, config_digest: &str) -> InstanceSpec {
        InstanceSpec {
            schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
            instance_id: instance_id.to_string(),
            app_id: "mini-data".to_string(),
            bundle: BundleRef {
                generation: "WS-20260712.1".to_string(),
                bundle_path: "apps/mini-data/env/WS-20260712.1".to_string(),
                digest: Some("bundle-digest".to_string()),
                toolchain_version: Some("1.0.0".to_string()),
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
                default_app: Some("mini-data".to_string()),
                ..Default::default()
            },
            runtime_abi: "2.4".to_string(),
            data_mode_ceiling: None,
        }
    }

    #[test]
    fn distinct_config_digests_yield_distinct_prefixes() {
        let a = CachePartitionKey::new("mini-data", "WS-1", "cfg-a");
        let b = CachePartitionKey::new("mini-data", "WS-1", "cfg-b");
        assert_ne!(a.prefix(), b.prefix());
        assert_ne!(a.prefix_key("inner"), b.prefix_key("inner"));
        assert!(a.matches_key(&a.prefix_key("inner")));
        assert!(!a.matches_key(&b.prefix_key("inner")));
    }

    #[test]
    fn from_instance_spec_uses_bundle_config_digest() {
        let spec = sample_spec("inst-1", "cfg-scoped");
        let key = CachePartitionKey::from_instance_spec(&spec);
        assert_eq!(key.app_id, "mini-data");
        assert_eq!(key.generation, "WS-20260712.1");
        assert_eq!(key.config_digest, "cfg-scoped");
        assert_eq!(
            key.prefix_key("layer"),
            "part:mini-data/WS-20260712.1/cfg-scoped|layer"
        );
    }

    #[test]
    fn partition_cache_key_helper_matches_struct() {
        let via_fn = partition_cache_key("app", "gen", "digest", "k");
        let via_struct = CachePartitionKey::new("app", "gen", "digest").prefix_key("k");
        assert_eq!(via_fn, via_struct);
    }
}
