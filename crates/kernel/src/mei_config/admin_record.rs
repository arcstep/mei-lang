//! Typed Admin provider state stored under the active generation `var/admin`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::admin_registry::{AdminApplyPolicy, AdminDangerLevel, ProviderBinding};
use super::io::{write_mei_config, write_string_atomically};
use super::workspace_paths::app_mei_config_path;
use super::MeiConfig;

pub const ADMIN_AUDIT_REL_PATH: &str = "var/admin/audit.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRecordFile {
    pub revision: u64,
    #[serde(default)]
    pub data: Value,
}

impl Default for ConfigRecordFile {
    fn default() -> Self {
        Self {
            revision: 0,
            data: Value::Object(serde_json::Map::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAuditEntry {
    pub actor: String,
    pub app_id: String,
    pub resource_id: String,
    pub module_id: String,
    pub provider_id: String,
    pub method: String,
    pub target: String,
    pub apply_policy: AdminApplyPolicy,
    pub danger: AdminDangerLevel,
    pub action: String,
    pub before_revision: Option<u64>,
    pub after_revision: Option<u64>,
    pub result: String,
    pub correlation_id: String,
    pub at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminRecordError {
    Io(String),
    Parse(String),
    Validation(String),
    Conflict { current_revision: u64 },
    NotFound(String),
}

impl std::fmt::Display for AdminRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "admin record io: {msg}"),
            Self::Parse(msg) => write!(f, "admin record parse: {msg}"),
            Self::Validation(msg) => write!(f, "admin record validation: {msg}"),
            Self::Conflict { current_revision } => {
                write!(
                    f,
                    "admin record conflict: current revision {current_revision}"
                )
            }
            Self::NotFound(msg) => write!(f, "admin record not-found: {msg}"),
        }
    }
}

impl std::error::Error for AdminRecordError {}

pub fn admin_var_root(app_root: &Path) -> Result<PathBuf, AdminRecordError> {
    let active_env = app_root.join("env/current");
    if !active_env.is_dir() {
        return Err(AdminRecordError::NotFound(format!(
            "active app generation is missing: {}",
            active_env.display()
        )));
    }
    Ok(active_env.join("var/admin"))
}

fn safe_identity<'a>(value: &'a str, field: &str) -> Result<&'a str, AdminRecordError> {
    if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        Ok(value)
    } else {
        Err(AdminRecordError::Validation(format!(
            "{field} must be an identifier"
        )))
    }
}

fn provider_state_path(
    app_root: &Path,
    binding: &ProviderBinding,
) -> Result<PathBuf, AdminRecordError> {
    let provider_id = safe_identity(binding.provider_id.as_str(), "provider_id")?;
    let target_digest = format!("{:x}", sha2::Sha256::digest(binding.target.as_bytes()));
    Ok(admin_var_root(app_root)?
        .join("records")
        .join(provider_id)
        .join(format!("{target_digest}.json")))
}

pub fn get_config_record(
    app_root: &Path,
    binding: &ProviderBinding,
) -> Result<ConfigRecordFile, AdminRecordError> {
    let path = provider_state_path(app_root, binding)?;
    let mut record = if path.is_file() {
        let raw = fs::read_to_string(&path)
            .map_err(|error| AdminRecordError::Io(format!("{}: {error}", path.display())))?;
        serde_json::from_str(&raw).map_err(|error| AdminRecordError::Parse(error.to_string()))?
    } else {
        ConfigRecordFile::default()
    };
    if binding.target.starts_with("ops.") {
        record.data = read_ops_target(app_root, binding.target.as_str())?;
    }
    Ok(record)
}

#[allow(clippy::too_many_arguments)]
pub fn put_config_record(
    app_root: &Path,
    binding: &ProviderBinding,
    expected_revision: u64,
    payload: Value,
    actor: &str,
    app_id: &str,
    resource_id: &str,
    module_id: &str,
    correlation_id: &str,
) -> Result<ConfigRecordFile, AdminRecordError> {
    if !payload.is_object() {
        return Err(AdminRecordError::Validation(
            "payload must be a JSON object".to_string(),
        ));
    }
    let path = provider_state_path(app_root, binding)?;
    let current = get_config_record(app_root, binding)?;
    if binding.revision == "required" && current.revision != expected_revision {
        return Err(AdminRecordError::Conflict {
            current_revision: current.revision,
        });
    }
    let next = ConfigRecordFile {
        revision: current.revision.saturating_add(1),
        data: payload.clone(),
    };
    if binding.target.starts_with("ops.") {
        write_ops_target(app_root, binding.target.as_str(), payload)?;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| AdminRecordError::Io(format!("{}: {error}", parent.display())))?;
    }
    let raw = serde_json::to_string_pretty(&next)
        .map_err(|error| AdminRecordError::Parse(error.to_string()))?;
    write_string_atomically(&path, raw.as_str())
        .map_err(|error| AdminRecordError::Io(error.to_string()))?;
    append_admin_audit(
        app_root,
        &AdminAuditEntry {
            actor: actor.to_string(),
            app_id: app_id.to_string(),
            resource_id: resource_id.to_string(),
            module_id: module_id.to_string(),
            provider_id: binding.provider_id.clone(),
            method: binding.method.clone(),
            target: binding.target.clone(),
            apply_policy: binding.apply_policy,
            danger: binding.danger,
            action: "put".to_string(),
            before_revision: Some(current.revision),
            after_revision: Some(next.revision),
            result: "success".to_string(),
            correlation_id: correlation_id.to_string(),
            at_ms: now_ms(),
        },
    )?;
    Ok(next)
}

