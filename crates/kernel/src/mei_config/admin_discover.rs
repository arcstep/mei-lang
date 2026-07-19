//! Discover and project app admin resources for Host Registry (0547).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::model::{AdminPageProgram, PageProgram};

use super::admin_manifest::{
    load_admin_manifest, load_admin_mdx_resource, resolve_admin_manifest_path,
    validate_admin_manifest, AdminManifest, AdminProviderKind, AdminResourceSpec, AdminTemplate,
    AppAdminRef, ADMIN_RESOURCE_API_VERSION,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    pub href: String,
    pub page_program: AdminPageProgram,
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
    let mdx_paths = match discover_admin_mdx_paths(app_root) {
        Ok(paths) => paths,
        Err(message) => {
            return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
                app_id: app_id.to_string(),
                kind: "io".into(),
                message,
            });
        }
    };
    if !mdx_paths.is_empty() {
        if !doc.admin.is_empty() {
            return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
                app_id: app_id.to_string(),
                kind: "dual-source".into(),
                message: "admin resources must use either src/admin/**/*.admin.mdx or [admin].manifest, not both".into(),
            });
        }
        return discover_from_admin_mdx(app_root, app_id, &mdx_paths);
    }
    discover_from_admin_ref(app_root, app_id, &doc.admin)
}

pub fn discover_admin_mdx_paths(app_root: &Path) -> Result<Vec<PathBuf>, String> {
    let source_root = app_root.join("src/admin");
    if !source_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in WalkDir::new(&source_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !name.starts_with('.') && name != "node_modules"
        })
    {
        let entry = entry.map_err(|error| format!("walk {}: {error}", source_root.display()))?;
        if entry.file_type().is_file()
            && entry
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".admin.mdx"))
        {
            paths.push(entry.into_path());
        }
    }
    paths.sort();
    Ok(paths)
}

fn discover_from_admin_mdx(
    app_root: &Path,
    app_id: &str,
    paths: &[PathBuf],
) -> AdminDiscoverOutcome {
    let mut resources = Vec::new();
    let mut digest_input = Vec::new();
    let mut source_anchors = BTreeMap::new();
    for path in paths {
        let resource = match load_admin_mdx_resource(path) {
            Ok(resource) => resource,
            Err(error) => {
                return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
                    app_id: app_id.to_string(),
                    kind: "validation".into(),
                    message: format!("{}: {error}", path.display()),
                });
            }
        };
        let raw = match fs::read(path) {
            Ok(raw) => raw,
            Err(error) => {
                return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
                    app_id: app_id.to_string(),
                    kind: "io".into(),
                    message: format!("read {}: {error}", path.display()),
                });
            }
        };
        let relative = path.strip_prefix(app_root).unwrap_or(path);
        let source_anchor = relative.to_string_lossy().replace('\\', "/");
        digest_input.extend_from_slice(source_anchor.as_bytes());
        digest_input.push(0);
        digest_input.extend_from_slice(&raw);
        digest_input.push(0xff);
        source_anchors.insert(resource.resource_id.clone(), source_anchor);
        resources.push(resource);
    }
    let manifest = AdminManifest {
        api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
        resources,
    };
    if let Err(error) = validate_admin_manifest(&manifest) {
        return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
            app_id: app_id.to_string(),
            kind: "validation".into(),
            message: error.to_string(),
        });
    }
    AdminDiscoverOutcome::Ok(project_manifest_with_digest(
        app_id,
        &manifest,
        hex_sha256(&digest_input),
        &source_anchors,
    ))
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
    let source_anchor = manifest_path.to_string_lossy().replace('\\', "/");
    let source_anchors = manifest
        .resources
        .iter()
        .map(|resource| (resource.resource_id.clone(), source_anchor.clone()))
        .collect();
    Ok(project_manifest_with_digest(
        app_id,
        manifest,
        digest,
        &source_anchors,
    ))
}

