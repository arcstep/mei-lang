//! Typed asset-slot provider backed by `upload/admin/{slot_id}/`.
//!
//! Current datasource = `app.toml [ops.sources.{slot}].path`.
//! Uploads append uniquely named files (date-prefixed); apply updates ops.sources.path.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::admin_record::{append_admin_audit, now_ms, AdminAuditEntry, AdminRecordError};
use super::admin_registry::ProviderBinding;
use super::app_manifest::{load_app_manifest, write_app_toml};

pub const ADMIN_UPLOAD_PREFIX: &str = "upload/admin";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotFileView {
    pub name: String,
    pub path: String,
    pub is_current: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotView {
    pub slot_id: String,
    pub title: String,
    /// Absolute-ish app-relative directory: `upload/admin/{slot}`.
    pub directory: String,
    /// Current ops.sources path when configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_path: Option<String>,
    pub kind: String,
    pub status: String,
    #[serde(default)]
    pub files: Vec<AssetSlotFileView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_ms: Option<u64>,
}

fn safe_slot_id(value: &str) -> Result<&str, AdminRecordError> {
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

/// Resolve slot id from binding target `upload/admin/{slot}` (preferred) or last path segment.
pub fn slot_id_from_binding(binding: &ProviderBinding) -> Result<String, AdminRecordError> {
    let target = binding.target.trim().trim_matches('/');
    let slot = if let Some(rest) = target.strip_prefix(ADMIN_UPLOAD_PREFIX) {
        rest.trim_matches('/')
    } else if let Some(rest) = target.strip_prefix("var/admin/uploads/") {
        // Legacy targets accepted during migration.
        rest.trim_matches('/')
    } else {
        target.rsplit('/').next().unwrap_or(target)
    };
    Ok(safe_slot_id(slot)?.to_string())
}

fn slot_dir(app_root: &Path, slot_id: &str) -> Result<PathBuf, AdminRecordError> {
    let id = safe_slot_id(slot_id)?;
    Ok(app_root.join(ADMIN_UPLOAD_PREFIX).join(id))
}

fn slot_rel_dir(slot_id: &str) -> Result<String, AdminRecordError> {
    Ok(format!("{ADMIN_UPLOAD_PREFIX}/{}", safe_slot_id(slot_id)?))
}

fn file_modified_ms(meta: &fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis() as u64)
}

fn sanitize_upload_basename(filename: &str) -> Result<String, AdminRecordError> {
    let base = Path::new(filename)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            AdminRecordError::Validation("filename must not contain a path".to_string())
        })?
        .trim();
    if base.is_empty() || base == "." || base == ".." {
        return Err(AdminRecordError::Validation(
            "filename is empty".to_string(),
        ));
    }
    if base.contains('/') || base.contains('\\') {
        return Err(AdminRecordError::Validation(
            "filename must not contain a path".to_string(),
        ));
    }
    Ok(base.to_string())
}

fn ops_source_path(app_root: &Path, slot_id: &str) -> Option<String> {
    let config = load_app_manifest(app_root).to_mei_config();
    config
        .ops
        .sources
        .get(slot_id)
        .map(|entry| entry.path.trim().trim_start_matches("./").to_string())
        .filter(|path| !path.is_empty())
}

fn ops_source_kind(app_root: &Path, slot_id: &str, fallback: &str) -> String {
    load_app_manifest(app_root)
        .to_mei_config()
        .ops
        .sources
        .get(slot_id)
        .map(|entry| entry.kind.clone())
        .filter(|kind| !kind.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn list_files_in_slot(
    app_root: &Path,
    slot_id: &str,
    active_path: Option<&str>,
) -> Result<Vec<AssetSlotFileView>, AdminRecordError> {
    let dir = slot_dir(app_root, slot_id)?;
    let rel_dir = slot_rel_dir(slot_id)?;
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    let entries = fs::read_dir(&dir)
        .map_err(|error| AdminRecordError::Io(format!("{}: {error}", dir.display())))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| AdminRecordError::Io(format!("{}: {error}", dir.display())))?;
        let meta = entry
            .metadata()
            .map_err(|error| AdminRecordError::Io(format!("{}: {error}", dir.display())))?;
        if !meta.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let rel_path = format!("{rel_dir}/{name}");
        let is_current = active_path.is_some_and(|active| {
            active == rel_path.as_str()
                || Path::new(active).file_name().and_then(|v| v.to_str()) == Some(name.as_str())
        });
        files.push(AssetSlotFileView {
            name,
            path: rel_path,
            is_current,
            size_bytes: Some(meta.len()),
            modified_ms: file_modified_ms(&meta),
        });
    }
    files.sort_by(|left, right| {
        right
            .modified_ms
            .cmp(&left.modified_ms)
            .then(left.name.cmp(&right.name))
    });
    Ok(files)
}

