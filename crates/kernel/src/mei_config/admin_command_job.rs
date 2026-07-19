//! Typed command jobs persisted under active-generation `var/admin/jobs`.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::admin_asset_slot::replace_asset_slot;
use super::admin_record::{
    admin_var_root, append_admin_audit, now_ms, AdminAuditEntry, AdminRecordError,
};
use super::admin_registry::ProviderBinding;
use super::io::write_string_atomically;

pub const ADMIN_JOBS_REL_DIR: &str = "var/admin/jobs";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AdminJobStatus {
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
    pub module_id: String,
    pub provider_id: String,
    pub action: String,
    pub status: AdminJobStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub created_ms: u64,
    pub updated_ms: u64,
}

fn job_path(app_root: &Path, job_id: &str) -> Result<std::path::PathBuf, AdminRecordError> {
    Ok(admin_var_root(app_root)?
        .join("jobs")
        .join(format!("{job_id}.json")))
}

pub fn get_command_job(app_root: &Path, job_id: &str) -> Result<AdminJobRecord, AdminRecordError> {
    if job_id.is_empty()
        || !job_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(AdminRecordError::Validation("invalid job_id".to_string()));
    }
    let path = job_path(app_root, job_id)?;
    if !path.is_file() {
        return Err(AdminRecordError::NotFound(format!(
            "job `{job_id}` not found"
        )));
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| AdminRecordError::Io(format!("{}: {error}", path.display())))?;
    serde_json::from_str(&raw).map_err(|error| AdminRecordError::Parse(error.to_string()))
}

fn write_job(app_root: &Path, job: &AdminJobRecord) -> Result<(), AdminRecordError> {
    let path = job_path(app_root, job.job_id.as_str())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| AdminRecordError::Io(format!("{}: {error}", parent.display())))?;
    }
    let raw = serde_json::to_string_pretty(job)
        .map_err(|error| AdminRecordError::Parse(error.to_string()))?;
    write_string_atomically(&path, raw.as_str())
        .map_err(|error| AdminRecordError::Io(error.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub fn run_import_job(
    app_root: &Path,
    command_binding: &ProviderBinding,
    asset_binding: &ProviderBinding,
    app_id: &str,
    resource_id: &str,
    module_id: &str,
    filename: &str,
    bytes: &[u8],
    actor: &str,
    correlation_id: &str,
) -> Result<AdminJobRecord, AdminRecordError> {
    if command_binding.provider_id != "command-job" || command_binding.method != "POST" {
        return Err(AdminRecordError::Validation(
            "binding is not a command-job POST".to_string(),
        ));
    }
    let ts = now_ms();
    let job_id = format!(
        "job-{ts}-{}",
        correlation_id.chars().take(8).collect::<String>()
    );
    let mut job = AdminJobRecord {
        job_id,
        app_id: app_id.to_string(),
        resource_id: resource_id.to_string(),
        module_id: module_id.to_string(),
        provider_id: command_binding.provider_id.clone(),
        action: command_binding.target.clone(),
        status: AdminJobStatus::Running,
        message: None,
        created_ms: ts,
        updated_ms: ts,
    };
    write_job(app_root, &job)?;
    match replace_asset_slot(
        app_root,
        asset_binding,
        filename,
        bytes,
        actor,
        app_id,
        resource_id,
        module_id,
        correlation_id,
    ) {
        Ok(_) => {
            job.status = AdminJobStatus::Succeeded;
            job.message = Some("asset imported".to_string());
            job.updated_ms = now_ms();
            write_job(app_root, &job)?;
            append_admin_audit(
                app_root,
                &AdminAuditEntry {
                    actor: actor.to_string(),
                    app_id: app_id.to_string(),
                    resource_id: resource_id.to_string(),
                    module_id: module_id.to_string(),
                    provider_id: command_binding.provider_id.clone(),
                    method: command_binding.method.clone(),
                    target: command_binding.target.clone(),
                    apply_policy: command_binding.apply_policy,
                    danger: command_binding.danger,
                    action: "run".to_string(),
                    before_revision: None,
                    after_revision: None,
                    result: "success".to_string(),
                    correlation_id: correlation_id.to_string(),
                    at_ms: now_ms(),
                },
            )?;
            Ok(job)
        }
        Err(error) => {
            job.status = AdminJobStatus::Failed;
            job.message = Some(error.to_string());
            job.updated_ms = now_ms();
            let _ = write_job(app_root, &job);
            Err(error)
        }
    }
}