fn project_manifest_with_digest(
    app_id: &str,
    manifest: &AdminManifest,
    digest: String,
    source_anchors: &BTreeMap<String, String>,
) -> AdminRegistryProjection {
    let resources = manifest
        .resources
        .iter()
        .map(|spec| {
            let source_anchor = source_anchors
                .get(&spec.resource_id)
                .cloned()
                .unwrap_or_else(|| format!("src/admin/{}.admin.mdx", spec.resource_id));
            AdminResourceProjection {
                resource_key: format!("app:{app_id}.{}", spec.resource_id),
                resource_id: spec.resource_id.clone(),
                app_id: app_id.to_string(),
                title: spec.title.clone(),
                description: spec.description.clone(),
                template: spec.template,
                provider: spec.provider,
                required_capabilities: spec.required_capabilities.clone(),
                record_path: spec.record_path.clone(),
                config_path: spec.config_path.clone(),
                href: format!("/admin/apps/{app_id}/{}", spec.resource_id),
                page_program: AdminPageProgram::new(
                    spec.resource_id.clone(),
                    PageProgram::from_scene_ref(
                        spec.resource_id.clone(),
                        Some(spec.title.clone()),
                        source_anchor,
                        format!("admin/{}", spec.resource_id),
                    ),
                ),
                ui_surface: ui_surface_for_template(spec.template),
                spec: spec.clone(),
            }
        })
        .collect();
    AdminRegistryProjection {
        app_id: app_id.to_string(),
        api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
        manifest_digest: digest,
        resources,
    }
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
    fn discovers_convention_admin_mdx_without_manifest_pointer() {
        let app_root = fixtures_root().join("admin-mdx-app");
        match discover_app_admin_resources(&app_root, "admin-mdx-app") {
            AdminDiscoverOutcome::Ok(projection) => {
                let ids = projection
                    .resources
                    .iter()
                    .map(|resource| resource.resource_id.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(ids, vec!["organization", "theme"]);
                let organization = projection
                    .resources
                    .iter()
                    .find(|resource| resource.resource_id == "organization")
                    .expect("organization");
                assert_eq!(organization.spec.sections.len(), 1);
                assert_eq!(organization.spec.sections[0].fields.len(), 2);
                assert_eq!(organization.page_program.page.surface.as_str(), "document");
                assert_eq!(
                    organization.page_program.page.source_anchor,
                    "src/admin/organization.admin.mdx"
                );
                assert_eq!(
                    organization.spec.apply_policy,
                    Some(super::super::admin_manifest::AdminApplyPolicy::Hot)
                );
                let theme = projection
                    .resources
                    .iter()
                    .find(|resource| resource.resource_id == "theme")
                    .expect("theme");
                assert_eq!(theme.config_path.as_deref(), Some("ops.themes.cockpit"));
                assert_eq!(
                    theme.spec.apply_policy,
                    Some(super::super::admin_manifest::AdminApplyPolicy::Hot)
                );
                assert_eq!(
                    theme.page_program.page.source_anchor,
                    "src/admin/theme.admin.mdx"
                );
            }
            other => panic!("expected convention MDX projection, got {other:?}"),
        }
    }

    #[test]
    fn discovers_phase_d_fixture_from_admin_mdx_without_toml_manifest() {
        let app_root = fixtures_root().join("admin-phase-d-app");
        assert!(!app_root.join("admin/admin.toml").is_file());
        match discover_app_admin_resources(&app_root, "admin-phase-d-app") {
            AdminDiscoverOutcome::Ok(projection) => {
                let ids = projection
                    .resources
                    .iter()
                    .map(|resource| resource.resource_id.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(ids, vec!["datasources", "organization"]);
                let datasources = projection
                    .resources
                    .iter()
                    .find(|resource| resource.resource_id == "datasources")
                    .expect("datasources");
                assert_eq!(
                    datasources.page_program.page.source_anchor,
                    "src/admin/datasources.admin.mdx"
                );
                assert_eq!(datasources.ui_surface, AdminUiSurface::AssetSlotCollection);
            }
            other => panic!("expected phase-d MDX projection, got {other:?}"),
        }
    }

    #[test]
    fn rejects_toml_and_mdx_dual_source() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/admin")).unwrap();
        fs::create_dir_all(dir.path().join("admin")).unwrap();
        fs::write(
            dir.path().join("src/admin/organization.admin.mdx"),
            fs::read_to_string(
                fixtures_root().join("admin-mdx-app/src/admin/organization.admin.mdx"),
            )
            .unwrap(),
        )
        .unwrap();
        fs::write(
            dir.path().join("admin/admin.toml"),
            fs::read_to_string(fixtures_root().join("admin-loop-app/admin/admin.toml")).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.path().join("app.toml"),
            "schema_version = \"mei-app-v1\"\n[admin]\nmanifest = \"admin/admin.toml\"\n",
        )
        .unwrap();
        match discover_app_admin_resources(dir.path(), "dual") {
            AdminDiscoverOutcome::Err(diagnostic) => {
                assert_eq!(diagnostic.kind, "dual-source");
            }
            other => panic!("expected dual-source diagnostic, got {other:?}"),
        }
    }

    #[test]
    fn admin_mdx_fixture_theme_config_path_round_trip() {
        use crate::mei_config::admin_record::{get_config_path_record, put_config_path_record};
        use crate::mei_config::types::APP_TOML_FILENAME;

        let src = fixtures_root().join("admin-mdx-app");
        let dir = tempfile::tempdir().unwrap();
        let app_root = dir.path().join("app");
        copy_dir_recursive(&src, &app_root);

        match discover_app_admin_resources(&app_root, "admin-mdx-app") {
            AdminDiscoverOutcome::Ok(projection) => {
                let theme = projection
                    .resources
                    .iter()
                    .find(|resource| resource.resource_id == "theme")
                    .expect("theme");
                let config_path = theme.config_path.as_deref().expect("config_path");
                assert_eq!(config_path, "ops.themes.cockpit");
                let before = get_config_path_record(&app_root, config_path).unwrap();
                assert_eq!(before.data["tokens"]["font"]["family_ui"], "system-ui");
                let after = put_config_path_record(
                    &app_root,
                    config_path,
                    before.revision,
                    {
                        let mut next = before.data.clone();
                        next["tokens"]["font"]["family_ui"] = serde_json::json!("IBM Plex Sans");
                        next
                    },
                    "tester",
                    "admin-mdx-app",
                    "theme",
                    "corr-theme",
                )
                .unwrap();
                assert_eq!(after.revision, before.revision + 1);
                let loaded = get_config_path_record(&app_root, config_path).unwrap();
                assert_eq!(loaded.data["tokens"]["font"]["family_ui"], "IBM Plex Sans");
                let raw = fs::read_to_string(app_root.join(APP_TOML_FILENAME)).unwrap();
                assert!(raw.contains("title = \"Admin MDX Fixture\""));
                assert!(raw.contains("IBM Plex Sans"));
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) {
        fs::create_dir_all(dst).unwrap();
        for entry in fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            let to = dst.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir_recursive(&entry.path(), &to);
            } else {
                fs::copy(entry.path(), to).unwrap();
            }
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
