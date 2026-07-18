//! config-record storage for Admin Platform (0547 / 0545).
//!
//! On-disk shape: `{ "revision": u64, "data": { ... } }` under a sandboxed relative path.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::admin_manifest::{validate_relative_sandbox_path, AdminManifestError};
use super::io::write_string_atomically;

pub const ADMIN_AUDIT_REL_PATH: &str = "admin/.mei-admin-audit.jsonl";

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
    pub scope: String,
    pub app_id: String,
    pub resource_id: String,
    pub action: String,
    pub provider: String,
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
                write!(f, "admin record conflict: current revision {current_revision}")
            }
            Self::NotFound(msg) => write!(f, "admin record not-found: {msg}"),
        }
    }
}

impl std::error::Error for AdminRecordError {}

impl From<AdminManifestError> for AdminRecordError {
    fn from(value: AdminManifestError) -> Self {
        match value {
            AdminManifestError::Validation(msg)
            | AdminManifestError::Parse(msg)
            | AdminManifestError::Io(msg)
            | AdminManifestError::UnsupportedApiVersion(msg) => Self::Validation(msg),
        }
    }
}

pub fn resolve_config_record_path(
    app_root: &Path,
    record_path: &str,
) -> Result<PathBuf, AdminRecordError> {
    validate_relative_sandbox_path(record_path, "record_path")?;
    Ok(app_root.join(record_path.trim()))
}

pub fn get_config_record(
    app_root: &Path,
    record_path: &str,
) -> Result<ConfigRecordFile, AdminRecordError> {
    let path = resolve_config_record_path(app_root, record_path)?;
    if !path.is_file() {
        return Ok(ConfigRecordFile::default());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| AdminRecordError::Io(format!("{}: {e}", path.display())))?;
    serde_json::from_str(&raw).map_err(|e| AdminRecordError::Parse(e.to_string()))
}

pub fn put_config_record(
    app_root: &Path,
    record_path: &str,
    expected_revision: u64,
    payload: Value,
    actor: &str,
    app_id: &str,
    resource_id: &str,
    correlation_id: &str,
) -> Result<ConfigRecordFile, AdminRecordError> {
    if !payload.is_object() {
        return Err(AdminRecordError::Validation(
            "payload must be a JSON object".into(),
        ));
    }
    let path = resolve_config_record_path(app_root, record_path)?;
    let current = get_config_record(app_root, record_path)?;
    if current.revision != expected_revision {
        return Err(AdminRecordError::Conflict {
            current_revision: current.revision,
        });
    }
    let next = ConfigRecordFile {
        revision: current.revision.saturating_add(1),
        data: payload,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AdminRecordError::Io(format!("{}: {e}", parent.display())))?;
    }
    let raw = serde_json::to_string_pretty(&next)
        .map_err(|e| AdminRecordError::Parse(e.to_string()))?;
    write_string_atomically(&path, &raw)
        .map_err(|e| AdminRecordError::Io(e.to_string()))?;

    let _ = append_admin_audit(
        app_root,
        &AdminAuditEntry {
            actor: actor.to_string(),
            scope: "app".to_string(),
            app_id: app_id.to_string(),
            resource_id: resource_id.to_string(),
            action: "put".to_string(),
            provider: "config-record".to_string(),
            before_revision: Some(current.revision),
            after_revision: Some(next.revision),
            result: "success".to_string(),
            correlation_id: correlation_id.to_string(),
            at_ms: now_ms(),
        },
    );

    Ok(next)
}

pub fn append_admin_audit(
    app_root: &Path,
    entry: &AdminAuditEntry,
) -> Result<(), AdminRecordError> {
    let path = app_root.join(ADMIN_AUDIT_REL_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AdminRecordError::Io(format!("{}: {e}", parent.display())))?;
    }
    let line = serde_json::to_string(entry).map_err(|e| AdminRecordError::Parse(e.to_string()))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| AdminRecordError::Io(format!("{}: {e}", path.display())))?;
    writeln!(file, "{line}")
        .map_err(|e| AdminRecordError::Io(format!("{}: {e}", path.display())))?;
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn get_missing_returns_revision_zero() {
        let dir = tempdir().unwrap();
        let record = get_config_record(dir.path(), "admin/data/organization.json").unwrap();
        assert_eq!(record.revision, 0);
        assert!(record.data.as_object().unwrap().is_empty());
    }

    #[test]
    fn put_round_trip_and_conflict() {
        let dir = tempdir().unwrap();
        let path = "admin/data/organization.json";
        let first = put_config_record(
            dir.path(),
            path,
            0,
            serde_json::json!({"name": "甲单位"}),
            "tester",
            "demo",
            "organization",
            "corr-1",
        )
        .unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(first.data["name"], "甲单位");

        let conflict = put_config_record(
            dir.path(),
            path,
            0,
            serde_json::json!({"name": "乙"}),
            "tester",
            "demo",
            "organization",
            "corr-2",
        )
        .unwrap_err();
        assert!(matches!(
            conflict,
            AdminRecordError::Conflict {
                current_revision: 1
            }
        ));

        let second = put_config_record(
            dir.path(),
            path,
            1,
            serde_json::json!({"name": "乙单位"}),
            "tester",
            "demo",
            "organization",
            "corr-3",
        )
        .unwrap();
        assert_eq!(second.revision, 2);

        let audit = fs::read_to_string(dir.path().join(ADMIN_AUDIT_REL_PATH)).unwrap();
        assert_eq!(audit.lines().count(), 2);
    }

    #[test]
    fn rejects_path_escape() {
        let dir = tempdir().unwrap();
        let err = get_config_record(dir.path(), "../outside.json").unwrap_err();
        assert!(matches!(err, AdminRecordError::Validation(_)));
    }
}