fn slot_view(
    app_root: &Path,
    binding: &ProviderBinding,
) -> Result<AssetSlotView, AdminRecordError> {
    let slot_id = slot_id_from_binding(binding)?;
    let active_path = ops_source_path(app_root, slot_id.as_str());
    let files = list_files_in_slot(app_root, slot_id.as_str(), active_path.as_deref())?;
    let current = files.iter().find(|file| file.is_current);
    let status = if active_path
        .as_deref()
        .is_some_and(|path| app_root.join(path).is_file())
    {
        "ready".to_string()
    } else if files.is_empty() {
        "missing".to_string()
    } else {
        "pending".to_string()
    };
    Ok(AssetSlotView {
        slot_id: slot_id.clone(),
        title: slot_id.clone(),
        directory: slot_rel_dir(slot_id.as_str())?,
        active_path,
        kind: ops_source_kind(
            app_root,
            slot_id.as_str(),
            binding.payload_type.name.as_str(),
        ),
        status,
        size_bytes: current.and_then(|file| file.size_bytes),
        modified_ms: current.and_then(|file| file.modified_ms),
        files,
    })
}

pub fn list_asset_slots(
    app_root: &Path,
    bindings: &[ProviderBinding],
) -> Result<Vec<AssetSlotView>, AdminRecordError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for binding in bindings
        .iter()
        .filter(|binding| binding.provider_id == "asset-slot")
    {
        let slot_id = slot_id_from_binding(binding)?;
        if !seen.insert(slot_id) {
            continue;
        }
        out.push(slot_view(app_root, binding)?);
    }
    Ok(out)
}

pub fn get_asset_slot(
    app_root: &Path,
    binding: &ProviderBinding,
) -> Result<AssetSlotView, AdminRecordError> {
    slot_view(app_root, binding)
}

/// Upload a new file into `upload/admin/{slot}/` keeping the original basename.
/// Duplicate names are rejected (no silent rename).
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
    upload_asset_slot_file(
        app_root,
        binding,
        filename,
        bytes,
        actor,
        app_id,
        resource_id,
        module_id,
        correlation_id,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn upload_asset_slot_file(
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
    let stored_name = sanitize_upload_basename(filename)?;
    let slot_id = slot_id_from_binding(binding)?;
    let dir = slot_dir(app_root, slot_id.as_str())?;
    fs::create_dir_all(&dir)
        .map_err(|error| AdminRecordError::Io(format!("{}: {error}", dir.display())))?;
    let path = dir.join(&stored_name);
    if path.exists() {
        return Err(AdminRecordError::Validation(format!(
            "file `{stored_name}` already exists in slot `{slot_id}`; choose a different filename or delete the unused copy first"
        )));
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
            action: format!("upload:{stored_name}"),
            before_revision: None,
            after_revision: None,
            result: "success".to_string(),
            correlation_id: correlation_id.to_string(),
            at_ms: now_ms(),
        },
    )?;
    slot_view(app_root, binding)
}

