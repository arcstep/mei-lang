//! Admin manifest / AdminResourceSpec v1 (0545).
//!
//! Authoring: TOML. Runtime: in-memory IR + static validation.
//! Does not implement Resource Registry, providers, or Admin UI.

use std::fs;
use std::path::{Component, Path, PathBuf};

use mei_syntax::{AdminMdxBlock, AdminMdxDocument};
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
    /// Dotted path inside `app.toml` for canonical app configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
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
    pub apply_policy: Option<AdminApplyPolicy>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdminApplyPolicy {
    Hot,
    ReloadView,
    RestartRuntime,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_path: Option<String>,
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

/// Parse one convention-discovered `src/admin/**/*.admin.mdx` resource and
/// lower its shallow Page/Form declarations into the existing governance IR.
pub fn load_admin_mdx_resource(path: &Path) -> Result<AdminResourceSpec, AdminManifestError> {
    let document = mei_syntax::parse_admin_mdx_file(path)
        .map_err(|error| AdminManifestError::Parse(error.to_string()))?;
    lower_admin_mdx_document(&document)
}

pub fn lower_admin_mdx_document(
    document: &AdminMdxDocument,
) -> Result<AdminResourceSpec, AdminManifestError> {
    let frontmatter = &document.frontmatter;
    let mut sections = Vec::new();
    let mut columns = Vec::new();
    let mut upload = None;
    let mut actions = Vec::new();
    for block in &document.blocks {
        match block {
            AdminMdxBlock::Section { id, title, fields } => {
                sections.push(AdminSection {
                    id: id.clone(),
                    title: title.clone(),
                    fields: fields
                        .iter()
                        .map(|field| {
                            Ok(AdminField {
                                id: field.id.clone(),
                                value_path: field.path.clone(),
                                label: field.label.clone(),
                                control: parse_field_control(&field.control)?,
                                required: field.required,
                                description: field.description.clone(),
                                readonly: field.readonly,
                                options: field
                                    .options
                                    .iter()
                                    .map(|option| AdminFieldOption {
                                        value: option.value.clone(),
                                        label: option.label.clone(),
                                    })
                                    .collect(),
                            })
                        })
                        .collect::<Result<Vec<_>, AdminManifestError>>()?,
                });
            }
            AdminMdxBlock::Column(column) => columns.push(AdminColumn {
                id: column.id.clone(),
                label: column.label.clone(),
                control: column
                    .control
                    .as_deref()
                    .map(parse_field_control)
                    .transpose()?,
            }),
            AdminMdxBlock::Upload(value) => {
                if upload.is_some() {
                    return Err(AdminManifestError::Validation(format!(
                        "resource {:?} declares more than one @upload",
                        frontmatter.resource_id
                    )));
                }
                upload = Some(AdminUploadSpec {
                    accept: value.accept.clone(),
                    max_bytes: value.max_bytes,
                    replace_modes: value.replace_modes.clone(),
                    retain_versions: value.retain_versions,
                    schema_ref: value.schema_ref.clone(),
                    requires_review: value.requires_review,
                });
            }
            AdminMdxBlock::Action(action) => actions.push(AdminAction {
                id: action.id.clone(),
                label: action.label.clone(),
                provider: parse_provider(&action.provider)?,
                method: action.method.clone(),
                danger_level: action
                    .danger_level
                    .as_deref()
                    .map(parse_danger_level)
                    .transpose()?,
            }),
            AdminMdxBlock::Markdown { .. } | AdminMdxBlock::Readonly(_) => {}
        }
    }

    let navigation = if frontmatter.navigation_menu.is_some()
        || frontmatter.navigation_parent.is_some()
        || frontmatter.navigation_order.is_some()
        || !frontmatter.navigation_keywords.is_empty()
    {
        Some(AdminNavigation {
            menu: frontmatter
                .navigation_menu
                .clone()
                .map(AdminMenuValue::Label),
            parent: frontmatter.navigation_parent.clone(),
            order: frontmatter.navigation_order,
            keywords: frontmatter.navigation_keywords.clone(),
        })
    } else {
        None
    };

    let resource = AdminResourceSpec {
        resource_id: frontmatter.resource_id.clone(),
        title: frontmatter.title.clone(),
        description: frontmatter.description.clone(),
        namespace: None,
        template: parse_template(&frontmatter.template)?,
        provider: parse_provider(&frontmatter.provider)?,
        record_path: frontmatter.record_path.clone(),
        config_path: frontmatter.config_path.clone(),
        required_capabilities: frontmatter.required_capabilities.clone(),
        scope: frontmatter.scope.clone(),
        audit: frontmatter.audit,
        danger_level: frontmatter
            .danger_level
            .as_deref()
            .map(parse_danger_level)
            .transpose()?,
        revision_policy: frontmatter
            .revision_policy
            .as_deref()
            .map(parse_revision_policy)
            .transpose()?,
        validation: None,
        idempotency: None,
        dirty_policy: frontmatter
            .dirty_policy
            .as_deref()
            .map(parse_dirty_policy)
            .transpose()?,
        apply_policy: frontmatter
            .apply_policy
            .as_deref()
            .map(parse_apply_policy)
            .transpose()?,
        navigation,
        sections,
        columns,
        allowed_views: Vec::new(),
        upload,
        actions,
        query: None,
        get: None,
        mutation: None,
    };
    validate_resource(&resource)?;
    Ok(resource)
}

/// Render the canonical shallow Admin MDX form used by migration tooling.
pub fn render_admin_resource_mdx(resource: &AdminResourceSpec) -> String {
    let mut lines = vec![
        "---".to_string(),
        format!("resource_id: {}", quote_mdx(&resource.resource_id)),
        format!("title: {}", quote_mdx(&resource.title)),
    ];
    push_frontmatter(&mut lines, "description", resource.description.as_deref());
    lines.push(format!(
        "template: {}",
        admin_template_name(resource.template)
    ));
    lines.push(format!(
        "provider: {}",
        admin_provider_name(resource.provider)
    ));
    push_frontmatter(&mut lines, "record_path", resource.record_path.as_deref());
    push_frontmatter(&mut lines, "config_path", resource.config_path.as_deref());
    lines.push(format!(
        "required_capabilities: [{}]",
        resource.required_capabilities.join(", ")
    ));
    push_frontmatter(&mut lines, "scope", resource.scope.as_deref());
    if let Some(value) = resource.audit {
        lines.push(format!("audit: {value}"));
    }
    if let Some(value) = resource.danger_level {
        lines.push(format!("danger_level: {}", danger_level_name(value)));
    }
    if let Some(value) = resource.revision_policy {
        lines.push(format!("revision_policy: {}", revision_policy_name(value)));
    }
    if let Some(value) = resource.dirty_policy {
        lines.push(format!("dirty_policy: {}", dirty_policy_name(value)));
    }
    if let Some(value) = resource.apply_policy {
        lines.push(format!("apply_policy: {}", apply_policy_name(value)));
    }
    if let Some(navigation) = &resource.navigation {
        if let Some(AdminMenuValue::Label(label)) = &navigation.menu {
            lines.push(format!("navigation_menu: {}", quote_mdx(label)));
        }
        push_frontmatter(
            &mut lines,
            "navigation_parent",
            navigation.parent.as_deref(),
        );
        if let Some(order) = navigation.order {
            lines.push(format!("navigation_order: {order}"));
        }
        if !navigation.keywords.is_empty() {
            lines.push(format!(
                "navigation_keywords: [{}]",
                navigation
                    .keywords
                    .iter()
                    .map(|value| quote_mdx(value))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    lines.push("---".to_string());
    if let Some(description) = resource.description.as_deref() {
        lines.extend(["".to_string(), description.to_string()]);
    }
    for section in &resource.sections {
        lines.extend([
            "".to_string(),
            format!("## {} {{#{}}}", section.title, section.id),
        ]);
        for field in &section.fields {
            let mut args = vec![
                format!("id={}", quote_mdx(&field.id)),
                format!("label={}", quote_mdx(&field.label)),
                format!("control={}", quote_mdx(field_control_name(field.control))),
            ];
            if let Some(path) = field.value_path.as_deref() {
                args.push(format!("path={}", quote_mdx(path)));
            }
            if let Some(value) = field.required {
                args.push(format!("required={value}"));
            }
            if let Some(value) = field.readonly {
                args.push(format!("readonly={value}"));
            }
            if let Some(description) = field.description.as_deref() {
                args.push(format!("description={}", quote_mdx(description)));
            }
            if !field.options.is_empty() {
                let options = field
                    .options
                    .iter()
                    .map(|option| format!("{}={}", option.value, option.label))
                    .collect::<Vec<_>>()
                    .join("|");
                args.push(format!("options={}", quote_mdx(&options)));
            }
            lines.push(format!("@field({})", args.join(", ")));
        }
    }
    for column in &resource.columns {
        let mut args = vec![
            format!("id={}", quote_mdx(&column.id)),
            format!("label={}", quote_mdx(&column.label)),
        ];
        if let Some(control) = column.control {
            args.push(format!(
                "control={}",
                quote_mdx(field_control_name(control))
            ));
        }
        lines.extend(["".to_string(), format!("@column({})", args.join(", "))]);
    }
    if let Some(upload) = &resource.upload {
        let mut args = Vec::new();
        if !upload.accept.is_empty() {
            args.push(format!("accept={}", quote_mdx(&upload.accept.join("|"))));
        }
        if let Some(value) = upload.max_bytes {
            args.push(format!("max_bytes={value}"));
        }
        if !upload.replace_modes.is_empty() {
            args.push(format!(
                "replace_modes={}",
                quote_mdx(&upload.replace_modes.join("|"))
            ));
        }
        if let Some(value) = upload.retain_versions {
            args.push(format!("retain_versions={value}"));
        }
        if let Some(value) = upload.schema_ref.as_deref() {
            args.push(format!("schema_ref={}", quote_mdx(value)));
        }
        if let Some(value) = upload.requires_review {
            args.push(format!("requires_review={value}"));
        }
        lines.extend(["".to_string(), format!("@upload({})", args.join(", "))]);
    }
    for action in &resource.actions {
        let mut args = vec![
            format!("id={}", quote_mdx(&action.id)),
            format!("label={}", quote_mdx(&action.label)),
            format!(
                "provider={}",
                quote_mdx(admin_provider_name(action.provider))
            ),
            format!("method={}", quote_mdx(&action.method)),
        ];
        if let Some(value) = action.danger_level {
            args.push(format!(
                "danger_level={}",
                quote_mdx(danger_level_name(value))
            ));
        }
        lines.extend(["".to_string(), format!("@action({})", args.join(", "))]);
    }
    lines.push(String::new());
    lines.join("\n")
}

fn push_frontmatter(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        lines.push(format!("{key}: {}", quote_mdx(value)));
    }
}

fn quote_mdx(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn admin_template_name(value: AdminTemplate) -> &'static str {
    match value {
        AdminTemplate::SingletonForm => "singleton-form",
        AdminTemplate::CollectionDetail => "collection-detail",
        AdminTemplate::AssetSlotCollection => "asset-slot-collection",
        AdminTemplate::ActionJobConsole => "action-job-console",
    }
}

fn admin_provider_name(value: AdminProviderKind) -> &'static str {
    match value {
        AdminProviderKind::ConfigRecord => "config-record",
        AdminProviderKind::CrudCollection => "crud-collection",
        AdminProviderKind::AssetSlot => "asset-slot",
        AdminProviderKind::CommandJob => "command-job",
    }
}

fn field_control_name(value: AdminFieldControl) -> &'static str {
    match value {
        AdminFieldControl::Text => "text",
        AdminFieldControl::Textarea => "textarea",
        AdminFieldControl::Number => "number",
        AdminFieldControl::Boolean => "boolean",
        AdminFieldControl::Select => "select",
        AdminFieldControl::Multiselect => "multiselect",
        AdminFieldControl::Datetime => "datetime",
        AdminFieldControl::Json => "json",
    }
}

fn danger_level_name(value: AdminDangerLevel) -> &'static str {
    match value {
        AdminDangerLevel::Normal => "normal",
        AdminDangerLevel::Elevated => "elevated",
        AdminDangerLevel::Critical => "critical",
    }
}

fn revision_policy_name(value: AdminRevisionPolicy) -> &'static str {
    match value {
        AdminRevisionPolicy::None => "none",
        AdminRevisionPolicy::Optimistic => "optimistic",
    }
}

