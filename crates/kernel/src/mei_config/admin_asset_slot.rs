//! Typed asset-slot provider backed by active-generation `var/admin/uploads`.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::admin_record::{
    admin_var_root, append_admin_audit, now_ms, AdminAuditEntry, AdminRecordError,
};
use super::admin_registry::ProviderBinding;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotView {
    pub slot_id: String,
    pub title: String,
    pub path: String,
    pub kind: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_ms: Option<u64>,
}

fn safe_identity(value: &str) -> Result<&str, AdminRecordError> {
    if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        Ok(value)
    } else {
        Err(AdminRecordError::Validation(
            "asset slot identity is invalid".to_string(),
        ))
    }
}

fn slot_path(
    app_root: &Path,
    binding: &ProviderBinding,
) -> Result<std::path::PathBuf, AdminRecordError> {
    Ok(admin_var_root(app_root)?
        .join("uploads")
        .join(safe_identity(binding.provider_id.as_str())?)
        .join(safe_identity(binding.binding_id.as_str())?))
}

fn slot_view(
    app_root: &Path,
    binding: &ProviderBinding,
) -> Result<AssetSlotView, AdminRecordError> {
    let path = slot_path(app_root, binding)?;
    let metadata = fs::metadata(&path).ok();
    let modified_ms = metadata
        .as_ref()
        .and_then(|value| value.modified().ok())
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as u64);
    Ok(AssetSlotView {
        slot_id: binding.binding_id.clone(),
        title: binding.binding_id.clone(),
        path: path.to_string_lossy().to_string(),
        kind: binding.payload_type.name.clone(),
        status: if metadata.is_some() {
            "ready".to_string()
        } else {
            "missing".to_string()
        },
        size_bytes: metadata.map(|value| value.len()),
        modified_ms,
    })
}

pub fn list_asset_slots(
    app_root: &Path,
    bindings: &[ProviderBinding],
) -> Result<Vec<AssetSlotView>, AdminRecordError> {
    bindings
        .iter()
        .filter(|binding| binding.provider_id == "asset-slot")
        .map(|binding| slot_view(app_root, binding))
        .collect()
}

pub fn get_asset_slot(
    app_root: &Path,
    binding: &ProviderBinding,
) -> Result<AssetSlotView, AdminRecordError> {
    slot_view(app_root, binding)
}

#[allow(clippy::too_many_arguments)]
pub fn replace_asset_slot(
    app_root: &Path,
    binding: &ProviderBinding,
    filename: &str,
    bytes: &[u8],
    actor: &str,
    app_id: &str,
    resource_id: &str,
    module_id: &str,
    correlation_id: &str,
) -> Result<AssetSlotView, AdminRecordError> {
    if binding.provider_id != "asset-slot" || !matches!(binding.method.as_str(), "PUT" | "POST") {
        return Err(AdminRecordError::Validation(
            "binding is not a writable asset-slot".to_string(),
        ));
    }
    if bytes.is_empty() {
        return Err(AdminRecordError::Validation("file is empty".to_string()));
    }
    if Path::new(filename)
        .file_name()
        .and_then(|value| value.to_str())
        != Some(filename)
    {
        return Err(AdminRecordError::Validation(
            "filename must not contain a path".to_string(),
        ));
    }
    let path = slot_path(app_root, binding)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| AdminRecordError::Io(format!("{}: {error}", parent.display())))?;
    }
    let temp = path.with_extension(format!(
        "tmp-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    fs::write(&temp, bytes)
        .map_err(|error| AdminRecordError::Io(format!("{}: {error}", temp.display())))?;
    fs::rename(&temp, &path)
        .map_err(|error| AdminRecordError::Io(format!("{}: {error}", path.display())))?;
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
            action: format!("replace:{filename}"),
            before_revision: None,
            after_revision: None,
            result: "success".to_string(),
            correlation_id: correlation_id.to_string(),
            at_ms: now_ms(),
        },
    )?;
    slot_view(app_root, binding)
}