/// Point `ops.sources.{slot}.path` at an existing file under the slot directory.
#[allow(clippy::too_many_arguments)]
pub fn apply_asset_slot_current(
    app_root: &Path,
    binding: &ProviderBinding,
    filename: &str,
    actor: &str,
    app_id: &str,
    resource_id: &str,
    module_id: &str,
    correlation_id: &str,
) -> Result<AssetSlotView, AdminRecordError> {
    if binding.provider_id != "asset-slot" {
        return Err(AdminRecordError::Validation(
            "binding is not an asset-slot".to_string(),
        ));
    }
    let name = sanitize_upload_basename(filename)?;
    let slot_id = slot_id_from_binding(binding)?;
    let dir = slot_dir(app_root, slot_id.as_str())?;
    let file_path = dir.join(&name);
    if !file_path.is_file() {
        return Err(AdminRecordError::NotFound(format!(
            "file `{name}` not found in slot `{slot_id}`"
        )));
    }
    let next_path = format!("{}/{}", slot_rel_dir(slot_id.as_str())?, name);
    let mut manifest = load_app_manifest(app_root);
    let mut ops = serde_json::to_value(&manifest.mei.ops)
        .map_err(|error| AdminRecordError::Parse(error.to_string()))?;
    let sources = ops
        .as_object_mut()
        .ok_or_else(|| AdminRecordError::Validation("ops is not an object".to_string()))?
        .entry("sources".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let source = sources
        .as_object_mut()
        .ok_or_else(|| AdminRecordError::Validation("ops.sources is not an object".to_string()))?
        .entry(slot_id.clone())
        .or_insert_with(|| {
            serde_json::json!({
                "kind": binding.payload_type.name,
                "path": next_path,
            })
        });
    let object = source.as_object_mut().ok_or_else(|| {
        AdminRecordError::Validation(format!("ops.sources.{slot_id} is not an object"))
    })?;
    object.insert("path".to_string(), Value::String(next_path.clone()));
    if let Some(kind) = kind_from_filename(name.as_str()) {
        object.insert("kind".to_string(), Value::String(kind.to_string()));
    } else if !object.contains_key("kind") {
        object.insert(
            "kind".to_string(),
            Value::String(binding.payload_type.name.clone()),
        );
    }
    manifest.mei.ops = serde_json::from_value(ops)
        .map_err(|error| AdminRecordError::Validation(error.to_string()))?;
    write_app_toml(app_root, &manifest).map_err(AdminRecordError::Io)?;
    append_admin_audit(
        app_root,
        &AdminAuditEntry {
            actor: actor.to_string(),
            app_id: app_id.to_string(),
            resource_id: resource_id.to_string(),
            module_id: module_id.to_string(),
            provider_id: binding.provider_id.clone(),
            method: "PUT".to_string(),
            target: format!("ops.sources.{slot_id}"),
            apply_policy: super::admin_registry::AdminApplyPolicy::RestartRuntime,
            danger: binding.danger,
            action: format!("apply-current:{next_path}"),
            before_revision: None,
            after_revision: None,
            result: "success".to_string(),
            correlation_id: correlation_id.to_string(),
            at_ms: now_ms(),
        },
    )?;
    slot_view(app_root, binding)
}

fn kind_from_filename(name: &str) -> Option<&'static str> {
    let ext = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase();
    match ext.as_str() {
        "xlsx" => Some("xlsx"),
        "xls" => Some("xls"),
        "csv" => Some("csv"),
        "json" => Some("json"),
        "geojson" => Some("geojson"),
        _ => None,
    }
}

/// Resolve a readable file path under `upload/admin/{slot}/` for download.
pub fn resolve_asset_slot_download_path(
    app_root: &Path,
    binding: &ProviderBinding,
    filename: &str,
) -> Result<(String, PathBuf), AdminRecordError> {
    if binding.provider_id != "asset-slot" {
        return Err(AdminRecordError::Validation(
            "binding is not an asset-slot".to_string(),
        ));
    }
    let name = sanitize_upload_basename(filename)?;
    let slot_id = slot_id_from_binding(binding)?;
    let dir = slot_dir(app_root, slot_id.as_str())?;
    let path = dir.join(&name);
    if !path.is_file() {
        return Err(AdminRecordError::NotFound(format!(
            "file `{name}` not found in slot `{slot_id}`"
        )));
    }
    let canonical_dir = fs::canonicalize(&dir)
        .map_err(|error| AdminRecordError::Io(format!("{}: {error}", dir.display())))?;
    let canonical_file = fs::canonicalize(&path)
        .map_err(|error| AdminRecordError::Io(format!("{}: {error}", path.display())))?;
    if !canonical_file.starts_with(&canonical_dir) {
        return Err(AdminRecordError::Validation(
            "download path escapes asset slot directory".to_string(),
        ));
    }
    Ok((name, path))
}

