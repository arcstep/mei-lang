//! xlsx 表快照的 Parquet 旁路：发布时物化、运行时 mmap 优先加载。

use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use arrow_array::{Array, ArrayRef, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::loaders::{load_xlsx_table_snapshot, XlsxTableSnapshot};
use super::scene_payload_cache::file_mtime_ms;
use crate::resolve_versioned_source_identifier;

pub const DATA_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub const DATA_SNAPSHOT_IMPORT_MANIFEST_SCHEMA_VERSION: &str = "mei-dataset-import-manifest-v1";
const PARQUET_META_COLUMNS: &str = "mei_columns_json";
const PARQUET_META_SOURCE_PATH: &str = "mei_source_path";
const PARQUET_META_SHEET: &str = "mei_sheet";
const PARQUET_META_HEADER_ROW: &str = "mei_header_row";
const PARQUET_META_CONTENT_SIG: &str = "mei_content_sig";
const PARQUET_META_ROW_COUNT: &str = "mei_row_count";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataSnapshotImportManifest {
    pub schema_version: String,
    #[serde(default)]
    pub entries: Vec<DataSnapshotImportEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataSnapshotImportEntry {
    pub source_path: String,
    pub resolved_source_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    pub header_row: usize,
    pub content_signature: String,
    pub artifact_path: String,
    pub row_count: usize,
    pub column_count: usize,
    #[serde(default)]
    pub columns: Vec<String>,
    pub imported_at_ms: u64,
}

pub fn data_snapshot_store_root(app_root: &Path) -> PathBuf {
    app_root.join(".mei").join("data-snapshots")
}

pub fn data_snapshot_import_manifest_path(app_root: &Path) -> PathBuf {
    data_snapshot_store_root(app_root).join("import-manifest.json")
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}

pub fn source_file_content_signature(path: &Path, rel: &str) -> String {
    let mtime = file_mtime_ms(path);
    std::fs::read(path)
        .ok()
        .map(|bytes| {
            let mut hasher = DefaultHasher::new();
            rel.hash(&mut hasher);
            bytes.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        })
        .unwrap_or_else(|| format!("mtime:{mtime}"))
}

fn parquet_snapshot_filename(content_sig: &str, sheet: &str, header_row: usize) -> String {
    let sheet_slug = if sheet.is_empty() {
        "default".to_string()
    } else {
        sheet.replace('/', "_")
    };
    format!("{content_sig}__{sheet_slug}__h{header_row}.parquet")
}

pub fn parquet_snapshot_path(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Option<PathBuf> {
    let source_path = source_path.trim();
    if source_path.is_empty() {
        return None;
    }
    let resolved = resolve_versioned_source_identifier(app_root, source_path);
    let absolute = app_root.join(&resolved);
    if !absolute.is_file() {
        return None;
    }
    let content_sig = source_file_content_signature(absolute.as_path(), resolved.as_str());
    let sheet = sheet.unwrap_or("").trim();
    Some(
        data_snapshot_store_root(app_root).join(parquet_snapshot_filename(
            content_sig.as_str(),
            sheet,
            header_row.max(1),
        )),
    )
}

pub fn read_data_snapshot_import_manifest(
    app_root: &Path,
) -> Result<Option<DataSnapshotImportManifest>> {
    let path = data_snapshot_import_manifest_path(app_root);
    if !path.is_file() {
        return Ok(None);
    }
    let manifest = serde_json::from_str::<DataSnapshotImportManifest>(
        &fs::read_to_string(&path)
            .with_context(|| format!("read import manifest {}", path.display()))?,
    )
    .with_context(|| format!("parse import manifest {}", path.display()))?;
    Ok(Some(manifest))
}

pub fn write_data_snapshot_import_manifest(
    app_root: &Path,
    manifest: &DataSnapshotImportManifest,
) -> Result<PathBuf> {
    let path = data_snapshot_import_manifest_path(app_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create import manifest dir {}", parent.display()))?;
    }
    fs::write(&path, serde_json::to_string_pretty(manifest)?)
        .with_context(|| format!("write import manifest {}", path.display()))?;
    Ok(path)
}

pub fn resolve_data_snapshot_import_entry(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Option<DataSnapshotImportEntry> {
    let source_path = source_path.trim();
    if source_path.is_empty() {
        return None;
    }
    let resolved = resolve_versioned_source_identifier(app_root, source_path);
    let absolute = app_root.join(&resolved);
    if !absolute.is_file() {
        return None;
    }
    let expected_sig = source_file_content_signature(absolute.as_path(), resolved.as_str());
    let manifest = read_data_snapshot_import_manifest(app_root)
        .ok()
        .flatten()?;
    manifest.entries.into_iter().find(|entry| {
        entry.resolved_source_path == resolved
            && entry.header_row == header_row.max(1)
            && entry.sheet.as_deref().unwrap_or("") == sheet.unwrap_or("").trim()
            && entry.content_signature == expected_sig
    })
}

fn upsert_data_snapshot_import_entry(
    app_root: &Path,
    entry: DataSnapshotImportEntry,
) -> Result<()> {
    let mut manifest =
        read_data_snapshot_import_manifest(app_root)?.unwrap_or(DataSnapshotImportManifest {
            schema_version: DATA_SNAPSHOT_IMPORT_MANIFEST_SCHEMA_VERSION.to_string(),
            entries: Vec::new(),
        });
    manifest.schema_version = DATA_SNAPSHOT_IMPORT_MANIFEST_SCHEMA_VERSION.to_string();
    manifest.entries.retain(|existing| {
        !(existing.resolved_source_path == entry.resolved_source_path
            && existing.header_row == entry.header_row
            && existing.sheet == entry.sheet)
    });
    manifest.entries.push(entry);
    manifest.entries.sort_by(|left, right| {
        left.resolved_source_path
            .cmp(&right.resolved_source_path)
            .then(left.sheet.cmp(&right.sheet))
            .then(left.header_row.cmp(&right.header_row))
    });
    write_data_snapshot_import_manifest(app_root, &manifest)?;
    Ok(())
}

fn cell_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        other => other.to_string(),
    }
}

fn snapshot_to_record_batch(snapshot: &XlsxTableSnapshot) -> Result<RecordBatch> {
    let fields = snapshot
        .columns
        .iter()
        .map(|name| Field::new(name.as_str(), DataType::Utf8, true))
        .collect::<Vec<_>>();
    let schema = Arc::new(Schema::new(fields));
    let mut arrays: Vec<ArrayRef> = Vec::with_capacity(snapshot.columns.len());
    for column in &snapshot.columns {
        let values = snapshot
            .rows
            .iter()
            .map(|row| row.get(column).map(cell_to_string))
            .collect::<Vec<_>>();
        arrays.push(Arc::new(StringArray::from(values)) as ArrayRef);
    }
    RecordBatch::try_new(schema, arrays).context("build parquet record batch")
}

pub fn write_xlsx_parquet_snapshot(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Result<PathBuf> {
    let source_path = source_path.trim();
    let resolved = resolve_versioned_source_identifier(app_root, source_path);
    let absolute = app_root.join(&resolved);
    let snapshot = load_xlsx_table_snapshot(
        absolute.as_path(),
        source_path,
        sheet,
        header_row.max(1),
        None,
    )?;
    let out_path = parquet_snapshot_path(app_root, source_path, sheet, header_row)
        .context("resolve parquet snapshot path")?;
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create data-snapshot dir {}", parent.display()))?;
    }
    let batch = snapshot_to_record_batch(&snapshot)?;
    let file = File::create(&out_path)
        .with_context(|| format!("create parquet {}", out_path.display()))?;
    let sheet_label = sheet.unwrap_or("").trim();
    let content_sig = source_file_content_signature(absolute.as_path(), resolved.as_str());
    let props = WriterProperties::builder()
        .set_key_value_metadata(Some(vec![
            parquet::file::metadata::KeyValue::new(
                PARQUET_META_COLUMNS.to_string(),
                serde_json::to_string(&snapshot.columns)?,
            ),
            parquet::file::metadata::KeyValue::new(
                PARQUET_META_SOURCE_PATH.to_string(),
                source_path.to_string(),
            ),
            parquet::file::metadata::KeyValue::new(
                PARQUET_META_SHEET.to_string(),
                sheet_label.to_string(),
            ),
            parquet::file::metadata::KeyValue::new(
                PARQUET_META_HEADER_ROW.to_string(),
                header_row.max(1).to_string(),
            ),
            parquet::file::metadata::KeyValue::new(
                PARQUET_META_CONTENT_SIG.to_string(),
                content_sig.clone(),
            ),
            parquet::file::metadata::KeyValue::new(
                PARQUET_META_ROW_COUNT.to_string(),
                snapshot.rows.len().to_string(),
            ),
        ]))
        .build();
    let mut writer =
        ArrowWriter::try_new(file, batch.schema(), Some(props)).context("open parquet writer")?;
    writer.write(&batch).context("write parquet batch")?;
    writer.close().context("close parquet writer")?;
    upsert_data_snapshot_import_entry(
        app_root,
        DataSnapshotImportEntry {
            source_path: source_path.to_string(),
            resolved_source_path: resolved,
            sheet: (!sheet_label.is_empty()).then(|| sheet_label.to_string()),
            header_row: header_row.max(1),
            content_signature: content_sig,
            artifact_path: out_path.display().to_string(),
            row_count: snapshot.rows.len(),
            column_count: snapshot.columns.len(),
            columns: snapshot.columns.clone(),
            imported_at_ms: now_epoch_ms(),
        },
    )?;
    Ok(out_path)
}

fn record_batch_to_snapshot(batch: &RecordBatch, columns: &[String]) -> Result<XlsxTableSnapshot> {
    let row_count = batch.num_rows();
    let mut rows = Vec::with_capacity(row_count);
    for row_index in 0..row_count {
        let mut obj = serde_json::Map::new();
        for (col_index, column) in columns.iter().enumerate() {
            let array = batch.column(col_index);
            let text = array
                .as_any()
                .downcast_ref::<StringArray>()
                .and_then(|values| values.is_valid(row_index).then(|| values.value(row_index)))
                .unwrap_or("")
                .to_string();
            if !text.is_empty() {
                obj.insert(column.clone(), Value::String(text));
            }
        }
        if obj.values().any(|value| !value.is_null()) {
            rows.push(Value::Object(obj));
        }
    }
    Ok(XlsxTableSnapshot {
        columns: columns.to_vec(),
        rows,
    })
}

pub fn try_load_xlsx_parquet_snapshot(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Option<XlsxTableSnapshot> {
    let path = parquet_snapshot_path(app_root, source_path, sheet, header_row)?;
    if !path.is_file() {
        return None;
    }
    let resolved = resolve_versioned_source_identifier(app_root, source_path.trim());
    let absolute = app_root.join(&resolved);
    if !absolute.is_file() {
        return None;
    }
    let expected_sig = source_file_content_signature(absolute.as_path(), resolved.as_str());
    let file = File::open(&path).ok()?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).ok()?;
    let meta = builder.metadata().file_metadata().key_value_metadata()?;
    let stored_sig = meta
        .iter()
        .find(|kv| kv.key == PARQUET_META_CONTENT_SIG)
        .map(|kv| kv.value.as_deref().unwrap_or(""))
        .unwrap_or("");
    if stored_sig != expected_sig {
        return None;
    }
    let columns: Vec<String> = meta
        .iter()
        .find(|kv| kv.key == PARQUET_META_COLUMNS)
        .and_then(|kv| kv.value.as_deref())
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();
    let mut reader = builder.build().ok()?;
    let mut rows = Vec::new();
    let mut merged_columns = columns.clone();
    while let Some(batch) = reader.next().transpose().ok()? {
        if merged_columns.is_empty() {
            merged_columns = batch
                .schema()
                .fields()
                .iter()
                .map(|field| field.name().clone())
                .collect();
        }
        let snapshot = record_batch_to_snapshot(&batch, merged_columns.as_slice()).ok()?;
        rows.extend(snapshot.rows);
    }
    Some(XlsxTableSnapshot {
        columns: merged_columns,
        rows,
    })
}

pub fn publish_xlsx_data_snapshots_for_paths(
    app_root: &Path,
    sources: &[(&str, Option<&str>, usize)],
) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for (path, sheet, header_row) in sources {
        let out = write_xlsx_parquet_snapshot(app_root, path, *sheet, *header_row)?;
        written.push(out);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn parquet_roundtrip_matches_calamine_snapshot() {
        let app_root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../workspaces/ws-spbjw/zhifa");
        let rel = "upload/8.行政处罚结果清单.xlsx";
        if !app_root.join(rel).is_file() {
            return;
        }
        let written = write_xlsx_parquet_snapshot(app_root.as_path(), rel, None, 1).expect("write");
        assert!(written.is_file());
        let from_parquet =
            try_load_xlsx_parquet_snapshot(app_root.as_path(), rel, None, 1).expect("read parquet");
        let from_xlsx = load_xlsx_table_snapshot(app_root.join(rel).as_path(), rel, None, 1, None)
            .expect("xlsx");
        assert_eq!(from_parquet.columns, from_xlsx.columns);
        assert_eq!(from_parquet.rows.len(), from_xlsx.rows.len());
        let _ = fs::remove_file(written);
    }
}
