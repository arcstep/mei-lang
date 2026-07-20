//! AOT Admin registry artifact (`admin-registry-v1`) under `build/registry/`.
//!
//! Design: docs/mei-lang-v2/05-host/0514 / 0545 — request path loads this file;
//! discover + enrich run at prebuild, not on every Admin HTML/API hit.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::admin_discover::AdminRegistryProjection;
use super::build_store::resolve_app_build_generation_from_current;
use super::io::write_string_atomically;
use super::workspace_paths::{resolve_app_registry_root, resolve_app_root};

pub const ADMIN_REGISTRY_ARTIFACT_FILENAME: &str = "admin-registry.json";
pub const ADMIN_REGISTRY_SCHEMA_VERSION: &str = "admin-registry-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRegistryArtifact {
    pub schema_version: String,
    pub app_id: String,
    pub build_generation: String,
    pub admin_registry_digest: String,
    pub page_structure_digest: String,
    pub projection: AdminRegistryProjection,
}

pub fn admin_registry_artifact_path(app_root: &Path) -> PathBuf {
    resolve_app_registry_root(app_root).join(ADMIN_REGISTRY_ARTIFACT_FILENAME)
}

pub fn admin_registry_artifact_path_for_app(workspace_root: &Path, app_id: &str) -> PathBuf {
    admin_registry_artifact_path(resolve_app_root(workspace_root, app_id).as_path())
}

/// Load AOT projection when the artifact exists and parses.
pub fn load_admin_registry_artifact(app_root: &Path) -> Result<Option<AdminRegistryProjection>> {
    let path = admin_registry_artifact_path(app_root);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read admin registry artifact {}", path.display()))?;
    let artifact: AdminRegistryArtifact = serde_json::from_str(&raw)
        .with_context(|| format!("parse admin registry artifact {}", path.display()))?;
    if artifact.schema_version != ADMIN_REGISTRY_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported admin registry schema `{}` (expected `{ADMIN_REGISTRY_SCHEMA_VERSION}`)",
            artifact.schema_version
        );
    }
    Ok(Some(artifact.projection))
}

pub fn load_admin_registry_artifact_for_app(
    workspace_root: &Path,
    app_id: &str,
) -> Result<Option<AdminRegistryProjection>> {
    load_admin_registry_artifact(resolve_app_root(workspace_root, app_id).as_path())
}

/// Persist a fully enriched projection for Host request-path loads.
pub fn write_admin_registry_artifact(
    app_root: &Path,
    projection: &AdminRegistryProjection,
) -> Result<PathBuf> {
    let path = admin_registry_artifact_path(app_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create admin registry dir {}", parent.display()))?;
    }
    let build_generation = resolve_app_build_generation_from_current(app_root)
        .unwrap_or_else(|_| "current".to_string());
    let artifact = AdminRegistryArtifact {
        schema_version: ADMIN_REGISTRY_SCHEMA_VERSION.to_string(),
        app_id: projection.app_id.clone(),
        build_generation,
        admin_registry_digest: projection.admin_registry_digest.clone(),
        page_structure_digest: projection.page_structure_digest.clone(),
        projection: projection.clone(),
    };
    let raw = serde_json::to_string_pretty(&artifact)
        .context("serialize admin registry artifact")?;
    write_string_atomically(&path, raw.as_str())
        .with_context(|| format!("write admin registry artifact {}", path.display()))?;
    Ok(path)
}

pub fn write_admin_registry_artifact_for_app(
    workspace_root: &Path,
    app_id: &str,
    projection: &AdminRegistryProjection,
) -> Result<PathBuf> {
    write_admin_registry_artifact(resolve_app_root(workspace_root, app_id).as_path(), projection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mei_config::admin_discover::AdminEntryProjection;
    use crate::mei_config::admin_registry::{
        AdminArtifactRefs, AdminDangerLevel, AdminRegistryEntry, ADMIN_RESOURCE_API_VERSION,
    };
    use crate::model::PageProgram;
    use tempfile::tempdir;

    fn sample_projection(app_id: &str) -> AdminRegistryProjection {
        AdminRegistryProjection {
            app_id: app_id.to_string(),
            api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
            admin_registry_digest: "sha256:reg".to_string(),
            page_structure_digest: "sha256:page".to_string(),
            resources: vec![AdminEntryProjection {
                registry_entry: AdminRegistryEntry {
                    api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
                    app_id: app_id.to_string(),
                    resource_id: "theme".to_string(),
                    module_id: "cockpit".to_string(),
                    resource_key: format!("app:{app_id}.theme.cockpit"),
                    canonical_route: format!("/admin/apps/{app_id}/theme/cockpit"),
                    title: "外观".to_string(),
                    short_title: None,
                    description: None,
                    navigation: None,
                    required_capabilities: vec!["config_upload".to_string()],
                    scope: "app".to_string(),
                    audit: true,
                    danger_level: AdminDangerLevel::Normal,
                    source_anchor: "src/admin/theme/cockpit.mdx".to_string(),
                },
                page_program: PageProgram::from_scene_ref(
                    "admin.theme.cockpit",
                    Some("外观".to_string()),
                    "src/admin/theme/cockpit.mdx",
                    "admin.theme.cockpit",
                ),
                page_structure_digest: "sha256:entry".to_string(),
                artifact_refs: AdminArtifactRefs::default(),
            }],
        }
    }

    #[test]
    fn admin_registry_artifact_roundtrip() {
        let dir = tempdir().unwrap();
        let app_root = dir.path().join("apps/demo");
        let env_ver = app_root.join("env/WS-20260720.0");
        fs::create_dir_all(env_ver.join("build/registry")).unwrap();
        std::os::unix::fs::symlink("WS-20260720.0", app_root.join("env/current")).unwrap();
        let projection = sample_projection("demo");
        let path = write_admin_registry_artifact(&app_root, &projection).unwrap();
        assert!(path.ends_with(ADMIN_REGISTRY_ARTIFACT_FILENAME));
        let loaded = load_admin_registry_artifact(&app_root)
            .unwrap()
            .expect("artifact");
        assert_eq!(loaded.app_id, "demo");
        assert_eq!(loaded.admin_registry_digest, "sha256:reg");
        assert_eq!(loaded.resources.len(), 1);
        assert_eq!(loaded.resources[0].registry_entry.module_id, "cockpit");
    }
}
