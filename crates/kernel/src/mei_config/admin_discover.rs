//! Discover and project app admin resources for Host Registry (0547).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::admin_manifest::{
    load_admin_manifest, resolve_admin_manifest_path, AdminManifest, AdminProviderKind,
    AdminResourceSpec, AdminTemplate, AppAdminRef, ADMIN_RESOURCE_API_VERSION,
};
use super::app_manifest::AppTomlDocument;
use super::types::APP_TOML_FILENAME;

/// Successful per-app admin projection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRegistryProjection {
    pub app_id: String,
    pub api_version: String,
    pub manifest_digest: String,
    pub resources: Vec<AdminResourceProjection>,
}

/// How Admin Shell should mount the resource main pane (0548 Host builtins / 0549 asset-slot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AdminUiSurface {
    /// Phase B Form Card + config-record.
    #[default]
    FormCard,
    /// Embed manage-ops-panel; writes via `/api/ops/*`.
    OpsEmbed,
    /// Embed upload-panel; writes via `/api/upload/*`.
    UploadEmbed,
    /// Phase D Asset Slot Kit; writes via `/api/admin/providers/asset-slot`.
    AssetSlotCollection,
}

fn ui_surface_for_template(template: AdminTemplate) -> AdminUiSurface {
    match template {
        AdminTemplate::AssetSlotCollection => AdminUiSurface::AssetSlotCollection,
        AdminTemplate::SingletonForm
        | AdminTemplate::CollectionDetail
        | AdminTemplate::ActionJobConsole => AdminUiSurface::FormCard,
    }
}

/// One navigable / callable resource after Host namespace injection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminResourceProjection {
    pub resource_key: String,
    pub resource_id: String,
    pub app_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub template: AdminTemplate,
    pub provider: AdminProviderKind,
    pub required_capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_path: Option<String>,
    pub href: String,
    #[serde(default)]
    pub ui_surface: AdminUiSurface,
    pub spec: AdminResourceSpec,
}

/// Isolated discovery failure for one app (does not poison Host).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminDiscoveryDiagnostic {
    pub app_id: String,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum AdminDiscoverOutcome {
    None,
    Ok(AdminRegistryProjection),
    Err(AdminDiscoveryDiagnostic),
}

pub fn discover_app_admin_resources(app_root: &Path, app_id: &str) -> AdminDiscoverOutcome {
    let toml_path = app_root.join(APP_TOML_FILENAME);
    if !toml_path.is_file() {
        return AdminDiscoverOutcome::None;
    }
    let raw = match fs::read_to_string(&toml_path) {
        Ok(raw) => raw,
        Err(e) => {
            return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
                app_id: app_id.to_string(),
                kind: "io".into(),
                message: format!("read {}: {e}", toml_path.display()),
            });
        }
    };
    let doc: AppTomlDocument = match toml::from_str(&raw) {
        Ok(doc) => doc,
        Err(e) => {
            return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
                app_id: app_id.to_string(),
                kind: "parse".into(),
                message: format!("parse {}: {e}", toml_path.display()),
            });
        }
    };
    discover_from_admin_ref(app_root, app_id, &doc.admin)
}

pub fn discover_from_admin_ref(
    app_root: &Path,
    app_id: &str,
    admin_ref: &AppAdminRef,
) -> AdminDiscoverOutcome {
    let manifest_path = match resolve_admin_manifest_path(app_root, admin_ref) {
        Ok(Some(path)) => path,
        Ok(None) => return AdminDiscoverOutcome::None,
        Err(e) => {
            return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
                app_id: app_id.to_string(),
                kind: "validation".into(),
                message: e.to_string(),
            });
        }
    };
    if !manifest_path.is_file() {
        return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
            app_id: app_id.to_string(),
            kind: "not-found".into(),
            message: format!("admin manifest missing: {}", manifest_path.display()),
        });
    }
    let manifest = match load_admin_manifest(&manifest_path) {
        Ok(m) => m,
        Err(e) => {
            return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
                app_id: app_id.to_string(),
                kind: "validation".into(),
                message: e.to_string(),
            });
        }
    };
    match project_manifest(app_id, &manifest, &manifest_path) {
        Ok(projection) => AdminDiscoverOutcome::Ok(projection),
        Err(diag) => AdminDiscoverOutcome::Err(diag),
    }
}

