//! In-memory Admin Resource Registry (0547 + 0548 Host builtins).
//!
//! Request path prefers AOT `admin-registry.json` (0514/0545); discover+enrich
//! only runs at prebuild materialize or as a one-shot fallback when missing.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use mei_lang_kernel::{
    discover_app_admin_resources, filter_admin_resources_for_capabilities,
    load_admin_registry_artifact, resolve_app_root, write_admin_registry_artifact,
    AdminArtifactRef, AdminDiscoverOutcome, AdminDiscoveryDiagnostic, AdminEntryProjection,
    AdminRegistryProjection, DataMode, WorkspaceAppMeta,
};
use sha2::{Digest, Sha256};

use crate::host_builtin::merge_host_builtins;

#[derive(Debug, Default)]
pub struct AdminRegistry {
    by_app: RwLock<HashMap<String, AdminRegistryProjection>>,
    /// Digest of the projection currently held in memory (skip disk re-read).
    loaded_digests: RwLock<HashMap<String, String>>,
    diagnostics: RwLock<Vec<AdminDiscoveryDiagnostic>>,
    /// Apps that already fell back to live discover this process (avoid repeat cost).
    fallback_done: RwLock<HashMap<String, bool>>,
}

pub type SharedAdminRegistry = Arc<AdminRegistry>;

impl AdminRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared() -> SharedAdminRegistry {
        Arc::new(Self::new())
    }

    /// Prefer AOT artifacts for every app; fallback discover only once per app when missing.
    pub fn ensure_workspace_loaded(&self, workspace_root: &Path, apps: &[WorkspaceAppMeta]) {
        for app in apps {
            self.ensure_app_loaded(workspace_root, app.id.as_str());
        }
    }

    /// Load one app from `admin-registry.json` or one-shot discover+enrich fallback.
    pub fn ensure_app_loaded(&self, workspace_root: &Path, app_id: &str) {
        let app_id = app_id.trim();
        if app_id.is_empty() {
            return;
        }
        if self
            .by_app
            .read()
            .expect("admin registry")
            .contains_key(app_id)
        {
            return;
        }
        let app_root = resolve_app_root(workspace_root, app_id);
        match load_admin_registry_artifact(app_root.as_path()) {
            Ok(Some(proj)) => {
                let digest = proj.admin_registry_digest.clone();
                self.insert_projection(app_id, merge_host_builtins(app_id, Some(proj)), digest);
                return;
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    target: "mei.admin_registry",
                    app_id = %app_id,
                    error = %error,
                    "failed to load admin-registry.json; falling back to discover"
                );
            }
        }

        let already = self
            .fallback_done
            .read()
            .expect("admin registry fallback")
            .get(app_id)
            .copied()
            .unwrap_or(false);
        if already {
            // Keep empty / previous builtins-only entry if any.
            if !self
                .by_app
                .read()
                .expect("admin registry")
                .contains_key(app_id)
            {
                self.insert_projection(app_id, merge_host_builtins(app_id, None), String::new());
            }
            return;
        }

        tracing::warn!(
            target: "mei.admin_registry",
            app_id = %app_id,
            "admin-registry.json missing; one-shot discover+enrich fallback (run prebuild)"
        );
        self.fallback_done
            .write()
            .expect("admin registry fallback")
            .insert(app_id.to_string(), true);

        let manifest = match discover_app_admin_resources(app_root.as_path(), app_id) {
            AdminDiscoverOutcome::None => None,
            AdminDiscoverOutcome::Ok(mut proj) => {
                enrich_projection_artifacts(workspace_root, &mut proj);
                Some(proj)
            }
            AdminDiscoverOutcome::Err(diag) => {
                let mut diags = self.diagnostics.write().expect("admin registry diags");
                diags.retain(|d| d.app_id != app_id);
                diags.push(diag);
                None
            }
        };
        let digest = manifest
            .as_ref()
            .map(|p| p.admin_registry_digest.clone())
            .unwrap_or_default();
        self.insert_projection(app_id, merge_host_builtins(app_id, manifest), digest);
    }

    /// Force rebuild from sources (tests / explicit refresh). Prefer `ensure_*` in request path.
    pub fn refresh_workspace(&self, workspace_root: &Path, apps: &[WorkspaceAppMeta]) {
        let mut next = HashMap::new();
        let mut digests = HashMap::new();
        let mut diags = Vec::new();
        for app in apps {
            let app_root = resolve_app_root(workspace_root, app.id.as_str());
            let manifest = match discover_app_admin_resources(&app_root, app.id.as_str()) {
                AdminDiscoverOutcome::None => None,
                AdminDiscoverOutcome::Ok(mut proj) => {
                    enrich_projection_artifacts(workspace_root, &mut proj);
                    Some(proj)
                }
                AdminDiscoverOutcome::Err(diag) => {
                    diags.push(diag);
                    None
                }
            };
            let digest = manifest
                .as_ref()
                .map(|p| p.admin_registry_digest.clone())
                .unwrap_or_default();
            digests.insert(app.id.clone(), digest);
            next.insert(
                app.id.clone(),
                merge_host_builtins(app.id.as_str(), manifest),
            );
        }
        *self.by_app.write().expect("admin registry") = next;
        *self.loaded_digests.write().expect("admin registry digests") = digests;
        *self.diagnostics.write().expect("admin registry diags") = diags;
    }

    pub fn refresh_app(&self, workspace_root: &Path, app_id: &str) {
        // Request-path callers should use ensure_app_loaded; keep refresh_app as
        // ensure for nav chips so we do not re-discover every chrome paint.
        self.ensure_app_loaded(workspace_root, app_id);
    }

    fn insert_projection(&self, app_id: &str, projection: AdminRegistryProjection, digest: String) {
        self.by_app
            .write()
            .expect("admin registry")
            .insert(app_id.to_string(), projection);
        self.loaded_digests
            .write()
            .expect("admin registry digests")
            .insert(app_id.to_string(), digest);
    }

    pub fn projection_for_app(&self, app_id: &str) -> Option<AdminRegistryProjection> {
        self.by_app
            .read()
            .expect("admin registry")
            .get(app_id)
            .cloned()
    }

    pub fn resource(
        &self,
        app_id: &str,
        resource_id: &str,
        module_id: &str,
    ) -> Option<AdminEntryProjection> {
        self.projection_for_app(app_id)?
            .resources
            .into_iter()
            .find(|r| {
                r.registry_entry.resource_id == resource_id
                    && r.registry_entry.module_id == module_id
            })
    }

    pub fn diagnostics(&self) -> Vec<AdminDiscoveryDiagnostic> {
        self.diagnostics
            .read()
            .expect("admin registry diags")
            .clone()
    }

    pub fn nav_items_for_capabilities(
        &self,
        app_id: &str,
        has_capability: &dyn Fn(&str) -> bool,
    ) -> Vec<AdminEntryProjection> {
        let Some(proj) = self.projection_for_app(app_id) else {
            return Vec::new();
        };
        filter_admin_resources_for_capabilities(&proj.resources, has_capability)
            .into_iter()
            .cloned()
            .collect()
    }
}

