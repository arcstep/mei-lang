//! Per-app `ops.sources` fingerprints for runtime data-plane reconciliation.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::compile::{
    source_file_content_signature, source_ingest_sidecar_key,
};
use crate::load_mei_config_for_app;
use crate::mei_config::resolve_app_var_root;
use crate::resolve_versioned_source_identifier;
use crate::resolve_versioned_source_path;

pub const SOURCE_FINGERPRINT_SCHEMA_VERSION: &str = "mei-source-fingerprint-v1";
pub const SOURCE_FINGERPRINT_REL: &str = "var/active/source-fingerprints.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceFingerprintEntry {
    pub resolved_path: String,
    #[serde(default)]
    pub sheet: Option<String>,
    #[serde(default, rename = "headerRow")]
    pub header_row: usize,
    #[serde(default, rename = "contentSignature")]
    pub content_signature: String,
    #[serde(default, rename = "primaryKey")]
    pub primary_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourceFingerprintRecord {
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(default, rename = "appId")]
    pub app_id: String,
    #[serde(default)]
    pub sources: BTreeMap<String, SourceFingerprintEntry>,
    #[serde(default, rename = "updatedAtMs")]
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceFingerprintDrift {
    pub changed_source_ids: Vec<String>,
}

pub fn source_fingerprint_path(app_root: &Path) -> PathBuf {
    resolve_app_var_root(app_root).join("source-fingerprints.json")
}

pub fn load_source_fingerprints(app_root: &Path, app_id: &str) -> SourceFingerprintRecord {
    let path = source_fingerprint_path(app_root);
    let Ok(raw) = fs::read_to_string(&path) else {
        return default_record(app_id);
    };
    serde_json::from_str(&raw).unwrap_or_else(|_| default_record(app_id))
}

pub fn save_source_fingerprints(app_root: &Path, record: &SourceFingerprintRecord) -> Result<()> {
    let path = source_fingerprint_path(app_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create source fingerprint dir {}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(record)?)
        .with_context(|| format!("write source fingerprint tmp {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("rename source fingerprint {}", path.display()))?;
    Ok(())
}

fn default_record(app_id: &str) -> SourceFingerprintRecord {
    SourceFingerprintRecord {
        schema_version: SOURCE_FINGERPRINT_SCHEMA_VERSION.to_string(),
        app_id: app_id.to_string(),
        sources: BTreeMap::new(),
        updated_at_ms: crate::cache_generation::current_time_ms(),
    }
}

pub fn compute_ops_source_fingerprints(
    app_root: &Path,
    workspace_root: Option<&Path>,
) -> BTreeMap<String, SourceFingerprintEntry> {
    let config = load_mei_config_for_app(app_root, workspace_root);
    let mut out = BTreeMap::new();
    for (source_id, entry) in &config.ops.sources {
        let kind = entry.kind.trim().to_ascii_lowercase();
        if matches!(
            kind.as_str(),
            "postgres" | "postgresql" | "timescale" | "timescaledb"
        ) || entry
            .connection
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        {
            continue;
        }
        let rel = entry.path.trim().replace('\\', "/");
        if rel.is_empty() {
            continue;
        }
        let resolved = resolve_versioned_source_identifier(app_root, rel.as_str());
        let absolute = resolve_versioned_source_path(app_root, rel.as_str());
        let content_signature = if absolute.is_file() {
            source_file_content_signature(absolute.as_path(), resolved.as_str())
        } else {
            String::new()
        };
        let sheet = entry
            .sheet
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let header_row = entry
            .header_row
            .and_then(|value| (value > 0).then_some(value as usize))
            .unwrap_or(1);
        let primary_key = entry
            .primary_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        out.insert(
            source_id.clone(),
            SourceFingerprintEntry {
                resolved_path: resolved,
                sheet,
                header_row,
                content_signature,
                primary_key,
            },
        );
    }
    out
}

pub fn detect_ops_source_fingerprint_drift(
    persisted: &SourceFingerprintRecord,
    current: &BTreeMap<String, SourceFingerprintEntry>,
) -> SourceFingerprintDrift {
    let mut changed = Vec::new();
    for (source_id, entry) in current {
        match persisted.sources.get(source_id) {
            Some(previous) if previous == entry => {}
            _ => changed.push(source_id.clone()),
        }
    }
    for source_id in persisted.sources.keys() {
        if !current.contains_key(source_id) {
            changed.push(source_id.clone());
        }
    }
    changed.sort();
    changed.dedup();
    SourceFingerprintDrift {
        changed_source_ids: changed,
    }
}

pub fn ingest_sidecar_key_for_ops_source(
    entry: &SourceFingerprintEntry,
) -> String {
    source_ingest_sidecar_key(
        entry.resolved_path.as_str(),
        entry.sheet.as_deref(),
        entry.header_row,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drift_detects_path_and_content_changes() {
        let mut persisted = SourceFingerprintRecord::default();
        persisted.sources.insert(
            "ledger".to_string(),
            SourceFingerprintEntry {
                resolved_path: "upload/a.xlsx".to_string(),
                sheet: None,
                header_row: 1,
                content_signature: "sig-a".to_string(),
                primary_key: None,
            },
        );
        let mut current = persisted.sources.clone();
        assert!(detect_ops_source_fingerprint_drift(&persisted, &current)
            .changed_source_ids
            .is_empty());
        current.get_mut("ledger").unwrap().content_signature = "sig-b".to_string();
        let drift = detect_ops_source_fingerprint_drift(&persisted, &current);
        assert_eq!(drift.changed_source_ids, vec!["ledger".to_string()]);
    }
}