fn project_manifest(
    app_id: &str,
    manifest: &AdminManifest,
    manifest_path: &Path,
) -> Result<AdminRegistryProjection, AdminDiscoveryDiagnostic> {
    let raw = fs::read_to_string(manifest_path).map_err(|e| AdminDiscoveryDiagnostic {
        app_id: app_id.to_string(),
        kind: "io".into(),
        message: e.to_string(),
    })?;
    let digest = hex_sha256(raw.as_bytes());
    let resources = manifest
        .resources
        .iter()
        .map(|spec| AdminResourceProjection {
            resource_key: format!("app:{app_id}.{}", spec.resource_id),
            resource_id: spec.resource_id.clone(),
            app_id: app_id.to_string(),
            title: spec.title.clone(),
            description: spec.description.clone(),
            template: spec.template,
            provider: spec.provider,
            required_capabilities: spec.required_capabilities.clone(),
            record_path: spec.record_path.clone(),
            href: format!("/admin/apps/{app_id}/{}", spec.resource_id),
            ui_surface: ui_surface_for_template(spec.template),
            spec: spec.clone(),
        })
        .collect();
    Ok(AdminRegistryProjection {
        app_id: app_id.to_string(),
        api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
        manifest_digest: digest,
        resources,
    })
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Filter resources the principal may see in navigation (capability names as strings).
pub fn filter_admin_resources_for_capabilities<'a>(
    resources: &'a [AdminResourceProjection],
    has_capability: &dyn Fn(&str) -> bool,
) -> Vec<&'a AdminResourceProjection> {
    resources
        .iter()
        .filter(|r| {
            r.required_capabilities
                .iter()
                .all(|cap| has_capability(cap.as_str()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
    }

    #[test]
    fn discovers_valid_admin_loop_fixture() {
        let app_root = fixtures_root().join("admin-loop-app");
        if !app_root.join("app.toml").is_file() {
            // Fixture created in same Phase B task; skip soft only if missing mid-edit.
            // Prefer fail when fixture expected — create before this test lands.
            panic!("missing tests/fixtures/admin-loop-app (Phase B fixture)");
        }
        match discover_app_admin_resources(&app_root, "admin-loop-app") {
            AdminDiscoverOutcome::Ok(proj) => {
                assert_eq!(proj.app_id, "admin-loop-app");
                assert!(proj
                    .resources
                    .iter()
                    .any(|r| r.resource_id == "organization"));
                let org = proj
                    .resources
                    .iter()
                    .find(|r| r.resource_id == "organization")
                    .unwrap();
                assert_eq!(org.href, "/admin/apps/admin-loop-app/organization");
                assert_eq!(org.resource_key, "app:admin-loop-app.organization");
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn isolates_bad_manifest() {
        let app_root = fixtures_root().join("admin-manifest");
        // Point at invalid host_namespace via a synthetic app.toml would need temp dir;
        // use load path through discover_from_admin_ref with bad file.
        let dir = tempfile::tempdir().unwrap();
        let admin_dir = dir.path().join("admin");
        fs::create_dir_all(&admin_dir).unwrap();
        fs::write(
            admin_dir.join("admin.toml"),
            fs::read_to_string(app_root.join("invalid/host_namespace.toml")).unwrap(),
        )
        .unwrap();
        let admin_ref = AppAdminRef {
            manifest: Some("admin/admin.toml".into()),
        };
        match discover_from_admin_ref(dir.path(), "bad-app", &admin_ref) {
            AdminDiscoverOutcome::Err(diag) => {
                assert_eq!(diag.app_id, "bad-app");
                assert!(diag.message.contains("host") || diag.message.contains("namespace"));
            }
            other => panic!("expected Err, got {other:?}"),
        }
    }
}