/// Delete a non-current file from the slot directory.
#[allow(clippy::too_many_arguments)]
pub fn delete_asset_slot_file(
    app_root: &Path,
    binding: &ProviderBinding,
    filename: &str,
    actor: &str,
    app_id: &str,
    resource_id: &str,
    module_id: &str,
    correlation_id: &str,
) -> Result<AssetSlotView, AdminRecordError> {
    if binding.provider_id != "asset-slot" {
        return Err(AdminRecordError::Validation(
            "binding is not an asset-slot".to_string(),
        ));
    }
    let name = sanitize_upload_basename(filename)?;
    let slot_id = slot_id_from_binding(binding)?;
    let active = ops_source_path(app_root, slot_id.as_str());
    let rel_path = format!("{}/{}", slot_rel_dir(slot_id.as_str())?, name);
    if active.as_deref() == Some(rel_path.as_str())
        || active
            .as_deref()
            .and_then(|path| Path::new(path).file_name()?.to_str())
            == Some(name.as_str())
    {
        return Err(AdminRecordError::Validation(
            "cannot delete the current datasource file; apply another file first".to_string(),
        ));
    }
    let path = slot_dir(app_root, slot_id.as_str())?.join(&name);
    if !path.is_file() {
        return Err(AdminRecordError::NotFound(format!(
            "file `{name}` not found in slot `{slot_id}`"
        )));
    }
    fs::remove_file(&path)
        .map_err(|error| AdminRecordError::Io(format!("{}: {error}", path.display())))?;
    append_admin_audit(
        app_root,
        &AdminAuditEntry {
            actor: actor.to_string(),
            app_id: app_id.to_string(),
            resource_id: resource_id.to_string(),
            module_id: module_id.to_string(),
            provider_id: binding.provider_id.clone(),
            method: "DELETE".to_string(),
            target: binding.target.clone(),
            apply_policy: binding.apply_policy,
            danger: binding.danger,
            action: format!("delete:{name}"),
            before_revision: None,
            after_revision: None,
            result: "success".to_string(),
            correlation_id: correlation_id.to_string(),
            at_ms: now_ms(),
        },
    )?;
    slot_view(app_root, binding)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mei_config::{AdminApplyPolicy, AdminDangerLevel, ProviderPayloadType};
    use std::fs;
    use tempfile::tempdir;

    fn binding(slot: &str, method: &str) -> ProviderBinding {
        ProviderBinding {
            binding_id: format!("{slot}_upload"),
            provider_id: "asset-slot".to_string(),
            method: method.to_string(),
            target: format!("upload/admin/{slot}"),
            payload_type: ProviderPayloadType {
                name: "file".to_string(),
                schema: None,
            },
            validator: None,
            revision: "none".to_string(),
            idempotency: "required".to_string(),
            apply_policy: AdminApplyPolicy::RestartRuntime,
            danger: AdminDangerLevel::Elevated,
            required_capabilities: vec!["config_upload".to_string()],
            source_anchor: "test".to_string(),
        }
    }

    #[test]
    fn upload_apply_and_delete_roundtrip() {
        let root = tempdir().unwrap();
        let app = root.path();
        fs::create_dir_all(app.join("env/current/var/admin")).unwrap();
        fs::write(
            app.join("app.toml"),
            r#"
schema = "mei-app-v1"
title = "demo"

[ops.sources.demo]
kind = "xlsx"
path = "upload/admin/demo/seed.xlsx"
header_row = 1
"#,
        )
        .unwrap();
        let dir = app.join("upload/admin/demo");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("seed.xlsx"), b"seed").unwrap();

        let write = binding("demo", "PUT");
        let uploaded = upload_asset_slot_file(
            app,
            &write,
            "事项.xlsx",
            b"new-bytes",
            "tester",
            "demo",
            "datasources",
            "import",
            "corr-1",
        )
        .unwrap();
        assert_eq!(uploaded.files.len(), 2);
        let new_file = uploaded
            .files
            .iter()
            .find(|file| file.name == "事项.xlsx")
            .expect("original filename preserved");
        assert!(!new_file.is_current);

        let dup = upload_asset_slot_file(
            app,
            &write,
            "事项.xlsx",
            b"again",
            "tester",
            "demo",
            "datasources",
            "import",
            "corr-1b",
        );
        assert!(dup.is_err());

        let applied = apply_asset_slot_current(
            app,
            &write,
            new_file.name.as_str(),
            "tester",
            "demo",
            "datasources",
            "import",
            "corr-2",
        )
        .unwrap();
        assert_eq!(applied.active_path.as_deref(), Some(new_file.path.as_str()));
        assert!(applied.files.iter().any(|file| file.is_current));

        let deleted = delete_asset_slot_file(
            app,
            &write,
            "seed.xlsx",
            "tester",
            "demo",
            "datasources",
            "import",
            "corr-3",
        )
        .unwrap();
        assert_eq!(deleted.files.len(), 1);

        let err = delete_asset_slot_file(
            app,
            &write,
            new_file.name.as_str(),
            "tester",
            "demo",
            "datasources",
            "import",
            "corr-4",
        );
        assert!(err.is_err());

        let (download_name, download_path) =
            resolve_asset_slot_download_path(app, &write, new_file.name.as_str()).unwrap();
        assert_eq!(download_name, new_file.name);
        assert_eq!(fs::read(download_path).unwrap(), b"new-bytes");

        let escape = resolve_asset_slot_download_path(app, &write, "../app.toml");
        assert!(escape.is_err());

        let xls_applied = apply_asset_slot_current(
            app,
            &write,
            {
                fs::write(dir.join("legacy.xls"), b"xls-bytes").unwrap();
                "legacy.xls"
            },
            "tester",
            "demo",
            "datasources",
            "import",
            "corr-5",
        )
        .unwrap();
        let manifest = load_app_manifest(app);
        let config = manifest.to_mei_config();
        let kind = config
            .ops
            .sources
            .get("demo")
            .map(|entry| entry.kind.as_str());
        assert_eq!(kind, Some("xls"));
        assert!(xls_applied
            .active_path
            .as_deref()
            .is_some_and(|path| path.ends_with("legacy.xls")));
    }
}
