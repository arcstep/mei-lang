//! Admin manifest / AdminResourceSpec v1 (0545).
//!
//! Authoring: TOML. Runtime: in-memory IR + static validation.
//! Does not implement Resource Registry, providers, or Admin UI.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const ADMIN_RESOURCE_API_VERSION: &str = "mei-admin-resource-v1";

const ALLOWED_CAPABILITIES: &[&str] = &["config_upload", "build_view", "access_view"];

/// Top-level `admin/admin.toml` document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminManifest {
    #[serde(alias = "apiVersion")]
    pub api_version: String,
    pub resources: Vec<AdminResourceSpec>,
}

/// Single admin resource declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminResourceSpec {
    pub resource_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub template: AdminTemplate,
    pub provider: AdminProviderKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_path: Option<String>,
    pub required_capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub danger_level: Option<AdminDangerLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_policy: Option<AdminRevisionPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency: Option<AdminIdempotency>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty_policy: Option<AdminDirtyPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigation: Option<AdminNavigation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<AdminSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<AdminColumn>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_views: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload: Option<AdminUploadSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<AdminAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub get: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminTemplate {
    SingletonForm,
    CollectionDetail,
    AssetSlotCollection,
    ActionJobConsole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminProviderKind {
    ConfigRecord,
    CrudCollection,
    AssetSlot,
    CommandJob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminDangerLevel {
    Normal,
    Elevated,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminRevisionPolicy {
    None,
    Optimistic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminIdempotency {
    RequiredOnWrite,
    Optional,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminDirtyPolicy {
    BlockLeave,
    Warn,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminNavigation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menu: Option<AdminMenuValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AdminMenuValue {
    Flag(bool),
    Label(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminSection {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<AdminField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminField {
    pub id: String,
    pub label: String,
    pub control: AdminFieldControl,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readonly: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<AdminFieldOption>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminFieldControl {
    Text,
    Textarea,
    Number,
    Boolean,
    Select,
    Multiselect,
    Datetime,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminFieldOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminColumn {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<AdminFieldControl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminUploadSpec {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accept: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replace_modes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_versions: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_review: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminAction {
    pub id: String,
    pub label: String,
    pub provider: AdminProviderKind,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub danger_level: Option<AdminDangerLevel>,
}

/// Optional `[admin]` block on `app.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppAdminRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<String>,
}

impl AppAdminRef {
    pub fn is_empty(&self) -> bool {
        self.manifest
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminManifestError {
    Io(String),
    Parse(String),
    UnsupportedApiVersion(String),
    Validation(String),
}

impl std::fmt::Display for AdminManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "admin manifest io: {msg}"),
            Self::Parse(msg) => write!(f, "admin manifest parse: {msg}"),
            Self::UnsupportedApiVersion(v) => {
                write!(
                    f,
                    "unsupported admin api_version {v:?}; expected {ADMIN_RESOURCE_API_VERSION}"
                )
            }
            Self::Validation(msg) => write!(f, "admin manifest validation: {msg}"),
        }
    }
}

impl std::error::Error for AdminManifestError {}

/// Parse TOML text into an [`AdminManifest`] without static validation.
pub fn parse_admin_manifest(toml_text: &str) -> Result<AdminManifest, AdminManifestError> {
    toml::from_str(toml_text).map_err(|e| AdminManifestError::Parse(e.to_string()))
}

/// Parse and enforce `api_version` + static validation rules.
pub fn parse_and_validate_admin_manifest(
    toml_text: &str,
) -> Result<AdminManifest, AdminManifestError> {
    let manifest = parse_admin_manifest(toml_text)?;
    validate_admin_manifest(&manifest)?;
    Ok(manifest)
}

pub fn load_admin_manifest(path: &Path) -> Result<AdminManifest, AdminManifestError> {
    let text = fs::read_to_string(path)
        .map_err(|e| AdminManifestError::Io(format!("{}: {e}", path.display())))?;
    parse_and_validate_admin_manifest(&text)
}

/// Resolve `[admin].manifest` relative to app root; `None` if unset.
pub fn resolve_admin_manifest_path(
    app_root: &Path,
    admin_ref: &AppAdminRef,
) -> Result<Option<PathBuf>, AdminManifestError> {
    let Some(rel) = admin_ref.manifest.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    validate_relative_sandbox_path(rel, "admin.manifest")?;
    Ok(Some(app_root.join(rel)))
}

pub fn validate_admin_manifest(manifest: &AdminManifest) -> Result<(), AdminManifestError> {
    if manifest.api_version.trim() != ADMIN_RESOURCE_API_VERSION {
        return Err(AdminManifestError::UnsupportedApiVersion(
            manifest.api_version.clone(),
        ));
    }
    if manifest.resources.is_empty() {
        return Err(AdminManifestError::Validation(
            "resources must contain at least one resource".into(),
        ));
    }

    let mut seen_ids = std::collections::HashSet::new();
    for resource in &manifest.resources {
        validate_resource(resource)?;
        if !seen_ids.insert(resource.resource_id.as_str()) {
            return Err(AdminManifestError::Validation(format!(
                "duplicate resource_id {:?}",
                resource.resource_id
            )));
        }
    }

    for resource in &manifest.resources {
        if let Some(nav) = &resource.navigation {
            if let Some(parent) = nav.parent.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                if !seen_ids.contains(parent) {
                    return Err(AdminManifestError::Validation(format!(
                        "resource {:?} navigation.parent {:?} is not in this manifest",
                        resource.resource_id, parent
                    )));
                }
            }
        }
    }

    Ok(())
}

fn validate_resource(resource: &AdminResourceSpec) -> Result<(), AdminManifestError> {
    validate_snake_id(&resource.resource_id, "resource_id")?;
    if resource.title.trim().is_empty() {
        return Err(AdminManifestError::Validation(format!(
            "resource {:?} title must be non-empty",
            resource.resource_id
        )));
    }

    if let Some(ns) = resource.namespace.as_ref().map(|s| s.trim()) {
        if ns != "app" {
            return Err(AdminManifestError::Validation(format!(
                "resource {:?} namespace {:?} is forbidden in app admin manifests (host.* is Host-native)",
                resource.resource_id, ns
            )));
        }
    }

    if let Some(scope) = resource.scope.as_ref().map(|s| s.trim()) {
        if scope != "app" {
            return Err(AdminManifestError::Validation(format!(
                "resource {:?} scope {:?} is not allowed; only \"app\"",
                resource.resource_id, scope
            )));
        }
    }

    if resource.required_capabilities.is_empty() {
        return Err(AdminManifestError::Validation(format!(
            "resource {:?} required_capabilities must be non-empty",
            resource.resource_id
        )));
    }
    for cap in &resource.required_capabilities {
        if !ALLOWED_CAPABILITIES.contains(&cap.as_str()) {
            return Err(AdminManifestError::Validation(format!(
                "resource {:?} unknown capability {:?}; allowed: {:?}",
                resource.resource_id, cap, ALLOWED_CAPABILITIES
            )));
        }
    }

    if let Some(path) = &resource.record_path {
        validate_relative_sandbox_path(path, &format!("resource {}.record_path", resource.resource_id))?;
    }
    if let Some(upload) = &resource.upload {
        if let Some(schema_ref) = &upload.schema_ref {
            validate_relative_sandbox_path(
                schema_ref,
                &format!("resource {}.upload.schema_ref", resource.resource_id),
            )?;
        }
        for mode in &upload.replace_modes {
            if mode != "hard" && mode != "soft" {
                return Err(AdminManifestError::Validation(format!(
                    "resource {:?} upload.replace_modes entry {:?} is invalid",
                    resource.resource_id, mode
                )));
            }
        }
    }
    if let Some(validation) = &resource.validation {
        validate_relative_sandbox_path(
            validation,
            &format!("resource {}.validation", resource.resource_id),
        )?;
    }

    for view in &resource.allowed_views {
        if !matches!(view.as_str(), "list" | "compact" | "gallery") {
            return Err(AdminManifestError::Validation(format!(
                "resource {:?} allowed_views entry {:?} is invalid",
                resource.resource_id, view
            )));
        }
    }

    for section in &resource.sections {
        validate_snake_id(&section.id, "section.id")?;
        for field in &section.fields {
            validate_snake_id(&field.id, "field.id")?;
            validate_field_control(field)?;
        }
    }
    for column in &resource.columns {
        validate_snake_id(&column.id, "column.id")?;
    }
    for action in &resource.actions {
        validate_snake_id(&action.id, "action.id")?;
        if action.method.trim().is_empty() {
            return Err(AdminManifestError::Validation(format!(
                "resource {:?} action {:?} method must be non-empty",
                resource.resource_id, action.id
            )));
        }
    }

    validate_template_matrix(resource)?;
    Ok(())
}

fn validate_field_control(field: &AdminField) -> Result<(), AdminManifestError> {
    match field.control {
        AdminFieldControl::Select | AdminFieldControl::Multiselect => {
            if field.options.is_empty() {
                return Err(AdminManifestError::Validation(format!(
                    "field {:?} with control {:?} requires options",
                    field.id, format!("{:?}", field.control).to_ascii_lowercase()
                )));
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_template_matrix(resource: &AdminResourceSpec) -> Result<(), AdminManifestError> {
    let id = &resource.resource_id;
    match resource.template {
        AdminTemplate::SingletonForm => {
            if resource.sections.is_empty() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} singleton-form requires sections"
                )));
            }
            if !resource.columns.is_empty() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} singleton-form must not declare columns"
                )));
            }
            if resource.upload.is_some() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} singleton-form must not declare upload"
                )));
            }
            if resource.allowed_views.iter().any(|v| v == "gallery") {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} singleton-form must not allow gallery view"
                )));
            }
            if resource.provider == AdminProviderKind::ConfigRecord && resource.record_path.is_none()
            {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} config-record singleton-form requires record_path"
                )));
            }
        }
        AdminTemplate::CollectionDetail => {
            if resource.columns.is_empty() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} collection-detail requires columns"
                )));
            }
            if resource.sections.is_empty() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} collection-detail requires sections for detail form"
                )));
            }
            if resource.upload.is_some() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} collection-detail must not declare upload"
                )));
            }
            if resource.record_path.is_some() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} collection-detail must not declare record_path"
                )));
            }
        }
        AdminTemplate::AssetSlotCollection => {
            if resource.upload.is_none() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} asset-slot-collection requires upload"
                )));
            }
            if !resource.sections.is_empty() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} asset-slot-collection must not use sections as primary editor"
                )));
            }
            if resource.record_path.is_some() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} asset-slot-collection must not declare record_path"
                )));
            }
        }
        AdminTemplate::ActionJobConsole => {
            if resource.actions.is_empty() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} action-job-console requires actions"
                )));
            }
            if !resource.sections.is_empty() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} action-job-console must not declare sections"
                )));
            }
            if resource.upload.is_some() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} action-job-console must not declare upload"
                )));
            }
            if resource.allowed_views.iter().any(|v| v == "gallery") {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} action-job-console must not allow gallery view"
                )));
            }
        }
    }
    Ok(())
}

