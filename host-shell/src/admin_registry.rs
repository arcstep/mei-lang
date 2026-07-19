//! In-memory Admin Resource Registry (0547 + 0548 Host builtins).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use mei_lang_kernel::{
    discover_app_admin_resources, filter_admin_resources_for_capabilities, resolve_app_root,
    AdminArtifactRef, AdminDiscoverOutcome, AdminDiscoveryDiagnostic, AdminEntryProjection,
    AdminRegistryProjection, DataMode, WorkspaceAppMeta,
};
use sha2::{Digest, Sha256};

use crate::host_builtin::merge_host_builtins;

#[derive(Debug, Default)]
pub struct AdminRegistry {
    by_app: RwLock<HashMap<String, AdminRegistryProjection>>,
    diagnostics: RwLock<Vec<AdminDiscoveryDiagnostic>>,
}

pub type SharedAdminRegistry = Arc<AdminRegistry>;

impl AdminRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared() -> SharedAdminRegistry {
        Arc::new(Self::new())
    }

    pub fn refresh_workspace(&self, workspace_root: &Path, apps: &[WorkspaceAppMeta]) {
        let mut next = HashMap::new();
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
            next.insert(
                app.id.clone(),
                merge_host_builtins(app.id.as_str(), manifest),
            );
        }
        *self.by_app.write().expect("admin registry") = next;
        *self.diagnostics.write().expect("admin registry diags") = diags;
    }

    pub fn refresh_app(&self, workspace_root: &Path, app_id: &str) {
        let app_root = resolve_app_root(workspace_root, app_id);
        let manifest = match discover_app_admin_resources(&app_root, app_id) {
            AdminDiscoverOutcome::None => None,
            AdminDiscoverOutcome::Ok(mut proj) => {
                enrich_projection_artifacts(workspace_root, &mut proj);
                self.diagnostics
                    .write()
                    .expect("admin registry diags")
                    .retain(|d| d.app_id != app_id);
                Some(proj)
            }
            AdminDiscoverOutcome::Err(diag) => {
                let mut diags = self.diagnostics.write().expect("admin registry diags");
                diags.retain(|d| d.app_id != app_id);
                diags.push(diag);
                None
            }
        };
        self.by_app
            .write()
            .expect("admin registry")
            .insert(app_id.to_string(), merge_host_builtins(app_id, manifest));
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

fn enrich_projection_artifacts(workspace_root: &Path, projection: &mut AdminRegistryProjection) {
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
