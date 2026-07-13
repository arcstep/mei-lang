use mei_lang_kernel::RuntimePlan;
use serde::{Deserialize, Serialize};

use super::sha256_hex;

pub const SCHEMA_INSTANCE_SPEC_V1: &str = "mei-instance-spec-v1";

/// Immutable, content-addressed description of an App Runtime instance.
///
/// `instance_id` identifies a concrete process launch and is excluded from
/// [`InstanceSpec::spec_digest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceSpec {
    pub schema_version: String,
    pub instance_id: String,
    pub app_id: String,
    pub bundle: BundleRef,
    pub config_snapshot: ConfigSnapshot,
    pub runtime_abi: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_mode_ceiling: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleRef {
    pub generation: String,
    pub bundle_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toolchain_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigSnapshot {
    pub profile_id: String,
    pub profile_revision: String,
    pub profile_file: String,
    pub runtime_plan: RuntimePlan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_config_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_config_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_config_file: Option<String>,
    /// Optional copy of launch `warmup` block for App Runtime consumers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warmup: Option<serde_json::Value>,
}

impl Default for ConfigSnapshot {
    fn default() -> Self {
        Self {
            profile_id: String::new(),
            profile_revision: String::new(),
            profile_file: String::new(),
            runtime_plan: RuntimePlan {
                default_mode: mei_lang_kernel::RuntimeMode::Lazy,
                apps: Default::default(),
            },
            default_app: None,
            launch_config_id: None,
            launch_config_revision: None,
            launch_config_file: None,
            warmup: None,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceSpecDigestPayload<'a> {
    schema_version: &'a str,
    app_id: &'a str,
    bundle: &'a BundleRef,
    config_snapshot: &'a ConfigSnapshot,
    runtime_abi: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    data_mode_ceiling: &'a Option<String>,
}

impl InstanceSpec {
    pub fn new(
        instance_id: impl Into<String>,
        app_id: impl Into<String>,
        bundle: BundleRef,
        config_snapshot: ConfigSnapshot,
        runtime_abi: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
            instance_id: instance_id.into(),
            app_id: app_id.into(),
            bundle,
            config_snapshot,
            runtime_abi: runtime_abi.into(),
            data_mode_ceiling: None,
        }
    }

    /// SHA-256 hex of canonical JSON over all stable fields except `instance_id`.
    pub fn spec_digest(&self) -> String {
        let payload = InstanceSpecDigestPayload {
            schema_version: self.schema_version.as_str(),
            app_id: self.app_id.as_str(),
            bundle: &self.bundle,
            config_snapshot: &self.config_snapshot,
            runtime_abi: self.runtime_abi.as_str(),
            data_mode_ceiling: &self.data_mode_ceiling,
        };
        let bytes = serde_json::to_vec(&payload).expect("InstanceSpec digest payload serializes");
        sha256_hex(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::RuntimeMode;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn sample_spec(instance_id: &str) -> InstanceSpec {
        InstanceSpec {
            schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
            instance_id: instance_id.to_string(),
            app_id: "mini-data".to_string(),
            bundle: BundleRef {
                generation: "WS-20260712.1".to_string(),
                bundle_path: "apps/mini-data/env/WS-20260712.1".to_string(),
                digest: Some("bundle-digest".to_string()),
                toolchain_version: Some("1.0.0".to_string()),
                config_digest: Some("cfg-digest".to_string()),
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
            data_mode_ceiling: Some("scoped".to_string()),
        }
    }

    #[test]
    fn instance_spec_roundtrip() {
        let spec = sample_spec("inst-1");
        let value = serde_json::to_value(&spec).expect("serialize");
        let back: InstanceSpec = serde_json::from_value(value).expect("deserialize");
        assert_eq!(back, spec);
    }

    #[test]
    fn instance_spec_denies_unknown_fields() {
        let err = serde_json::from_value::<InstanceSpec>(json!({
            "schemaVersion": SCHEMA_INSTANCE_SPEC_V1,
            "instanceId": "inst-1",
            "appId": "mini-data",
            "bundle": {
                "generation": "g1",
                "bundlePath": "path"
            },
            "configSnapshot": {
                "profileId": "local",
                "profileRevision": "r1",
                "profileFile": "configs/local.json",
                "runtimePlan": { "defaultMode": "hot", "apps": {} }
            },
            "runtimeAbi": "2.4",
            "unexpected": true
        }));
        assert!(err.is_err(), "unknown fields must be rejected");
    }

    #[test]
    fn spec_digest_is_stable_and_ignores_instance_id() {
        let a = sample_spec("inst-a");
        let b = sample_spec("inst-b");
        let digest_a = a.spec_digest();
        let digest_b = b.spec_digest();
        assert_eq!(digest_a, digest_b);
        assert_eq!(digest_a.len(), 64);
        assert!(digest_a.chars().all(|c| c.is_ascii_hexdigit()));

        let mut c = sample_spec("inst-a");
        c.runtime_abi = "2.5".to_string();
        assert_ne!(a.spec_digest(), c.spec_digest());
    }
}
