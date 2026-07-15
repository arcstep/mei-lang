//! Persist / load immutable [`InstanceSpec`] under app ephemeral runtime dirs.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::paths::{
    app_ephemeral_runtime_root, instance_runtime_root, legacy_instance_runtime_root,
};
use super::InstanceSpec;

/// Immutable spec for one instance:
/// `{app_ephemeral_root}/instances/{instance_id}/spec.json`.
pub fn instance_spec_path(workspace: &Path, app_id: &str, instance_id: &str) -> PathBuf {
    instance_runtime_root(workspace, app_id, instance_id).join("spec.json")
}

/// Compatibility pointer for the currently active instance:
/// `{app_ephemeral_root}/spec.json`.
pub fn active_instance_spec_path(workspace: &Path, app_id: &str) -> PathBuf {
    app_ephemeral_runtime_root(workspace, app_id).join("spec.json")
}

pub fn write_instance_spec(workspace: &Path, spec: &InstanceSpec) -> Result<()> {
    let path = instance_spec_path(workspace, spec.app_id.as_str(), spec.instance_id.as_str());
    write_spec_atomically(path.as_path(), spec)
}

/// Publish the active compatibility pointer after route cutover.
///
/// Candidate startup must only call [`write_instance_spec`]; otherwise a warming
/// candidate can replace the active app's spec before it receives traffic.
pub fn publish_active_instance_spec(workspace: &Path, spec: &InstanceSpec) -> Result<()> {
    write_instance_spec(workspace, spec)?;
    write_spec_atomically(
        active_instance_spec_path(workspace, spec.app_id.as_str()).as_path(),
        spec,
    )
}

fn write_spec_atomically(path: &Path, spec: &InstanceSpec) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create runtime dir {}", parent.display()))?;
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
    let path = active_instance_spec_path(workspace, app_id);
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Resolve by instance id: prefer instance-scoped specs, then legacy app/global roots.
pub fn read_instance_spec(workspace: &Path, instance_id: &str) -> Option<InstanceSpec> {
    if let Some(spec) = scan_instance_runtime_specs(workspace)
        .into_iter()
        .find(|spec| spec.instance_id == instance_id)
    {
        return Some(spec);
    }
    if let Some(spec) = scan_legacy_app_runtime_specs(workspace)
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
    let mut ids = scan_instance_runtime_specs(workspace)
        .into_iter()
        .map(|spec| spec.instance_id)
        .collect::<Vec<_>>();
    for id in scan_legacy_app_runtime_specs(workspace)
        .into_iter()
        .map(|spec| spec.instance_id)
    {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
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

fn scan_instance_runtime_specs(workspace: &Path) -> Vec<InstanceSpec> {
    let root = workspace.join("deploy/runtime/apps");
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut specs = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let instances_root = entry.path().join("instances");
        let Ok(instances) = fs::read_dir(instances_root) else {
            continue;
        };
        for instance in instances.flatten() {
            if !instance
                .file_type()
                .map(|kind| kind.is_dir())
                .unwrap_or(false)
            {
                continue;
            }
            let path = instance.path().join("spec.json");
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            if let Ok(spec) = serde_json::from_slice::<InstanceSpec>(&bytes) {
                specs.push(spec);
            }
        }
    }
    specs
}

fn scan_legacy_app_runtime_specs(workspace: &Path) -> Vec<InstanceSpec> {
    let root = workspace.join("deploy/runtime/apps");
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| fs::read(entry.path().join("spec.json")).ok())
        .filter_map(|bytes| serde_json::from_slice::<InstanceSpec>(&bytes).ok())
        .collect()
}

/// Remove ephemeral runtime dir for an app. Never touches durable `apps/{app}/`.
pub fn clear_app_ephemeral_runtime(workspace: &Path, app_id: &str) -> Result<()> {
    let path = app_ephemeral_runtime_root(workspace, app_id);
    if path.exists() {
        fs::remove_dir_all(&path).with_context(|| format!("clear ephemeral {}", path.display()))?;
    }
    Ok(())
}

/// Remove mutable state for one retired instance without touching active/candidate siblings.
pub fn clear_instance_ephemeral_runtime(
    workspace: &Path,
    app_id: &str,
    instance_id: &str,
) -> Result<()> {
    let path = instance_runtime_root(workspace, app_id, instance_id);
    if path.exists() {
        fs::remove_dir_all(&path)
            .with_context(|| format!("clear instance ephemeral {}", path.display()))?;
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
    fn candidate_spec_does_not_replace_active_pointer() {
        let tmp = tempfile::tempdir().expect("temp");
        let active = sample_spec("inst-a", "mini-data");
        publish_active_instance_spec(tmp.path(), &active).expect("publish active");
        let candidate = sample_spec("inst-b", "mini-data");
        write_instance_spec(tmp.path(), &candidate).expect("write candidate");
        let path = instance_spec_path(tmp.path(), "mini-data", "inst-b");
        assert!(path.exists());
        assert!(path
            .to_string_lossy()
            .contains("deploy/runtime/apps/mini-data/instances/inst-b/spec.json"));
        let back = read_instance_spec_for_app(tmp.path(), "mini-data").expect("read app");
        assert_eq!(back.instance_id, "inst-a");
        let by_id = read_instance_spec(tmp.path(), "inst-b").expect("read id");
        assert_eq!(by_id.app_id, "mini-data");
    }

    #[test]
    fn clearing_retired_instance_preserves_active_sibling() {
        let tmp = tempfile::tempdir().expect("temp");
        let active = sample_spec("inst-a", "mini-data");
        let retired = sample_spec("inst-b", "mini-data");
        publish_active_instance_spec(tmp.path(), &active).expect("active");
        write_instance_spec(tmp.path(), &retired).expect("retired");
        clear_instance_ephemeral_runtime(tmp.path(), "mini-data", "inst-b").expect("clear");
        assert!(instance_spec_path(tmp.path(), "mini-data", "inst-a").exists());
        assert!(!instance_spec_path(tmp.path(), "mini-data", "inst-b").exists());
        assert_eq!(
            read_instance_spec_for_app(tmp.path(), "mini-data")
                .expect("active pointer")
                .instance_id,
            "inst-a"
        );
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
