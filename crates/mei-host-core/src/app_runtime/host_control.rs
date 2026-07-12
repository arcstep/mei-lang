use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mei_lang_kernel::RuntimePlan;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::LaunchManifest;
use super::LastSuccessfulApply;

pub const SCHEMA_HOST_CONTROL_V1: &str = "mei-host-control-v1";
pub const SCHEMA_HOST_CONTROL_V2: &str = "mei-host-control-v2";

/// Typed envelope for `{workspace}/deploy/state/host-control.json`.
///
/// Writes always use [`SCHEMA_HOST_CONTROL_V2`]. Legacy v1 documents are accepted on read
/// and migrated in-memory into a [`LaunchManifest`] (instances may be empty).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostControlState {
    pub schema_version: String,
    pub launch_manifest: LaunchManifest,
    /// Compatibility field for migration-period readers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<ActiveProfileRef>,
    /// Compatibility mirror of `launch_manifest.last_successful_apply`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_successful_apply: Option<LastSuccessfulApply>,
    /// Compatibility field for migration-period readers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_plan: Option<RuntimePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveProfileRef {
    pub id: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub file: String,
}

#[derive(Debug, Error)]
pub enum HostControlConflict {
    #[error("launch manifest revision conflict: expected {expected}, current {current}")]
    Conflict { expected: String, current: String },
    #[error(transparent)]
    Io(#[from] anyhow::Error),
}

impl PartialEq for HostControlConflict {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Conflict {
                    expected: e1,
                    current: c1,
                },
                Self::Conflict {
                    expected: e2,
                    current: c2,
                },
            ) => e1 == e2 && c1 == c2,
            (Self::Io(a), Self::Io(b)) => a.to_string() == b.to_string(),
            _ => false,
        }
    }
}

impl HostControlState {
    pub fn new(launch_manifest: LaunchManifest) -> Self {
        let last_successful_apply = launch_manifest.last_successful_apply.clone();
        Self {
            schema_version: SCHEMA_HOST_CONTROL_V2.to_string(),
            launch_manifest,
            active_profile: None,
            last_successful_apply,
            runtime_plan: None,
        }
    }

    pub fn empty() -> Self {
        Self::new(LaunchManifest::empty())
    }

    /// Parse raw JSON, migrating legacy `mei-host-control-v1` documents.
    pub fn from_value(value: Value) -> Option<Self> {
        let schema = value
            .get("schemaVersion")
            .and_then(Value::as_str)
            .unwrap_or("");
        if schema == SCHEMA_HOST_CONTROL_V2 || value.get("launchManifest").is_some() {
            return serde_json::from_value(value).ok();
        }
        if schema == SCHEMA_HOST_CONTROL_V1
            || value.get("activeProfile").is_some()
            || value.get("lastSuccessfulApply").is_some()
            || value.get("runtimePlan").is_some()
        {
            return migrate_v1(value);
        }
        serde_json::from_value(value.clone())
            .ok()
            .or_else(|| migrate_v1(value))
    }

    pub fn sync_compat_fields(&mut self) {
        self.schema_version = SCHEMA_HOST_CONTROL_V2.to_string();
        self.last_successful_apply = self.launch_manifest.last_successful_apply.clone();
    }
}

fn migrate_v1(value: Value) -> Option<HostControlState> {
    let active_profile = value
        .get("activeProfile")
        .cloned()
        .and_then(|entry| serde_json::from_value(entry).ok());
    let last_successful_apply = value
        .get("lastSuccessfulApply")
        .cloned()
        .and_then(|entry| serde_json::from_value(entry).ok());
    let runtime_plan = value
        .get("runtimePlan")
        .cloned()
        .and_then(|entry| serde_json::from_value(entry).ok());

    let mut launch_manifest = LaunchManifest::empty();
    launch_manifest.last_successful_apply = last_successful_apply.clone();
    launch_manifest = launch_manifest.with_recomputed_revision();

    Some(HostControlState {
        schema_version: SCHEMA_HOST_CONTROL_V2.to_string(),
        launch_manifest,
        active_profile,
        last_successful_apply,
        runtime_plan,
    })
}

pub fn host_control_path(workspace: &Path) -> PathBuf {
    workspace.join("deploy/state/host-control.json")
}

pub fn read_host_control_state(workspace: &Path) -> Option<HostControlState> {
    let path = host_control_path(workspace);
    let bytes = fs::read(&path).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    HostControlState::from_value(value)
}

