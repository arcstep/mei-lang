use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use datafusion::arrow::array::{ArrayRef, StringArray};
use datafusion::arrow::datatypes::{DataType, Field, Schema};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::prelude::{ParquetReadOptions, SessionContext};
use mei_lang_kernel::{
    data_snapshot_store_root, parse_geojson_rows, parquet_snapshot_path,
    resolve_data_snapshot_import_entry, write_xlsx_parquet_snapshot, ColumnSchema, DatasetView,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use serde_json::Value;

use super::connection::{block_on, with_app_session};
use super::sql::{quote_ident, sql_cast_type};
use crate::paths::resolve_source_path;

const GEOJSON_ATTR_COLUMNS: &[&str] = &["id", "name", "type", "geometry-type"];

/// True when the dataset is a GeoJSON / FeatureCollection source.
pub fn is_geojson_source(view: &DatasetView) -> bool {
    let kind = view.source.kind.trim().to_ascii_lowercase();
    let path = view.source.path.as_str();
    kind == "geojson" || path.ends_with(".geojson")
}

/// Resolve parquet for SQL: tabular snapshots, or materialized GeoJSON attribute tables.
pub fn resolve_parquet_for_dataset_view(
    app_root: &Path,
    view: &DatasetView,
) -> Result<Option<PathBuf>> {
    if is_geojson_source(view) {
        return resolve_or_materialize_geojson_attr_parquet(app_root, view.source.path.as_str());
    }
    let header = view.source.header_row.unwrap_or(1).max(1) as usize;
    if let Some(path) = resolve_parquet_file_for_source(
        app_root,
        view.source.path.as_str(),
        view.source.sheet.as_deref(),
        header,
    ) {
        return Ok(Some(path));
    }
    // Demand-load materialize for tabular file sources when prebuild snapshot is missing.
    let kind = view.source.kind.trim().to_ascii_lowercase();
    if !matches!(kind.as_str(), "csv" | "json" | "xlsx" | "xls" | "file" | "") {
        return Ok(None);
    }
    let path = view.source.path.as_str();
    let lower = path.to_ascii_lowercase();
    if !(lower.ends_with(".csv")
        || lower.ends_with(".json")
        || lower.ends_with(".xlsx")
        || lower.ends_with(".xls"))
    {
        return Ok(None);
    }
    match write_xlsx_parquet_snapshot(
        app_root,
        path,
        view.source.sheet.as_deref(),
        header,
    ) {
        Ok(written) => Ok(Some(written)),
        Err(err) => {
            tracing::debug!(
                error = %err,
                source = path,
                "demand-load parquet snapshot failed"
            );
            Ok(None)
        }
    }
}

/// Materialize GeoJSON feature properties (no coordinates) into a cached parquet
/// under the app data-snapshot store, for DataFusion SQL joins / counts.
pub fn resolve_or_materialize_geojson_attr_parquet(
    app_root: &Path,
    source_path: &str,
) -> Result<Option<PathBuf>> {
    let abs = resolve_source_path(app_root, source_path);
    if !abs.is_file() {
        return Ok(None);
    }
    let meta = fs::metadata(&abs)
        .with_context(|| format!("stat geojson {}", abs.display()))?;
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut hasher = DefaultHasher::new();
    source_path.hash(&mut hasher);
    mtime_ms.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    let out = data_snapshot_store_root(app_root).join(format!(
        "geojson-attr-{:016x}.parquet",
        hasher.finish()
    ));
    if out.is_file() {
        return Ok(Some(out));
    }
    let raw = fs::read_to_string(&abs)
        .with_context(|| format!("read geojson {}", abs.display()))?;
    let rows = parse_geojson_rows(&raw)
        .with_context(|| format!("parse geojson {}", abs.display()))?;
    fs::create_dir_all(out.parent().unwrap_or(app_root))
        .with_context(|| format!("mkdir {}", out.parent().unwrap_or(app_root).display()))?;
    write_geojson_attr_parquet(&out, &rows)
        .with_context(|| format!("write geojson attr parquet {}", out.display()))?;
    Ok(Some(out))
}

fn write_geojson_attr_parquet(path: &Path, rows: &[Value]) -> Result<()> {
    let mut columns: Vec<ArrayRef> = Vec::with_capacity(GEOJSON_ATTR_COLUMNS.len());
    for name in GEOJSON_ATTR_COLUMNS {
        let values: Vec<Option<String>> = rows
            .iter()
            .map(|row| {
                row.as_object()
                    .and_then(|map| map.get(*name))
                    .and_then(json_cell_to_string)
            })
            .collect();
        columns.push(Arc::new(StringArray::from(values)) as ArrayRef);
    }
    let schema = Arc::new(Schema::new(
        GEOJSON_ATTR_COLUMNS
            .iter()
            .map(|name| Field::new(*name, DataType::Utf8, true))
            .collect::<Vec<_>>(),
    ));
    let batch = RecordBatch::try_new(schema.clone(), columns)
        .context("geojson attr RecordBatch")?;
    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut writer = ArrowWriter::try_new(file, schema, None)
        .context("geojson attr ArrowWriter")?;
    writer.write(&batch).context("write geojson attr batch")?;
    writer.close().context("close geojson attr parquet")?;
    Ok(())
}

fn json_cell_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        other => Some(other.to_string()),
    }
}

