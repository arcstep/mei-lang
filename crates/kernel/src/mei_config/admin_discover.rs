//! Convention-only v2 Admin Entry discovery and kernel projection.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use mei_syntax::v2::{parse_v2_source_file, V2Expr, V2Item};
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::model::{PageFill, PageProgram};

use super::admin_registry::{
    AdminArtifactRefs, AdminDangerLevel, AdminNavigation, AdminRegistryEntry,
    ADMIN_RESOURCE_API_VERSION,
};
use super::provider_binding::{discover_provider_binding_catalog, provider_bindings_for_scene};
use super::types::APP_TOML_FILENAME;

pub const ADMIN_SOURCE_PATH_INVALID: &str = "admin_source_path_invalid";
pub const ADMIN_MODULE_ID_DUPLICATE: &str = "admin_module_id_duplicate";
pub const ADMIN_ENTRY_MODULE_FORBIDDEN: &str = "admin_entry_module_forbidden";
pub const ADMIN_SCENE_ROOT_UNKNOWN: &str = "admin_scene_root_unknown";
pub const ADMIN_SCENE_ROOT_DUPLICATE: &str = "admin_scene_root_duplicate";
pub const ADMIN_LEGACY_MANIFEST_FORBIDDEN: &str = "admin_legacy_manifest_forbidden";
pub const ADMIN_LEGACY_DATA_JSON_FORBIDDEN: &str = "admin_legacy_data_json_forbidden";
pub const ADMIN_LEGACY_DUAL_PROJECTION_FORBIDDEN: &str = "admin_legacy_dual_projection_forbidden";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRegistryProjection {
    pub app_id: String,
    pub api_version: String,
    pub admin_registry_digest: String,
    pub page_structure_digest: String,
    pub resources: Vec<AdminEntryProjection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminEntryProjection {
    pub registry_entry: AdminRegistryEntry,
    pub page_program: PageProgram,
    pub page_structure_digest: String,
    #[serde(default)]
    pub artifact_refs: AdminArtifactRefs,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneRootCatalogEntry {
    pub root_id: String,
    pub source_anchor: String,
}

pub fn discover_app_admin_resources(app_root: &Path, app_id: &str) -> AdminDiscoverOutcome {
    if let Some(diagnostic) = reject_legacy_manifest_pointer(app_root, app_id) {
        return AdminDiscoverOutcome::Err(diagnostic);
    }
    if let Some(diagnostic) = reject_legacy_admin_sources(app_root, app_id) {
        return AdminDiscoverOutcome::Err(diagnostic);
    }
    let paths = match discover_admin_mdx_paths(app_root) {
        Ok(paths) if paths.is_empty() => return AdminDiscoverOutcome::None,
        Ok(paths) => paths,
        Err(message) => return diagnostic_from_message(app_id, message),
    };
    let scene_catalog = match discover_scene_root_catalog(app_root) {
        Ok(catalog) => catalog,
        Err(message) => return diagnostic_from_message(app_id, message),
    };
    let provider_catalog = match discover_provider_binding_catalog(app_root) {
        Ok(catalog) => catalog,
        Err(message) => return diagnostic_from_message(app_id, message),
    };
    project_entries(app_root, app_id, &paths, &scene_catalog, &provider_catalog)
}

fn reject_legacy_admin_sources(app_root: &Path, app_id: &str) -> Option<AdminDiscoveryDiagnostic> {
    let legacy_root = app_root.join("admin");
    if legacy_root.is_dir() {
        let legacy_json = WalkDir::new(&legacy_root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .find(|entry| {
                entry.file_type().is_file()
                    && entry.path().extension().and_then(|value| value.to_str()) == Some("json")
            });
        if let Some(entry) = legacy_json {
            return Some(AdminDiscoveryDiagnostic {
                app_id: app_id.to_string(),
                kind: ADMIN_LEGACY_DATA_JSON_FORBIDDEN.to_string(),
                message: format!(
                    "[{ADMIN_LEGACY_DATA_JSON_FORBIDDEN}] source Admin JSON is forbidden: `{}`",
                    entry.path().display()
                ),
            });
        }
    }

    let source_root = app_root.join("src/admin");
    if !source_root.is_dir() {
        return None;
    }
    let mut legacy_index = None;
    let mut legacy_page = None;
    for entry in WalkDir::new(&source_root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let name = entry.file_name().to_string_lossy();
        if name == "index.admin.mdx" {
            legacy_index = Some(entry.path().to_path_buf());
        } else if name == "page.mei" {
            legacy_page = Some(entry.path().to_path_buf());
        }
    }
    match (legacy_index, legacy_page) {
        (Some(index), Some(page)) => Some(AdminDiscoveryDiagnostic {
            app_id: app_id.to_string(),
            kind: ADMIN_LEGACY_DUAL_PROJECTION_FORBIDDEN.to_string(),
            message: format!(
                "[{ADMIN_LEGACY_DUAL_PROJECTION_FORBIDDEN}] legacy Admin dual projection is forbidden: `{}` + `{}`",
                index.display(),
                page.display()
            ),
        }),
        _ => None,
    }
}

fn reject_legacy_manifest_pointer(
    app_root: &Path,
    app_id: &str,
) -> Option<AdminDiscoveryDiagnostic> {
    let path = app_root.join(APP_TOML_FILENAME);
    let raw = fs::read_to_string(path).ok()?;
    let value = toml::from_str::<toml::Value>(&raw).ok()?;
    value.get("admin").map(|_| AdminDiscoveryDiagnostic {
        app_id: app_id.to_string(),
        kind: ADMIN_LEGACY_MANIFEST_FORBIDDEN.to_string(),
        message: "`app.toml [admin].manifest` is not supported by Admin v2".to_string(),
    })
}

fn diagnostic_from_message(app_id: &str, message: String) -> AdminDiscoverOutcome {
    let kind = message
        .strip_prefix('[')
        .and_then(|value| value.split_once(']'))
        .map(|(code, _)| code)
        .unwrap_or("io")
        .to_string();
    AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
        app_id: app_id.to_string(),
        kind,
        message,
    })
}

pub fn discover_admin_mdx_paths(app_root: &Path) -> Result<Vec<PathBuf>, String> {
    let source_root = app_root.join("src/admin");
    if !source_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    let mut keys = BTreeSet::new();
    for entry in WalkDir::new(&source_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
    {
        let entry = entry.map_err(|error| format!("walk {}: {error}", source_root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(&source_root)
            .map_err(|_| format!("[{ADMIN_SOURCE_PATH_INVALID}] {}", path.display()))?;
        let components = relative.components().collect::<Vec<_>>();
        let extension = path.extension().and_then(|value| value.to_str());
        if extension != Some("mdx") {
            return Err(format!(
                "[{ADMIN_ENTRY_MODULE_FORBIDDEN}] src/admin contains non-MDX file `{}`",
                relative.display()
            ));
        }
        if components.len() != 2 {
            return Err(format!(
                "[{ADMIN_SOURCE_PATH_INVALID}] `{}` must match src/admin/{{resource}}/{{module}}.mdx",
                relative.display()
            ));
        }
        let resource_id = components[0].as_os_str().to_string_lossy();
        let module_id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if !valid_identity(&resource_id)
            || !valid_identity(module_id)
            || file_name.ends_with(".admin.mdx")
            || file_name == "index.admin.mdx"
        {
            return Err(format!(
                "[{ADMIN_SOURCE_PATH_INVALID}] invalid Admin Entry path `{}`",
                relative.display()
            ));
        }
        let key = (resource_id.into_owned(), module_id.to_string());
        if !keys.insert(key.clone()) {
            return Err(format!(
                "[{ADMIN_MODULE_ID_DUPLICATE}] duplicate Admin Entry `{}/{}`",
                key.0, key.1
            ));
        }
        paths.push(path.to_path_buf());
    }
    paths.sort();
    Ok(paths)
}

pub fn discover_scene_root_catalog(
    app_root: &Path,
) -> Result<BTreeMap<String, SceneRootCatalogEntry>, String> {
    let scene_root = app_root.join("src/scene");
    if !scene_root.is_dir() {
        return Ok(BTreeMap::new());
    }
    let mut catalog = BTreeMap::new();
    for entry in WalkDir::new(&scene_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
    {
        let entry = entry.map_err(|error| format!("walk {}: {error}", scene_root.display()))?;
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("mei")
        {
            continue;
        }
        let source_anchor = path
            .strip_prefix(app_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let root_ids = scene_ids_from_v2(path);
        let root_ids = if root_ids.is_empty() {
            vec![fallback_scene_id(&scene_root, path)]
        } else {
            root_ids
        };
        for root_id in root_ids {
            let value = SceneRootCatalogEntry {
                root_id: root_id.clone(),
                source_anchor: source_anchor.clone(),
            };
            if let Some(previous) = catalog.insert(root_id.clone(), value) {
                return Err(format!(
                    "[{ADMIN_SCENE_ROOT_DUPLICATE}] scene root `{root_id}` is declared by `{}` and `{source_anchor}`",
                    previous.source_anchor
                ));
            }
        }
    }
    Ok(catalog)
}

fn scene_ids_from_v2(path: &Path) -> Vec<String> {
    let Ok(source) = parse_v2_source_file(path) else {
        return Vec::new();
    };
    source
        .items
        .into_iter()
        .filter_map(|item| {
            let V2Item::TopLevel { name, args } = item else {
                return None;
            };
            if name != "scene" {
                return None;
            }
            args.keywords
                .into_iter()
                .find_map(|(key, value)| match (key.as_str(), value) {
                    ("id", V2Expr::String(id)) => Some(id),
                    _ => None,
                })
        })
        .collect()
}

fn fallback_scene_id(scene_root: &Path, path: &Path) -> String {
    path.strip_prefix(scene_root)
        .unwrap_or(path)
        .with_extension("")
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join(".")
}

fn project_entries(
    app_root: &Path,
    app_id: &str,
    paths: &[PathBuf],
    scene_catalog: &BTreeMap<String, SceneRootCatalogEntry>,
    provider_catalog: &BTreeMap<String, super::admin_registry::ProviderBinding>,
) -> AdminDiscoverOutcome {
    let admin_root = app_root.join("src/admin");
    let mut resources = Vec::new();
    for path in paths {
        let relative_admin = path
            .strip_prefix(&admin_root)
            .expect("validated Admin path");
        let resource_id = relative_admin
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            .expect("validated resource");
        let module_id = path
            .file_stem()
            .and_then(|value| value.to_str())
            .expect("validated module");
        let source_anchor = path
            .strip_prefix(app_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let document = match mei_syntax::parse_admin_mdx_file(path) {
            Ok(document) => document,
            Err(error) => {
                return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
                    app_id: app_id.to_string(),
                    kind: error.code.clone(),
                    message: error.to_string(),
                });
            }
        };
        let Some(scene) = scene_catalog.get(&document.scene_use) else {
            return AdminDiscoverOutcome::Err(AdminDiscoveryDiagnostic {
                app_id: app_id.to_string(),
                kind: ADMIN_SCENE_ROOT_UNKNOWN.to_string(),
                message: format!(
                    "[{ADMIN_SCENE_ROOT_UNKNOWN}] `{}` references unknown scene root `{}`",
                    source_anchor, document.scene_use
                ),
            });
        };
        let scene_path = app_root.join(&scene.source_anchor);
        let provider_bindings = match provider_bindings_for_scene(&scene_path, provider_catalog) {
            Ok(bindings) => bindings,
            Err(message) => return diagnostic_from_message(app_id, message),
        };
        let dependency_digest = match scene_dependency_digest(app_root, &scene_path) {
            Ok(digest) => digest,
            Err(message) => return diagnostic_from_message(app_id, message),
        };
        let page_structure_digest = digest_serializable(&(
            &document.visible_body,
            &document.scene_use,
            &document.fills,
            dependency_digest,
            &provider_bindings,
        ));
        let page_id = format!("{resource_id}.{module_id}");
        let fills = document
            .fills
            .iter()
            .map(|fill| PageFill {
                slot: fill.slot.clone(),
                content: fill.content.clone(),
                source: fill.source.clone(),
                source_anchor: format!("{source_anchor}:{}", fill.line),
            })
            .collect();
        let page_program = PageProgram::from_admin_entry(
            page_id,
            Some(document.frontmatter.title.clone()),
            source_anchor.clone(),
            document.scene_use,
            scene.source_anchor.clone(),
            document.visible_body.markdown,
            document.visible_body.html,
            fills,
            provider_bindings,
        );
        let navigation = if document.frontmatter.menu.is_some()
            || document.frontmatter.parent.is_some()
            || document.frontmatter.order.is_some()
            || !document.frontmatter.keywords.is_empty()
            || document.frontmatter.default.is_some()
        {
            Some(AdminNavigation {
                menu: document.frontmatter.menu,
                parent: document.frontmatter.parent,
                order: document.frontmatter.order,
                keywords: document.frontmatter.keywords,
                default: document.frontmatter.default,
            })
        } else {
            None
        };
        let registry_entry = AdminRegistryEntry {
            api_version: document.frontmatter.api_version,
            app_id: app_id.to_string(),
            resource_id: resource_id.to_string(),
            module_id: module_id.to_string(),
            resource_key: format!("app:{app_id}.{resource_id}.{module_id}"),
            canonical_route: format!("/admin/apps/{app_id}/{resource_id}/{module_id}"),
            title: document.frontmatter.title,
            short_title: document.frontmatter.short_title,
            description: document.frontmatter.description,
            navigation,
            required_capabilities: document.frontmatter.required_capabilities,
            scope: document
                .frontmatter
                .scope
                .unwrap_or_else(|| "app".to_string()),
            audit: document.frontmatter.audit.unwrap_or(true),
            danger_level: AdminDangerLevel::parse(document.frontmatter.danger_level.as_deref()),
            source_anchor,
        };
        resources.push(AdminEntryProjection {
            registry_entry,
            page_program,
            page_structure_digest,
            artifact_refs: AdminArtifactRefs::default(),
        });
    }
    let admin_registry_digest = digest_serializable(
        &resources
            .iter()
            .map(|resource| &resource.registry_entry)
            .collect::<Vec<_>>(),
    );
    let page_structure_digest = digest_serializable(
        &resources
            .iter()
            .map(|resource| resource.page_structure_digest.as_str())
            .collect::<Vec<_>>(),
    );
    AdminDiscoverOutcome::Ok(AdminRegistryProjection {
        app_id: app_id.to_string(),
        api_version: ADMIN_RESOURCE_API_VERSION.to_string(),
        admin_registry_digest,
        page_structure_digest,
        resources,
    })
}

fn scene_dependency_digest(app_root: &Path, scene_path: &Path) -> Result<String, String> {
    let canonical_root = app_root
        .canonicalize()
        .map_err(|error| format!("resolve app root {}: {error}", app_root.display()))?;
    let mut pending = vec![scene_path.to_path_buf()];
    let mut visited = BTreeSet::new();
    let mut sources = Vec::new();
    while let Some(path) = pending.pop() {
        let canonical = path
            .canonicalize()
            .map_err(|error| format!("read scene dependency {}: {error}", path.display()))?;
        if !canonical.starts_with(&canonical_root) || !visited.insert(canonical.clone()) {
            continue;
        }
        let raw = fs::read_to_string(&canonical)
            .map_err(|error| format!("read scene dependency {}: {error}", canonical.display()))?;
        let anchor = canonical
            .strip_prefix(&canonical_root)
            .unwrap_or(canonical.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let parsed = parse_v2_source_file(&canonical)
            .map_err(|error| format!("parse scene dependency {anchor}: {error}"))?;
        for item in parsed.items {
            if let V2Item::UseTemplate { path, .. } = item {
                let candidate = resolve_dependency_path(app_root, &canonical, path.as_str());
                if candidate.is_file() {
                    pending.push(candidate);
                }
            }
        }
        sources.push((anchor, raw));
    }
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(digest_serializable(&sources))
}

fn resolve_dependency_path(app_root: &Path, source: &Path, reference: &str) -> PathBuf {
    let reference = Path::new(reference);
    if reference.is_absolute() {
        return reference.to_path_buf();
    }
    let relative = source.parent().unwrap_or(app_root).join(reference);
    if relative.is_file() {
        relative
    } else {
        app_root.join("src").join(reference)
    }
}

fn digest_serializable(value: &impl Serialize) -> String {
    let encoded = serde_json::to_vec(value).expect("Admin digest input must serialize");
    format!("sha256:{:x}", Sha256::digest(encoded))
}

fn valid_identity(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|character| character.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

pub fn filter_admin_resources_for_capabilities<'a>(
    resources: &'a [AdminEntryProjection],
    has_capability: &dyn Fn(&str) -> bool,
) -> Vec<&'a AdminEntryProjection> {
    resources
        .iter()
        .filter(|resource| {
            resource
                .registry_entry
                .required_capabilities
                .iter()
                .all(|capability| has_capability(capability))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_entry(root: &Path, governance: &str, prose: &str, scene: &str) {
        fs::create_dir_all(root.join("src/admin/organization")).unwrap();
        fs::create_dir_all(root.join("src/scene/admin/organization")).unwrap();
        fs::write(
            root.join("app.toml"),
            "schema_version = \"mei-app-v1\"\ntitle = \"Admin v2\"\n",
        )
        .unwrap();
        fs::write(
            root.join("src/admin/organization/overview.mdx"),
            format!(
                "---\napi_version: mei-admin-resource-v2\ntitle: 单位信息\nrequired_capabilities: [config_upload]\n{governance}---\n\n{prose}\n\n@scene(use=\"{scene}\")\n"
            ),
        )
        .unwrap();
        fs::write(
            root.join("src/scene/admin/organization/overview.mei"),
            "scene(id = \"admin.organization.overview\", profile = \"page\")\n",
        )
        .unwrap();
    }

    fn projection(root: &Path) -> AdminRegistryProjection {
        match discover_app_admin_resources(root, "demo") {
            AdminDiscoverOutcome::Ok(projection) => projection,
            other => panic!("expected projection, got {other:?}"),
        }
    }

    #[test]
    fn admin_v2_derives_identity_route_and_scene_relation() {
        let dir = tempfile::tempdir().unwrap();
        write_entry(
            dir.path(),
            "audit: true\n",
            "维护单位信息。",
            "admin.organization.overview",
        );
        let projection = projection(dir.path());
        let entry = &projection.resources[0];
        assert_eq!(entry.registry_entry.resource_id, "organization");
        assert_eq!(entry.registry_entry.module_id, "overview");
        assert_eq!(
            entry.registry_entry.canonical_route,
            "/admin/apps/demo/organization/overview"
        );
        assert_eq!(
            entry.page_program.root.scene_ref(),
            "admin.organization.overview"
        );
        assert_eq!(
            entry.page_program.visible_body.source_anchor,
            "src/admin/organization/overview.mdx"
        );
    }

    #[test]
    fn admin_digests_are_orthogonal() {
        let dir = tempfile::tempdir().unwrap();
        write_entry(
            dir.path(),
            "audit: true\n",
            "第一版帮助。",
            "admin.organization.overview",
        );
        let first = projection(dir.path());
        write_entry(
            dir.path(),
            "audit: true\n",
            "第二版帮助。",
            "admin.organization.overview",
        );
        let prose = projection(dir.path());
        assert_eq!(first.admin_registry_digest, prose.admin_registry_digest);
        assert_ne!(first.page_structure_digest, prose.page_structure_digest);
        write_entry(
            dir.path(),
            "audit: false\n",
            "第二版帮助。",
            "admin.organization.overview",
        );
        let governance = projection(dir.path());
        assert_ne!(
            prose.admin_registry_digest,
            governance.admin_registry_digest
        );
        assert_eq!(
            prose.page_structure_digest,
            governance.page_structure_digest
        );

        fs::write(
            dir.path()
                .join("src/scene/admin/organization/overview.mei"),
            "scene(id = \"admin.organization.overview\", profile = \"page\", summary = \"changed\")\n",
        )
        .unwrap();
        let scene_changed = projection(dir.path());
        assert_eq!(
            governance.admin_registry_digest,
            scene_changed.admin_registry_digest
        );
        assert_ne!(
            governance.page_structure_digest,
            scene_changed.page_structure_digest
        );
    }

    #[test]
    fn admin_rejects_invalid_path_non_mdx_and_unknown_root() {
        let invalid = tempfile::tempdir().unwrap();
        fs::create_dir_all(invalid.path().join("src/admin")).unwrap();
        fs::write(invalid.path().join("src/admin/legacy.admin.mdx"), "").unwrap();
        let error = discover_admin_mdx_paths(invalid.path()).unwrap_err();
        assert!(error.contains(ADMIN_SOURCE_PATH_INVALID));

        let polluted = tempfile::tempdir().unwrap();
        fs::create_dir_all(polluted.path().join("src/admin/organization")).unwrap();
        fs::write(polluted.path().join("src/admin/organization/page.mei"), "").unwrap();
        let error = discover_admin_mdx_paths(polluted.path()).unwrap_err();
        assert!(error.contains(ADMIN_ENTRY_MODULE_FORBIDDEN));

        let unknown = tempfile::tempdir().unwrap();
        write_entry(unknown.path(), "", "帮助。", "admin.organization.missing");
        match discover_app_admin_resources(unknown.path(), "demo") {
            AdminDiscoverOutcome::Err(diagnostic) => {
                assert_eq!(diagnostic.kind, ADMIN_SCENE_ROOT_UNKNOWN);
            }
            other => panic!("expected unknown root, got {other:?}"),
        }
    }

    #[test]
    fn admin_rejects_legacy_manifest_pointer_and_duplicate_scene_roots() {
        let legacy = tempfile::tempdir().unwrap();
        fs::write(
            legacy.path().join("app.toml"),
            "[admin]\nmanifest = \"admin/admin.toml\"\n",
        )
        .unwrap();
        match discover_app_admin_resources(legacy.path(), "demo") {
            AdminDiscoverOutcome::Err(diagnostic) => {
                assert_eq!(diagnostic.kind, ADMIN_LEGACY_MANIFEST_FORBIDDEN);
            }
            other => panic!("expected legacy manifest diagnostic, got {other:?}"),
        }

        let duplicate = tempfile::tempdir().unwrap();
        fs::create_dir_all(duplicate.path().join("src/scene/a")).unwrap();
        fs::write(
            duplicate.path().join("src/scene/a/one.mei"),
            "scene(id = \"admin.shared.root\", profile = \"page\")\n",
        )
        .unwrap();
        fs::write(
            duplicate.path().join("src/scene/a/two.mei"),
            "scene(id = \"admin.shared.root\", profile = \"page\")\n",
        )
        .unwrap();
        let error = discover_scene_root_catalog(duplicate.path()).unwrap_err();
        assert!(error.contains(ADMIN_SCENE_ROOT_DUPLICATE));
    }
}