/// Discover → warm admin scenes → enrich → write `admin-registry.json`.
pub fn materialize_admin_registry_for_app(
    workspace_root: &Path,
    app_id: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let app_id = app_id.trim();
    anyhow::ensure!(!app_id.is_empty(), "app_id is required");
    let app_root = resolve_app_root(workspace_root, app_id);
    let mut projection = match discover_app_admin_resources(app_root.as_path(), app_id) {
        AdminDiscoverOutcome::None => AdminRegistryProjection {
            app_id: app_id.to_string(),
            api_version: mei_lang_kernel::ADMIN_RESOURCE_API_VERSION.to_string(),
            admin_registry_digest: String::new(),
            page_structure_digest: String::new(),
            resources: Vec::new(),
        },
        AdminDiscoverOutcome::Ok(proj) => proj,
        AdminDiscoverOutcome::Err(diag) => {
            anyhow::bail!(
                "admin discover failed for {app_id}: [{}] {}",
                diag.kind,
                diag.message
            );
        }
    };

    warm_admin_scene_manifests(workspace_root, app_id, &projection);
    enrich_projection_artifacts(workspace_root, &mut projection);
    let path = write_admin_registry_artifact(app_root.as_path(), &projection)?;
    tracing::info!(
        target: "mei.admin_registry",
        app_id = %app_id,
        path = %path.display(),
        resources = projection.resources.len(),
        "materialized admin-registry.json"
    );
    Ok(path)
}

fn warm_admin_scene_manifests(
    workspace_root: &Path,
    app_id: &str,
    projection: &AdminRegistryProjection,
) {
    let mut seen = std::collections::BTreeSet::new();
    for resource in &projection.resources {
        let scene_id = resource.page_program.root.scene_ref().to_string();
        if scene_id.trim().is_empty() || !seen.insert(scene_id.clone()) {
            continue;
        }
        if let Err(error) = mei_host_graph::warm_manifest_index_for_scope(
            workspace_root,
            app_id,
            scene_id.as_str(),
            DataMode::Static,
        ) {
            tracing::warn!(
                target: "mei.admin_registry",
                app_id = %app_id,
                scene_id = %scene_id,
                error = %error,
                "warm admin scene manifest failed (enrich may omit scene_manifest)"
            );
        }
    }
}