fn dirty_policy_name(value: AdminDirtyPolicy) -> &'static str {
    match value {
        AdminDirtyPolicy::BlockLeave => "block-leave",
        AdminDirtyPolicy::Warn => "warn",
        AdminDirtyPolicy::None => "none",
    }
}

fn apply_policy_name(value: AdminApplyPolicy) -> &'static str {
    match value {
        AdminApplyPolicy::Hot => "hot",
        AdminApplyPolicy::ReloadView => "reload-view",
        AdminApplyPolicy::RestartRuntime => "restart-runtime",
    }
}

fn parse_template(value: &str) -> Result<AdminTemplate, AdminManifestError> {
    match value {
        "singleton-form" => Ok(AdminTemplate::SingletonForm),
        "collection-detail" => Ok(AdminTemplate::CollectionDetail),
        "asset-slot-collection" => Ok(AdminTemplate::AssetSlotCollection),
        "action-job-console" => Ok(AdminTemplate::ActionJobConsole),
        _ => Err(AdminManifestError::Validation(format!(
            "unknown admin template {value:?}"
        ))),
    }
}

fn parse_provider(value: &str) -> Result<AdminProviderKind, AdminManifestError> {
    match value {
        "config-record" => Ok(AdminProviderKind::ConfigRecord),
        "crud-collection" => Ok(AdminProviderKind::CrudCollection),
        "asset-slot" => Ok(AdminProviderKind::AssetSlot),
        "command-job" => Ok(AdminProviderKind::CommandJob),
        _ => Err(AdminManifestError::Validation(format!(
            "unknown admin provider {value:?}"
        ))),
    }
}

