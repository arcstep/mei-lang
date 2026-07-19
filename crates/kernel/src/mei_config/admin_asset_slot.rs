//! Asset-slot provider for Admin Platform (0549).
//!
//! Slots are declared in `schema_ref` JSON (`slots[]` with id/path/kind).
//! Physical files live under the app `paths.upload` sandbox.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::admin_manifest::{validate_relative_sandbox_path, AdminResourceSpec, AdminUploadSpec};
use super::admin_record::{append_admin_audit, AdminAuditEntry, AdminRecordError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotSchemaDoc {
    #[serde(default)]
    pub slots: Vec<AssetSlotDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotDef {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub path: String,
    #[serde(default)]
    pub kind: Option<String>,
}

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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn load_slot_schema(
    app_root: &Path,
    schema_ref: &str,
) -> Result<AssetSlotSchemaDoc, AdminRecordError> {
    validate_relative_sandbox_path(schema_ref, "schema_ref")?;
    let path = app_root.join(schema_ref.trim());
    if !path.is_file() {
        return Err(AdminRecordError::NotFound(format!(
            "schema_ref missing: {}",
            path.display()
        )));
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| AdminRecordError::Io(format!("{}: {e}", path.display())))?;
    serde_json::from_str(&raw).map_err(|e| AdminRecordError::Parse(e.to_string()))
}

pub fn resolve_slot_defs(
    app_root: &Path,
    resource: &AdminResourceSpec,
) -> Result<Vec<AssetSlotDef>, AdminRecordError> {
    let Some(upload) = resource.upload.as_ref() else {
        return Err(AdminRecordError::Validation(
            "asset-slot resource requires upload spec".into(),
        ));
    };
    let Some(schema_ref) = upload
        .schema_ref
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    else {
        return Err(AdminRecordError::Validation(
            "asset-slot resource requires upload.schema_ref".into(),
        ));
    };
    let doc = load_slot_schema(app_root, schema_ref)?;
    if doc.slots.is_empty() {
        return Err(AdminRecordError::Validation(
            "datasource schema must declare at least one slot".into(),
        ));
    }
    for slot in &doc.slots {
        validate_relative_sandbox_path(&slot.path, "slot.path")?;
    }
    Ok(doc.slots)
}

fn file_meta(path: &Path) -> (String, Option<u64>, Option<u64>) {
    if !path.is_file() {
        return ("missing".into(), None, None);
    }
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return ("error".into(), None, None),
    };
    let size = meta.len();
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    ("ready".into(), Some(size), modified_ms)
}

pub fn list_asset_slots(
    app_root: &Path,
    resource: &AdminResourceSpec,
) -> Result<Vec<AssetSlotView>, AdminRecordError> {
    let slots = resolve_slot_defs(app_root, resource)?;
    Ok(slots
        .into_iter()
        .map(|slot| {
            let abs = app_root.join(&slot.path);
            let (status, size_bytes, modified_ms) = file_meta(&abs);
            AssetSlotView {
                slot_id: slot.id.clone(),
                title: slot.title.clone().unwrap_or_else(|| slot.id.clone()),
                path: slot.path,
                kind: slot.kind.unwrap_or_else(|| "file".into()),
                status,
                size_bytes,
                modified_ms,
            }
        })
        .collect())
}

pub fn get_asset_slot(
    app_root: &Path,
    resource: &AdminResourceSpec,
    slot_id: &str,
) -> Result<AssetSlotView, AdminRecordError> {
    list_asset_slots(app_root, resource)?
        .into_iter()
        .find(|s| s.slot_id == slot_id)
        .ok_or_else(|| AdminRecordError::NotFound(format!("slot `{slot_id}` not found")))
}

fn extension_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_ascii_lowercase()))
        .unwrap_or_default()
}

fn validate_replace_bytes(
    upload: &AdminUploadSpec,
    filename: &str,
    bytes: &[u8],
) -> Result<(), AdminRecordError> {
    let ext = extension_of(filename);
    if !upload.accept.is_empty() && !upload.accept.iter().any(|a| a.eq_ignore_ascii_case(&ext)) {
        return Err(AdminRecordError::Validation(format!(
            "file extension `{ext}` not in accept {:?}",
            upload.accept
        )));
    }
    if let Some(max) = upload.max_bytes {
        if (bytes.len() as u64) > max {
            return Err(AdminRecordError::Validation(format!(
                "file size {} exceeds max_bytes {max}",
                bytes.len()
            )));
        }
    }
    if bytes.is_empty() {
        return Err(AdminRecordError::Validation("file is empty".into()));
    }
    if ext == ".csv" {
        let text = String::from_utf8_lossy(bytes);
        let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
        if lines < 1 {
            return Err(AdminRecordError::Validation(
                "csv must contain at least a header row".into(),
            ));
        }
    }
    Ok(())
}

