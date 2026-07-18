//! Lightweight command-job for Admin Platform (0549).
//!
//! Sync import: validate → replace asset slot → persist job record.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::admin_asset_slot::replace_asset_slot;
use super::admin_manifest::AdminResourceSpec;
use super::admin_record::{append_admin_audit, AdminAuditEntry, AdminRecordError};
use super::io::write_string_atomically;

pub const ADMIN_JOBS_REL_DIR: &str = "admin/jobs";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AdminJobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminJobRecord {
    pub job_id: String,
    pub app_id: String,
    pub resource_id: String,
    pub action: String,
    pub slot_id: Option<String>,
    pub status: AdminJobStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub created_ms: u64,
    pub updated_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn job_path(app_root: &Path, job_id: &str) -> std::path::PathBuf {
    app_root.join(ADMIN_JOBS_REL_DIR).join(format!("{job_id}.json"))
}

pub fn get_command_job(app_root: &Path, job_id: &str) -> Result<AdminJobRecord, AdminRecordError> {
    let path = job_path(app_root, job_id);
    if !path.is_file() {
        return Err(AdminRecordError::NotFound(format!("job `{job_id}` not found")));
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| AdminRecordError::Io(format!("{}: {e}", path.display())))?;
    serde_json::from_str(&raw).map_err(|e| AdminRecordError::Parse(e.to_string()))
}

fn write_job(app_root: &Path, job: &AdminJobRecord) -> Result<(), AdminRecordError> {
    let path = job_path(app_root, &job.job_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AdminRecordError::Io(format!("{}: {e}", parent.display())))?;
    }
    let raw = serde_json::to_string_pretty(job)
        .map_err(|e| AdminRecordError::Parse(e.to_string()))?;
    write_string_atomically(&path, &raw).map_err(|e| AdminRecordError::Io(e.to_string()))
}

/// Run import action synchronously: replace slot then mark job succeeded/failed.
pub fn run_import_job(
    app_root: &Path,
    resource: &AdminResourceSpec,
    app_id: &str,
    slot_id: &str,
    filename: &str,
    bytes: &[u8],
    actor: &str,
    correlation_id: &str,
) -> Result<AdminJobRecord, AdminRecordError> {
    let ts = now_ms();
    let job_id = format!("job-{ts}-{}", &correlation_id.chars().take(8).collect::<String>());
    let mut job = AdminJobRecord {
        job_id: job_id.clone(),
        app_id: app_id.to_string(),
        resource_id: resource.resource_id.clone(),
        action: "import".into(),
        slot_id: Some(slot_id.to_string()),
        status: AdminJobStatus::Running,
        message: None,
        created_ms: ts,
        updated_ms: ts,
    };
    write_job(app_root, &job)?;

    match replace_asset_slot(
        app_root,
        resource,
        slot_id,
        filename,
        bytes,
        actor,
        app_id,
        correlation_id,
    ) {
        Ok(_) => {
            job.status = AdminJobStatus::Succeeded;
            job.message = Some(format!("replaced slot `{slot_id}`"));
            job.updated_ms = now_ms();
            write_job(app_root, &job)?;
            let _ = append_admin_audit(
                app_root,
                &AdminAuditEntry {
                    actor: actor.to_string(),
                    scope: "app".to_string(),
                    app_id: app_id.to_string(),
                    resource_id: resource.resource_id.clone(),
                    action: format!("import:{slot_id}"),
                    provider: "command-job".to_string(),
                    before_revision: None,
                    after_revision: None,
                    result: "success".to_string(),
                    correlation_id: correlation_id.to_string(),
                    at_ms: now_ms(),
                },
            );
            Ok(job)
        }
        Err(e) => {
            job.status = AdminJobStatus::Failed;
            job.message = Some(e.to_string());
            job.updated_ms = now_ms();
            let _ = write_job(app_root, &job);
            let _ = append_admin_audit(
                app_root,
                &AdminAuditEntry {
                    actor: actor.to_string(),
                    scope: "app".to_string(),
                    app_id: app_id.to_string(),
                    resource_id: resource.resource_id.clone(),
                    action: format!("import:{slot_id}"),
                    provider: "command-job".to_string(),
                    before_revision: None,
                    after_revision: None,
                    result: "failed".to_string(),
                    correlation_id: correlation_id.to_string(),
                    at_ms: now_ms(),
                },
            );
            Err(e)
        }
    }
}