fn validate_snake_id(id: &str, field: &str) -> Result<(), AdminManifestError> {
    let ok = id.len() <= 64
        && id
            .chars()
            .enumerate()
            .all(|(i, c)| match (i, c) {
                (0, c) => c.is_ascii_lowercase(),
                (_, c) => c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_',
            });
    if !ok {
        return Err(AdminManifestError::Validation(format!(
            "{field} {id:?} must match [a-z][a-z0-9_]{{0,63}}"
        )));
    }
    Ok(())
}

pub fn validate_relative_sandbox_path(path: &str, field: &str) -> Result<(), AdminManifestError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AdminManifestError::Validation(format!(
            "{field} must be non-empty"
        )));
    }
    let p = Path::new(trimmed);
    if p.is_absolute() {
        return Err(AdminManifestError::Validation(format!(
            "{field} must be a relative path (got absolute {trimmed:?})"
        )));
    }
    for component in p.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(AdminManifestError::Validation(format!(
                    "{field} must not contain '..' (got {trimmed:?})"
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(AdminManifestError::Validation(format!(
                    "{field} must be a relative sandbox path (got {trimmed:?})"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/admin-manifest")
    }

    #[test]
    fn valid_gold_manifest_parses_and_validates() {
        let path = fixtures_root().join("valid/admin.toml");
        let manifest = load_admin_manifest(&path).expect("valid admin.toml");
        assert_eq!(manifest.api_version, ADMIN_RESOURCE_API_VERSION);
        assert_eq!(manifest.resources.len(), 2);
        assert_eq!(manifest.resources[0].resource_id, "organization");
        assert_eq!(manifest.resources[0].template, AdminTemplate::SingletonForm);
        assert_eq!(
            manifest.resources[0].provider,
            AdminProviderKind::ConfigRecord
        );
        assert_eq!(manifest.resources[1].resource_id, "datasources");
        assert_eq!(
            manifest.resources[1].template,
            AdminTemplate::AssetSlotCollection
        );
    }

    #[test]
    fn rejects_bad_api_version() {
        let text = fs::read_to_string(fixtures_root().join("invalid/bad_api_version.toml"))
            .expect("fixture");
        let err = parse_and_validate_admin_manifest(&text).expect_err("api version");
        assert!(matches!(err, AdminManifestError::UnsupportedApiVersion(_)));
    }

    #[test]
    fn rejects_host_namespace() {
        let text = fs::read_to_string(fixtures_root().join("invalid/host_namespace.toml"))
            .expect("fixture");
        let err = parse_and_validate_admin_manifest(&text).expect_err("host ns");
        match err {
            AdminManifestError::Validation(msg) => {
                assert!(msg.contains("host"), "{msg}");
            }
            other => panic!("expected validation, got {other}"),
        }
    }

    #[test]
    fn rejects_unknown_provider_at_parse() {
        let text = fs::read_to_string(fixtures_root().join("invalid/unknown_provider.toml"))
            .expect("fixture");
        let err = parse_and_validate_admin_manifest(&text).expect_err("provider");
        assert!(matches!(err, AdminManifestError::Parse(_)), "{err}");
    }

    #[test]
    fn rejects_script_injection_unknown_fields() {
        let text = fs::read_to_string(fixtures_root().join("invalid/script_injection.toml"))
            .expect("fixture");
        let err = parse_and_validate_admin_manifest(&text).expect_err("script");
        assert!(matches!(err, AdminManifestError::Parse(_)), "{err}");
    }

    #[test]
    fn rejects_path_escape() {
        let text =
            fs::read_to_string(fixtures_root().join("invalid/path_escape.toml")).expect("fixture");
        let err = parse_and_validate_admin_manifest(&text).expect_err("path");
        match err {
            AdminManifestError::Validation(msg) => {
                assert!(msg.contains(".."), "{msg}");
            }
            other => panic!("expected validation, got {other}"),
        }
    }

    #[test]
    fn resolve_admin_manifest_path_rejects_escape() {
        let app_root = Path::new("/tmp/app");
        let bad = AppAdminRef {
            manifest: Some("../outside.toml".into()),
        };
        let err = resolve_admin_manifest_path(app_root, &bad).expect_err("escape");
        assert!(matches!(err, AdminManifestError::Validation(_)));
    }

    #[test]
    fn resolve_admin_manifest_path_ok() {
        let app_root = Path::new("/tmp/app");
        let good = AppAdminRef {
            manifest: Some("admin/admin.toml".into()),
        };
        let path = resolve_admin_manifest_path(app_root, &good)
            .expect("ok")
            .expect("some");
        assert_eq!(path, PathBuf::from("/tmp/app/admin/admin.toml"));
    }
}