fn parse_field_control(value: &str) -> Result<AdminFieldControl, AdminManifestError> {
    match value {
        "text" => Ok(AdminFieldControl::Text),
        "textarea" => Ok(AdminFieldControl::Textarea),
        "number" => Ok(AdminFieldControl::Number),
        "boolean" => Ok(AdminFieldControl::Boolean),
        "select" => Ok(AdminFieldControl::Select),
        "multiselect" => Ok(AdminFieldControl::Multiselect),
        "datetime" => Ok(AdminFieldControl::Datetime),
        "json" => Ok(AdminFieldControl::Json),
        _ => Err(AdminManifestError::Validation(format!(
            "unknown admin field control {value:?}"
        ))),
    }
}

fn parse_danger_level(value: &str) -> Result<AdminDangerLevel, AdminManifestError> {
    match value {
        "normal" => Ok(AdminDangerLevel::Normal),
        "elevated" => Ok(AdminDangerLevel::Elevated),
        "critical" => Ok(AdminDangerLevel::Critical),
        _ => Err(AdminManifestError::Validation(format!(
            "unknown admin danger_level {value:?}"
        ))),
    }
}

fn parse_revision_policy(value: &str) -> Result<AdminRevisionPolicy, AdminManifestError> {
    match value {
        "none" => Ok(AdminRevisionPolicy::None),
        "optimistic" => Ok(AdminRevisionPolicy::Optimistic),
        _ => Err(AdminManifestError::Validation(format!(
            "unknown admin revision_policy {value:?}"
        ))),
    }
}

