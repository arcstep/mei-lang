//! In-memory Admin Resource Registry (0547 + 0548 Host builtins).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use mei_lang_kernel::{
    discover_app_admin_resources, filter_admin_resources_for_capabilities, resolve_app_root,
    AdminDiscoverOutcome, AdminDiscoveryDiagnostic, AdminRegistryProjection,
    AdminResourceProjection, WorkspaceAppMeta,
};

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
                AdminDiscoverOutcome::Ok(proj) => Some(proj),
                AdminDiscoverOutcome::Err(diag) => {
                    diags.push(diag);
                    None
                }
            };
            next.insert(app.id.clone(), merge_host_builtins(app.id.as_str(), manifest));
        }
        *self.by_app.write().expect("admin registry") = next;
        *self.diagnostics.write().expect("admin registry diags") = diags;
    }

    pub fn refresh_app(&self, workspace_root: &Path, app_id: &str) {
        let app_root = resolve_app_root(workspace_root, app_id);
        let manifest = match discover_app_admin_resources(&app_root, app_id) {
            AdminDiscoverOutcome::None => None,
            AdminDiscoverOutcome::Ok(proj) => {
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
    ) -> Option<AdminResourceProjection> {
        self.projection_for_app(app_id)?
            .resources
            .into_iter()
            .find(|r| r.resource_id == resource_id)
    }

    pub fn diagnostics(&self) -> Vec<AdminDiscoveryDiagnostic> {
        self.diagnostics.read().expect("admin registry diags").clone()
    }

    pub fn nav_items_for_capabilities(
        &self,
        app_id: &str,
        has_capability: &dyn Fn(&str) -> bool,
    ) -> Vec<AdminResourceProjection> {
        let Some(proj) = self.projection_for_app(app_id) else {
            return Vec::new();
        };
        filter_admin_resources_for_capabilities(&proj.resources, has_capability)
            .into_iter()
            .cloned()
            .collect()
    }
}