pub fn write_host_control_state(workspace: &Path, state: &HostControlState) -> Result<()> {
    let path = host_control_path(workspace);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create host-control dir {}", parent.display()))?;
    }
    let mut to_write = state.clone();
    to_write.sync_compat_fields();
    let bytes = serde_json::to_vec_pretty(&to_write).context("serialize host-control state")?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes).with_context(|| format!("write temp {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Compare-and-swap `LaunchManifest` by expected revision.
pub fn write_if_revision_matches(
    workspace: &Path,
    expected_revision: &str,
    new_manifest: LaunchManifest,
) -> Result<(), HostControlConflict> {
    let current_revision = read_host_control_state(workspace)
        .map(|state| state.launch_manifest.revision)
        .unwrap_or_default();
    if current_revision != expected_revision {
        return Err(HostControlConflict::Conflict {
            expected: expected_revision.to_string(),
            current: current_revision,
        });
    }
    let mut state = read_host_control_state(workspace).unwrap_or_else(HostControlState::empty);
    state.launch_manifest = new_manifest;
    state.sync_compat_fields();
    write_host_control_state(workspace, &state)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_legacy_host_control_v1() {
        let value = json!({
            "schemaVersion": SCHEMA_HOST_CONTROL_V1,
            "activeProfile": {
                "id": "local",
                "revision": "r1",
                "file": "configs/local.json"
            },
            "lastSuccessfulApply": {
                "profileId": "local",
                "profileRevision": "r1",
                "envVersion": "WS-1",
                "appliedAtMs": 99u64,
                "apps": ["mini-data"]
            },
            "runtimePlan": {
                "defaultMode": "hot",
                "apps": {}
            }
        });
        let state = HostControlState::from_value(value).expect("migrate v1");
        assert_eq!(state.schema_version, SCHEMA_HOST_CONTROL_V2);
        assert!(state.launch_manifest.instances.is_empty());
        assert_eq!(
            state
                .launch_manifest
                .last_successful_apply
                .as_ref()
                .map(|apply| apply.profile_id.as_str()),
            Some("local")
        );
        assert_eq!(
            state.active_profile.as_ref().map(|p| p.id.as_str()),
            Some("local")
        );
        assert!(state.runtime_plan.is_some());
        assert!(!state.launch_manifest.revision.is_empty());
    }

    #[test]
    fn write_read_roundtrip_and_cas() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();

        let mut state = HostControlState::empty();
        state.active_profile = Some(ActiveProfileRef {
            id: "local".to_string(),
            revision: "r1".to_string(),
            file: "configs/local.json".to_string(),
        });
        state.launch_manifest.last_successful_apply = Some(LastSuccessfulApply {
            profile_id: "local".to_string(),
            profile_revision: "r1".to_string(),
            env_version: Some("WS-1".to_string()),
            applied_at_ms: 7,
            instance_ids: vec![],
            apps: vec!["mini-data".to_string()],
        });
        state.launch_manifest = state.launch_manifest.clone().with_recomputed_revision();
        state.sync_compat_fields();

        write_host_control_state(workspace, &state).expect("write");
        let loaded = read_host_control_state(workspace).expect("read");
        assert_eq!(loaded.schema_version, SCHEMA_HOST_CONTROL_V2);
        assert_eq!(
            loaded.active_profile.as_ref().map(|p| p.id.as_str()),
            Some("local")
        );
        assert_eq!(
            loaded
                .last_successful_apply
                .as_ref()
                .map(|a| a.apps.clone()),
            Some(vec!["mini-data".to_string()])
        );

        let expected = loaded.launch_manifest.revision.clone();
        let mut next = loaded.launch_manifest.clone();
        next.workspace_root = Some("/tmp/ws".to_string());
        next = next.with_recomputed_revision();
        write_if_revision_matches(workspace, expected.as_str(), next.clone()).expect("cas ok");

        let conflict = write_if_revision_matches(workspace, "stale-revision", next);
        assert!(matches!(
            conflict,
            Err(HostControlConflict::Conflict { .. })
        ));
    }

    #[test]
    fn write_uses_tmp_rename_and_compat_fields() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        let state = HostControlState::empty();
        write_host_control_state(workspace, &state).expect("write");
        let raw: Value =
            serde_json::from_slice(&fs::read(host_control_path(workspace)).expect("read raw"))
                .expect("parse");
        assert_eq!(
            raw.get("schemaVersion").and_then(Value::as_str),
            Some(SCHEMA_HOST_CONTROL_V2)
        );
        assert!(raw.get("launchManifest").is_some());
        assert!(!host_control_path(workspace)
            .with_extension("json.tmp")
            .exists());
    }
}