fn parse_dirty_policy(value: &str) -> Result<AdminDirtyPolicy, AdminManifestError> {
    match value {
        "block-leave" => Ok(AdminDirtyPolicy::BlockLeave),
        "warn" => Ok(AdminDirtyPolicy::Warn),
        "none" => Ok(AdminDirtyPolicy::None),
        _ => Err(AdminManifestError::Validation(format!(
            "unknown admin dirty_policy {value:?}"
        ))),
    }
}

fn parse_apply_policy(value: &str) -> Result<AdminApplyPolicy, AdminManifestError> {
    match value {
        "hot" => Ok(AdminApplyPolicy::Hot),
        "reload-view" => Ok(AdminApplyPolicy::ReloadView),
        "restart-runtime" => Ok(AdminApplyPolicy::RestartRuntime),
        _ => Err(AdminManifestError::Validation(format!(
            "unknown admin apply_policy {value:?}"
        ))),
    }
}

/// Resolve `[admin].manifest` relative to app root; `None` if unset.
pub fn resolve_admin_manifest_path(
    app_root: &Path,
    admin_ref: &AppAdminRef,
) -> Result<Option<PathBuf>, AdminManifestError> {
    let Some(rel) = admin_ref
        .manifest
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    else {
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
            if let Some(parent) = nav
                .parent
                .as_ref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
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
        validate_relative_sandbox_path(
            path,
            &format!("resource {}.record_path", resource.resource_id),
        )?;
    }
    if let Some(path) = &resource.config_path {
        validate_config_path(
            path,
            &format!("resource {}.config_path", resource.resource_id),
        )?;
    }
    if resource.record_path.is_some() && resource.config_path.is_some() {
        return Err(AdminManifestError::Validation(format!(
            "resource {:?} must declare only one of record_path or config_path",
            resource.resource_id
        )));
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
            if let Some(path) = &field.value_path {
                validate_value_path(path, "field.value_path")?;
            }
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
                    field.id,
                    format!("{:?}", field.control).to_ascii_lowercase()
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
            if resource.provider == AdminProviderKind::ConfigRecord
                && resource.record_path.is_none()
                && resource.config_path.is_none()
            {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} config-record singleton-form requires record_path or config_path"
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
            if resource.config_path.is_some() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} collection-detail must not declare config_path"
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
            if resource.config_path.is_some() {
                return Err(AdminManifestError::Validation(format!(
                    "resource {id:?} asset-slot-collection must not declare config_path"
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
        && id.chars().enumerate().all(|(i, c)| match (i, c) {
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

fn validate_config_path(path: &str, field: &str) -> Result<(), AdminManifestError> {
    let segments = path.trim().split('.').collect::<Vec<_>>();
    if segments.is_empty()
        || segments
            .iter()
            .any(|segment| segment.is_empty() || !validate_config_segment(segment))
    {
        return Err(AdminManifestError::Validation(format!(
            "{field} {path:?} must be a dotted identifier path"
        )));
    }
    if segments.first().copied() != Some("ops") {
        return Err(AdminManifestError::Validation(format!(
            "{field} {path:?} must stay under ops.*"
        )));
    }
    Ok(())
}

fn validate_value_path(path: &str, field: &str) -> Result<(), AdminManifestError> {
    if path.trim().split('.').any(|segment| {
        segment.is_empty()
            || !segment.chars().enumerate().all(|(index, ch)| match index {
                0 => ch.is_ascii_alphanumeric() || ch == '_',
                _ => ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'),
            })
    }) {
        return Err(AdminManifestError::Validation(format!(
            "{field} {path:?} must be a dotted field path"
        )));
    }
    Ok(())
}

fn validate_config_segment(segment: &str) -> bool {
    segment.chars().enumerate().all(|(index, ch)| match index {
        0 => ch.is_ascii_alphabetic() || ch == '_',
        _ => ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'),
    })
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

    #[test]
    fn renders_toml_resource_as_equivalent_admin_mdx() {
        let manifest =
            load_admin_manifest(&fixtures_root().join("valid/admin.toml")).expect("manifest");
        for resource in &manifest.resources {
            let mdx = render_admin_resource_mdx(resource);
            let document = mei_syntax::parse_admin_mdx_source(&mdx).expect("rendered mdx parses");
            let lowered = lower_admin_mdx_document(&document).expect("rendered mdx lowers");
            assert_eq!(lowered.resource_id, resource.resource_id);
            assert_eq!(lowered.template, resource.template);
            assert_eq!(lowered.provider, resource.provider);
            assert_eq!(lowered.sections.len(), resource.sections.len());
            assert_eq!(lowered.columns.len(), resource.columns.len());
            assert_eq!(lowered.upload.is_some(), resource.upload.is_some());
        }
    }
}