/// Resolve on-disk parquet for an xlsx/csv-backed source when import snapshot exists.
pub fn resolve_parquet_file_for_source(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Option<PathBuf> {
    let header_row = header_row.max(1);
    if let Some(entry) =
        resolve_data_snapshot_import_entry(app_root, source_path, sheet, header_row)
    {
        let candidate = PathBuf::from(&entry.artifact_path);
        if candidate.is_file() {
            return Some(candidate);
        }
        if let Some(name) = candidate.file_name() {
            let under_store = mei_lang_kernel::data_snapshot_store_root(app_root).join(name);
            if under_store.is_file() {
                return Some(under_store);
            }
        }
    }
    let path = parquet_snapshot_path(app_root, source_path, sheet, header_row)?;
    path.is_file().then_some(path)
}

fn view_name_for(parquet_path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    parquet_path.to_string_lossy().hash(&mut hasher);
    format!("mei_pq_{:016x}", hasher.finish())
}

fn raw_table_name(view: &str) -> String {
    format!("{view}_raw")
}

/// Ensure a CAST view over registered parquet exists; returns view name + logical columns.
pub fn ensure_parquet_view(
    app_root: &Path,
    parquet_path: &Path,
    schema: &[ColumnSchema],
    physical_columns: Option<&[String]>,
) -> Result<(String, Vec<String>)> {
    if !parquet_path.is_file() {
        bail!("parquet file missing: {}", parquet_path.display());
    }
    let view = view_name_for(parquet_path);
    let abs = parquet_path
        .canonicalize()
        .unwrap_or_else(|_| parquet_path.to_path_buf());
    let abs_str = abs.to_string_lossy().into_owned();

    with_app_session(app_root, |ctx| {
        let columns = if let Some(cols) = physical_columns.filter(|c| !c.is_empty()) {
            cols.iter()
                .filter(|c| !is_parquet_metadata_column(c))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            discover_parquet_columns(&abs)?
        };

        let select_list = if schema.is_empty() {
            columns
                .iter()
                .map(|c| quote_ident(c))
                .collect::<Result<Vec<_>>>()?
                .join(", ")
        } else {
            schema
                .iter()
                .map(|col| {
                    let physical = col.source.as_deref().unwrap_or(col.name.as_str());
                    let cast_ty = sql_cast_type(col.type_name.as_str());
                    let alias = quote_ident(col.name.as_str())?;
                    if columns.iter().any(|c| c == physical) {
                        let src = quote_ident(physical)?;
                        Ok(format!("try_cast({src} AS {cast_ty}) AS {alias}"))
                    } else {
                        Ok(format!("CAST(NULL AS {cast_ty}) AS {alias}"))
                    }
                })
                .collect::<Result<Vec<_>>>()?
                .join(", ")
        };

        let raw = raw_table_name(&view);
        register_parquet_table(ctx, &raw, &abs_str)?;
        let view_ident = quote_ident(&view)?;
        let raw_ident = quote_ident(&raw)?;
        if ctx.table_exist(&view).unwrap_or(false) {
            let _ = ctx.deregister_table(&view);
        }
        let ddl = format!("CREATE VIEW {view_ident} AS SELECT {select_list} FROM {raw_ident}");
        block_on(async {
            let _ = ctx
                .sql(&ddl)
                .await
                .with_context(|| {
                    format!(
                        "create parquet view {} path={} ddl={}",
                        view,
                        abs.display(),
                        ddl
                    )
                })?
                .collect()
                .await
                .with_context(|| format!("collect create view {}", view))?;
            Ok::<(), anyhow::Error>(())
        })?;

        let out_columns = if schema.is_empty() {
            columns
        } else {
            schema.iter().map(|c| c.name.clone()).collect()
        };
        Ok((view, out_columns))
    })
}

fn register_parquet_table(ctx: &SessionContext, table: &str, path: &str) -> Result<()> {
    // Re-register is idempotent enough for MVP: drop if present then register.
    let exists = ctx.table_exist(table).unwrap_or(false);
    if exists {
        let _ = ctx.deregister_table(table);
    }
    block_on(async {
        ctx.register_parquet(table, path, ParquetReadOptions::default())
            .await
            .with_context(|| format!("register_parquet {table} path={path}"))
    })
}

fn is_parquet_metadata_column(name: &str) -> bool {
    // Skip Arrow/engine metadata columns if present in older snapshots.
    matches!(name, "arrow_schema" | "duckdb_schema")
}

fn discover_parquet_columns(path: &Path) -> Result<Vec<String>> {
    let file = File::open(path).with_context(|| format!("open parquet {}", path.display()))?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file)
        .with_context(|| format!("parquet reader {}", path.display()))?;
    let schema = builder.schema();
    let mut columns = Vec::new();
    for field in schema.fields() {
        let name = field.name();
        if !is_parquet_metadata_column(name) {
            columns.push(name.clone());
        }
    }
    if columns.is_empty() {
        bail!(
            "parquet schema returned no data columns for {}",
            path.display()
        );
    }
    Ok(columns)
}