/// Fill `artifact_refs` / page digests using MCG assemble (shared by AOT + fallback).
pub fn enrich_projection_artifacts(
    workspace_root: &Path,
    projection: &mut AdminRegistryProjection,
) {
    let app_root = resolve_app_root(workspace_root, &projection.app_id);
    if !app_root.join("env/current").is_dir() {
        return;
    }
    for resource in &mut projection.resources {
        let scene_id = resource.page_program.root.scene_ref().to_string();
        let Ok(Some(outcome)) = mei_host_graph::assemble_scope_from_registry(
            workspace_root,
            &projection.app_id,
            &scene_id,
        ) else {
            continue;
        };
        let semantic_core = mei_host_graph::build_semantic_core_for_scene(
            workspace_root,
            &projection.app_id,
            &scene_id,
        );
        let layout_revision = outcome.compile_revision.as_str();
        if let Ok((_document, structure_ref, _)) = mei_host_graph::structure_full_from_compiled(
            workspace_root,
            &outcome.compiled,
            &semantic_core,
            layout_revision,
        ) {
            resource.artifact_refs.structure_full = Some(AdminArtifactRef {
                artifact_id: mei_host_graph::structure_full_cache_key(
                    &semantic_core,
                    layout_revision,
                ),
                content_hash: structure_ref.content_hash.clone(),
                kind: "structure.full".to_string(),
                schema_version: Some(structure_ref.schema_version.clone()),
            });
            resource.page_structure_digest = digest_json(&(
                &resource.page_structure_digest,
                &structure_ref.content_hash,
                &outcome.compile_revision,
            ));
        }
        let runtime_document = mei_host_graph::runtime_plans_from_outcome(&outcome, workspace_root);
        if let Ok(runtime_ref) =
            mei_host_graph::persist_runtime_plans(app_root.as_path(), &runtime_document)
        {
            resource.artifact_refs.runtime_plans = Some(AdminArtifactRef {
                artifact_id: mei_host_graph::runtime_plans_cache_key(
                    &semantic_core,
                    layout_revision,
                ),
                content_hash: runtime_ref.content_hash,
                kind: "runtime.plans".to_string(),
                schema_version: Some(runtime_ref.schema_version),
            });
        }
        let mut hits = mei_host_graph::ArtifactHitMatrix::default();
        if let Ok(index) = mei_host_graph::ensure_manifest_index(
            workspace_root,
            &projection.app_id,
            &scene_id,
            DataMode::Static,
            &mut hits,
            None,
        ) {
            resource.artifact_refs.scene_manifest = Some(AdminArtifactRef {
                artifact_id: format!("scene-view-manifest:{}:{}", projection.app_id, scene_id),
                content_hash: index.manifest_revision_digest,
                kind: "scene.view-manifest".to_string(),
                schema_version: Some(index.schema_version),
            });
        }
    }
    projection.page_structure_digest = digest_json(
        &projection
            .resources
            .iter()
            .map(|resource| resource.page_structure_digest.as_str())
            .collect::<Vec<_>>(),
    );
}

fn digest_json(value: &impl serde::Serialize) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{
        write_admin_registry_artifact, AdminArtifactRefs, AdminDangerLevel, AdminRegistryEntry,
        PageProgram, ADMIN_RESOURCE_API_VERSION,
    };
    use tempfile::tempdir;

    #[test]
    fn ensure_app_loaded_uses_artifact_without_admin_mdx() {
        let dir = tempdir().unwrap();
        let workspace = dir.path();
        let app_root = workspace.join("apps/demo");
        let env_ver = app_root.join("env/WS-20260720.0");
        std::fs::create_dir_all(env_ver.join("build/registry")).unwrap();
        std::os::unix::fs::symlink("WS-20260720.0", app_root.join("env/current")).unwrap();
        // No src/admin — discover would yield None; artifact must be the source.
        let projection = AdminRegistryProjection {
            app_id: "demo".to_string(),
            api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
            admin_registry_digest: "sha256:aot".to_string(),
            page_structure_digest: "sha256:page".to_string(),
            resources: vec![AdminEntryProjection {
                registry_entry: AdminRegistryEntry {
                    api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
                    app_id: "demo".to_string(),
                    resource_id: "theme".to_string(),
                    module_id: "cockpit".to_string(),
                    resource_key: "app:demo.theme.cockpit".to_string(),
                    canonical_route: "/admin/apps/demo/theme/cockpit".to_string(),
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
        };
        write_admin_registry_artifact(&app_root, &projection).unwrap();

        let registry = AdminRegistry::new();
        registry.ensure_app_loaded(workspace, "demo");
        let loaded = registry
            .resource("demo", "theme", "cockpit")
            .expect("artifact resource");
        assert_eq!(loaded.registry_entry.title, "外观");
        assert_eq!(
            registry
                .loaded_digests
                .read()
                .unwrap()
                .get("demo")
                .map(String::as_str),
            Some("sha256:aot")
        );
    }
}
