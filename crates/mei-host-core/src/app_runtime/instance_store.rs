//! Persist / load immutable [`InstanceSpec`] under app ephemeral runtime dirs.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::paths::{instance_runtime_root, legacy_instance_runtime_root};
use super::InstanceSpec;

/// `{app_ephemeral_root}/spec.json`
pub fn instance_spec_path(workspace: &Path, app_id: &str) -> PathBuf {
    instance_runtime_root(workspace, app_id).join("spec.json")
}

pub fn write_instance_spec(workspace: &Path, spec: &InstanceSpec) -> Result<()> {
    let path = instance_spec_path(workspace, spec.app_id.as_str());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create app runtime dir {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(spec).context("serialize InstanceSpec")?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes).with_context(|| format!("write temp {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

/// Read spec for an app from the ephemeral app root (preferred).
pub fn read_instance_spec_for_app(workspace: &Path, app_id: &str) -> Option<InstanceSpec> {
    let path = instance_spec_path(workspace, app_id);
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Resolve by instance id: prefer app roots that contain a matching spec, then legacy
/// `deploy/runtime/instances/{id}/spec.json`.
pub fn read_instance_spec(workspace: &Path, instance_id: &str) -> Option<InstanceSpec> {
    if let Some(spec) = scan_app_runtime_specs(workspace)
        .into_iter()
        .find(|spec| spec.instance_id == instance_id)
    {
        return Some(spec);
    }
    let legacy = legacy_instance_runtime_root(workspace, instance_id).join("spec.json");
    let bytes = fs::read(legacy).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// List instance ids from app ephemeral dirs plus legacy instance dirs.
pub fn list_instance_runtime_ids(workspace: &Path) -> Vec<String> {
    let mut ids = scan_app_runtime_specs(workspace)
        .into_iter()
        .map(|spec| spec.instance_id)
        .collect::<Vec<_>>();
    let legacy_root = workspace.join("deploy/runtime/instances");
    if let Ok(entries) = fs::read_dir(legacy_root) {
        for entry in entries.flatten() {
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                if !ids.contains(&name) {
                    ids.push(name);
                }
            }
        }
    }
    ids.sort();
    ids
}

fn scan_app_runtime_specs(workspace: &Path) -> Vec<InstanceSpec> {
    let root = workspace.join("deploy/runtime/apps");
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut specs = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = entry.path().join("spec.json");
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        if let Ok(spec) = serde_json::from_slice::<InstanceSpec>(&bytes) {
            specs.push(spec);
        }
    }
    specs
}

/// Remove ephemeral runtime dir for an app. Never touches durable `apps/{app}/`.
pub fn clear_app_ephemeral_runtime(workspace: &Path, app_id: &str) -> Result<()> {
    let path = instance_runtime_root(workspace, app_id);
    if path.exists() {
        fs::remove_dir_all(&path).with_context(|| format!("clear ephemeral {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BundleRef, ConfigSnapshot, SCHEMA_INSTANCE_SPEC_V1};
    use mei_lang_kernel::{RuntimeMode, RuntimePlan};
    use std::collections::BTreeMap;

    fn sample_spec(id: &str, app_id: &str) -> InstanceSpec {
        InstanceSpec {
            schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
            instance_id: id.to_string(),
            app_id: app_id.to_string(),
            bundle: BundleRef {
                generation: "WS-1".to_string(),
                bundle_path: format!("apps/{app_id}/env/WS-1"),
                digest: None,
                toolchain_version: None,
                config_digest: None,
            },
            config_snapshot: ConfigSnapshot {
                profile_id: "local".to_string(),
                profile_revision: "r1".to_string(),
                profile_file: "configs/local.json".to_string(),
                runtime_plan: RuntimePlan {
                    default_mode: RuntimeMode::Lazy,
                    apps: BTreeMap::new(),
                },
                default_app: None,
                ..Default::default()
            },
            runtime_abi: "0.0.0".to_string(),
            data_mode_ceiling: None,
        }
    }

    #[test]
    fn write_and_read_roundtrip_under_app_root() {
        let tmp = tempfile::tempdir().expect("temp");
        let spec = sample_spec("inst-a", "mini-data");
        write_instance_spec(tmp.path(), &spec).expect("write");
        let path = instance_spec_path(tmp.path(), "mini-data");
        assert!(path.exists());
        assert!(path
            .to_string_lossy()
            .contains("deploy/runtime/apps/mini-data/spec.json"));
        let back = read_instance_spec_for_app(tmp.path(), "mini-data").expect("read app");
        assert_eq!(back.instance_id, "inst-a");
        let by_id = read_instance_spec(tmp.path(), "inst-a").expect("read id");
        assert_eq!(by_id.app_id, "mini-data");
    }

    #[test]
    fn read_falls_back_to_legacy_instances_dir() {
        let tmp = tempfile::tempdir().expect("temp");
        let spec = sample_spec("inst-legacy", "mini-data");
        let legacy = legacy_instance_runtime_root(tmp.path(), "inst-legacy");
        fs::create_dir_all(&legacy).expect("mkdir");
        fs::write(
            legacy.join("spec.json"),
            serde_json::to_vec_pretty(&spec).expect("ser"),
        )
        .expect("write");
        let back = read_instance_spec(tmp.path(), "inst-legacy").expect("legacy");
        assert_eq!(back.instance_id, "inst-legacy");
    }
}