fn read_ops_target(app_root: &Path, target: &str) -> Result<Value, AdminRecordError> {
    let config = MeiConfig::load_or_default(&app_mei_config_path(app_root));
    let value = serde_json::to_value(config.ops)
        .map_err(|error| AdminRecordError::Parse(error.to_string()))?;
    let segments = ops_target_segments(target)?;
    Ok(segments
        .iter()
        .try_fold(&value, |current, segment| current.get(segment))
        .cloned()
        .unwrap_or_else(|| Value::Object(serde_json::Map::new())))
}

fn write_ops_target(app_root: &Path, target: &str, payload: Value) -> Result<(), AdminRecordError> {
    let config_path = app_mei_config_path(app_root);
    let mut config = MeiConfig::load_or_default(&config_path);
    let mut ops = serde_json::to_value(&config.ops)
        .map_err(|error| AdminRecordError::Parse(error.to_string()))?;
    let segments = ops_target_segments(target)?;
    let mut current = &mut ops;
    for segment in &segments[..segments.len().saturating_sub(1)] {
        let object = current.as_object_mut().ok_or_else(|| {
            AdminRecordError::Validation(format!("ops target `{target}` crosses a non-object"))
        })?;
        current = object
            .entry((*segment).to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
    }
    let leaf = segments.last().expect("validated non-empty ops target");
    let object = current.as_object_mut().ok_or_else(|| {
        AdminRecordError::Validation(format!("ops target `{target}` parent is not an object"))
    })?;
    object.insert((*leaf).to_string(), payload);
    config.ops = serde_json::from_value(ops)
        .map_err(|error| AdminRecordError::Validation(error.to_string()))?;
    write_mei_config(&config_path, &config).map_err(|error| AdminRecordError::Io(error.to_string()))
}

fn ops_target_segments(target: &str) -> Result<Vec<&str>, AdminRecordError> {
    let segments = target
        .strip_prefix("ops.")
        .into_iter()
        .flat_map(|suffix| suffix.split('.'))
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty()
        || segments.iter().any(|segment| {
            !segment.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
        })
    {
        return Err(AdminRecordError::Validation(format!(
            "invalid ops target `{target}`"
        )));
    }
    Ok(segments)
}

pub fn append_admin_audit(
    app_root: &Path,
    entry: &AdminAuditEntry,
) -> Result<(), AdminRecordError> {
    let path = admin_var_root(app_root)?.join("audit.jsonl");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| AdminRecordError::Io(format!("{}: {error}", parent.display())))?;
    }
    let line =
        serde_json::to_string(entry).map_err(|error| AdminRecordError::Parse(error.to_string()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| AdminRecordError::Io(format!("{}: {error}", path.display())))?;
    writeln!(file, "{line}")
        .map_err(|error| AdminRecordError::Io(format!("{}: {error}", path.display())))
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

use sha2::Digest;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mei_config::{ProviderPayloadType, ProviderValidator};

    fn binding() -> ProviderBinding {
        ProviderBinding {
            binding_id: "organization.put".to_string(),
            provider_id: "config-record".to_string(),
            method: "PUT".to_string(),
            target: "ops.params.organization".to_string(),
            payload_type: ProviderPayloadType {
                name: "object".to_string(),
                schema: Some("organization-v1".to_string()),
            },
            validator: Some(ProviderValidator {
                kind: "schema-ref".to_string(),
                reference: "organization-v1".to_string(),
            }),
            revision: "required".to_string(),
            idempotency: "required".to_string(),
            apply_policy: AdminApplyPolicy::Hot,
            danger: AdminDangerLevel::Normal,
            required_capabilities: vec!["config_upload".to_string()],
            source_anchor: "src/data/admin/organization.mei".to_string(),
        }
    }

    #[test]
    fn config_record_uses_ops_truth_and_active_generation_revision_store() {
        let root = tempfile::tempdir().unwrap();
        let binding = binding();
        assert!(matches!(
            get_config_record(root.path(), &binding),
            Err(AdminRecordError::NotFound(_))
        ));
        fs::create_dir_all(root.path().join("env/current/var")).unwrap();
        let payload = serde_json::json!({"name": "Mei"});
        let saved = put_config_record(
            root.path(),
            &binding,
            0,
            payload.clone(),
            "tester",
            "demo",
            "organization",
            "overview",
            "request-1",
        )
        .unwrap();
        assert_eq!(saved.revision, 1);
        assert_eq!(
            get_config_record(root.path(), &binding).unwrap().data,
            payload
        );
        assert!(root
            .path()
            .join("env/current/var/admin/records/config-record")
            .is_dir());
        assert!(app_mei_config_path(root.path()).is_file());
        assert!(!root.path().join("var/admin").exists());
    }
}