/// Replace slot file contents (hard replace into declared path).
pub fn replace_asset_slot(
    app_root: &Path,
    resource: &AdminResourceSpec,
    slot_id: &str,
    filename: &str,
    bytes: &[u8],
    actor: &str,
    app_id: &str,
    correlation_id: &str,
) -> Result<AssetSlotView, AdminRecordError> {
    let upload = resource.upload.as_ref().ok_or_else(|| {
        AdminRecordError::Validation("asset-slot resource requires upload spec".into())
    })?;
    validate_replace_bytes(upload, filename, bytes)?;

    let slots = resolve_slot_defs(app_root, resource)?;
    let slot = slots
        .iter()
        .find(|s| s.id == slot_id)
        .ok_or_else(|| AdminRecordError::NotFound(format!("slot `{slot_id}` not found")))?;

    let target_ext = extension_of(&slot.path);
    let upload_ext = extension_of(filename);
    if !target_ext.is_empty() && !upload_ext.is_empty() && target_ext != upload_ext {
        return Err(AdminRecordError::Validation(format!(
            "uploaded extension `{upload_ext}` must match slot path extension `{target_ext}`"
        )));
    }

    let dest = app_root.join(&slot.path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AdminRecordError::Io(format!("{}: {e}", parent.display())))?;
    }
    // Binary-safe write via temp + rename.
    let tmp = dest.with_extension(format!(
        "tmp-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::write(&tmp, bytes).map_err(|e| AdminRecordError::Io(format!("{}: {e}", tmp.display())))?;
    fs::rename(&tmp, &dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        AdminRecordError::Io(format!("{}: {e}", dest.display()))
    })?;

    let _ = append_admin_audit(
        app_root,
        &AdminAuditEntry {
            actor: actor.to_string(),
            scope: "app".to_string(),
            app_id: app_id.to_string(),
            resource_id: resource.resource_id.clone(),
            action: format!("replace:{slot_id}"),
            provider: "asset-slot".to_string(),
            before_revision: None,
            after_revision: None,
            result: "success".to_string(),
            correlation_id: correlation_id.to_string(),
            at_ms: now_ms(),
        },
    );

    get_asset_slot(app_root, resource, slot_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mei_config::admin_manifest::{AdminProviderKind, AdminTemplate, AdminUploadSpec};
    use std::path::PathBuf;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/admin-phase-d-app")
    }

    fn datasources_spec() -> AdminResourceSpec {
        crate::mei_config::load_admin_mdx_resource(
            &fixture_root().join("src/admin/datasources.admin.mdx"),
        )
        .expect("phase-d datasources admin.mdx")
    }

    #[test]
    fn lists_four_slots_from_phase_d_fixture() {
        let root = fixture_root();
        assert!(root.join("app.toml").is_file());
        let spec = datasources_spec();
        let slots = list_asset_slots(&root, &spec).unwrap();
        assert_eq!(slots.len(), 4);
        assert!(slots.iter().any(|s| s.slot_id == "enforcement_objects"));
        assert!(slots.iter().all(|s| s.status == "ready"));
    }

    #[test]
    fn rejects_bad_extension_on_replace() {
        let root = fixture_root();
        let spec = datasources_spec();
        let err = replace_asset_slot(
            &root,
            &spec,
            "enforcement_objects",
            "evil.exe",
            b"not-a-csv",
            "test",
            "admin-phase-d-app",
            "c1",
        )
        .unwrap_err();
        assert!(matches!(err, AdminRecordError::Validation(_)));
    }

    #[test]
    fn replace_csv_round_trip_in_temp_copy() {
        let src = fixture_root();
        let dir = tempfile::tempdir().unwrap();
        let app_root = dir.path().join("app");
        copy_dir(&src, &app_root);
        let spec = crate::mei_config::load_admin_mdx_resource(
            &app_root.join("src/admin/datasources.admin.mdx"),
        )
        .expect("copied datasources admin.mdx");
        let body = "id,name\n9,new-object\n".as_bytes();
        let view = replace_asset_slot(
            &app_root,
            &spec,
            "enforcement_objects",
            "enforcement-objects.csv",
            body,
            "test",
            "admin-phase-d-app",
            "c2",
        )
        .unwrap();
        assert_eq!(view.status, "ready");
        let written = fs::read_to_string(app_root.join("upload/enforcement-objects.csv")).unwrap();
        assert!(written.contains("new-object"));
    }

    fn copy_dir(src: &Path, dst: &Path) {
        fs::create_dir_all(dst).unwrap();
        for entry in fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            let to = dst.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &to);
            } else {
                fs::copy(entry.path(), to).unwrap();
            }
        }
    }

    #[test]
    fn upload_spec_required() {
        let spec = AdminResourceSpec {
            resource_id: "x".into(),
            title: "x".into(),
            description: None,
            namespace: None,
            template: AdminTemplate::AssetSlotCollection,
            provider: AdminProviderKind::AssetSlot,
            record_path: None,
            config_path: None,
            required_capabilities: vec![],
            scope: None,
            audit: None,
            danger_level: None,
            revision_policy: None,
            validation: None,
            idempotency: None,
            dirty_policy: None,
            apply_policy: None,
            navigation: None,
            sections: vec![],
            columns: vec![],
            allowed_views: vec![],
            upload: Some(AdminUploadSpec {
                accept: vec![".csv".into()],
                max_bytes: Some(10),
                replace_modes: vec![],
                retain_versions: None,
                schema_ref: None,
                requires_review: None,
            }),
            actions: vec![],
            query: None,
            get: None,
            mutation: None,
        };
        let err = resolve_slot_defs(Path::new("/tmp"), &spec).unwrap_err();
        assert!(matches!(err, AdminRecordError::Validation(_)));
    }
}
