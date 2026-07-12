//! Persist / load immutable [`InstanceSpec`] under instance-private runtime dirs.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::paths::instance_runtime_root;
use super::InstanceSpec;

/// `{instance_root}/spec.json`
pub fn instance_spec_path(workspace: &Path, instance_id: &str) -> PathBuf {
    instance_runtime_root(workspace, instance_id).join("spec.json")
}

pub fn write_instance_spec(workspace: &Path, spec: &InstanceSpec) -> Result<()> {
    let path = instance_spec_path(workspace, spec.instance_id.as_str());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create instance dir {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(spec).context("serialize InstanceSpec")?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes).with_context(|| format!("write temp {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
    Ok(())
}

pub fn read_instance_spec(workspace: &Path, instance_id: &str) -> Option<InstanceSpec> {
    let path = instance_spec_path(workspace, instance_id);
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// List instance ids that have a private runtime directory under deploy/runtime/instances.
pub fn list_instance_runtime_ids(workspace: &Path) -> Vec<String> {
    let root = workspace.join("deploy/runtime/instances");
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut ids = entries
        .flatten()
        .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BundleRef, ConfigSnapshot, SCHEMA_INSTANCE_SPEC_V1};
    use mei_lang_kernel::{RuntimeMode, RuntimePlan};
    use std::collections::BTreeMap;

    fn sample_spec(id: &str) -> InstanceSpec {
        InstanceSpec {
            schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
            instance_id: id.to_string(),
            app_id: "mini-data".to_string(),
            bundle: BundleRef {
                generation: "WS-1".to_string(),
                bundle_path: "apps/mini-data/env/WS-1".to_string(),
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
            },
            runtime_abi: "1".to_string(),
            data_mode_ceiling: None,
        }
    }

    #[test]
    fn write_read_instance_spec_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let spec = sample_spec("inst-a");
        write_instance_spec(tmp.path(), &spec).expect("write");
        let back = read_instance_spec(tmp.path(), "inst-a").expect("read");
        assert_eq!(back, spec);
        assert_eq!(list_instance_runtime_ids(tmp.path()), vec!["inst-a".to_string()]);
    }
}
