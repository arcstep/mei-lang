use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::sha256_hex;

pub const SCHEMA_LAUNCH_MANIFEST_V1: &str = "mei-launch-manifest-v1";

/// Host-persisted desired topology: instances, routes, and last successful apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LaunchManifest {
    pub schema_version: String,
    /// Content hash of the stable manifest fields (excluding this revision).
    pub revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub instances: BTreeMap<String, DesiredInstance>,
    #[serde(default)]
    pub routes: BTreeMap<String, RouteBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_successful_apply: Option<LastSuccessfulApply>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredInstance {
    pub spec_ref: String,
    pub desired_state: DesiredState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesiredState {
    Running,
    Standby,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LastSuccessfulApply {
    pub profile_id: String,
    /// May be empty when migrating partial legacy documents.
    #[serde(default)]
    pub profile_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_version: Option<String>,
    #[serde(default)]
    pub applied_at_ms: u64,
    #[serde(default)]
    pub instance_ids: Vec<String>,
    #[serde(default)]
    pub apps: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchManifestRevisionPayload<'a> {
    schema_version: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace_root: &'a Option<String>,
    instances: &'a BTreeMap<String, DesiredInstance>,
    routes: &'a BTreeMap<String, RouteBinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_successful_apply: &'a Option<LastSuccessfulApply>,
}

impl LaunchManifest {
    pub fn empty() -> Self {
        let mut manifest = Self {
            schema_version: SCHEMA_LAUNCH_MANIFEST_V1.to_string(),
            revision: String::new(),
            workspace_root: None,
            instances: BTreeMap::new(),
            routes: BTreeMap::new(),
            last_successful_apply: None,
        };
        manifest.revision = manifest.compute_revision();
        manifest
    }

    /// SHA-256 hex over canonical JSON of stable fields (excludes `revision`).
    pub fn compute_revision(&self) -> String {
        let payload = LaunchManifestRevisionPayload {
            schema_version: self.schema_version.as_str(),
            workspace_root: &self.workspace_root,
            instances: &self.instances,
            routes: &self.routes,
            last_successful_apply: &self.last_successful_apply,
        };
        let bytes =
            serde_json::to_vec(&payload).expect("LaunchManifest revision payload serializes");
        sha256_hex(&bytes)
    }

    pub fn with_recomputed_revision(mut self) -> Self {
        self.revision = self.compute_revision();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn launch_manifest_roundtrip() {
        let mut manifest = LaunchManifest::empty();
        manifest.instances.insert(
            "inst-1".to_string(),
            DesiredInstance {
                spec_ref: "sha256:abc".to_string(),
                desired_state: DesiredState::Running,
            },
        );
        manifest.routes.insert(
            "mini-data".to_string(),
            RouteBinding {
                active: Some("inst-1".to_string()),
                candidate: None,
                previous: None,
            },
        );
        manifest = manifest.with_recomputed_revision();

        let value = serde_json::to_value(&manifest).expect("serialize");
        let back: LaunchManifest = serde_json::from_value(value).expect("deserialize");
        assert_eq!(back, manifest);
    }

    #[test]
    fn launch_manifest_denies_unknown_fields() {
        let err = serde_json::from_value::<LaunchManifest>(json!({
            "schemaVersion": SCHEMA_LAUNCH_MANIFEST_V1,
            "revision": "r0",
            "instances": {},
            "routes": {},
            "extra": 1
        }));
        assert!(err.is_err(), "unknown fields must be rejected");
    }

    #[test]
    fn last_successful_apply_accepts_legacy_without_instance_ids() {
        let apply: LastSuccessfulApply = serde_json::from_value(json!({
            "profileId": "local",
            "profileRevision": "r1",
            "envVersion": "WS-1",
            "appliedAtMs": 123u64,
            "apps": ["mini-data"]
        }))
        .expect("legacy apply");
        assert!(apply.instance_ids.is_empty());
        assert_eq!(apply.apps, vec!["mini-data".to_string()]);
    }
}
