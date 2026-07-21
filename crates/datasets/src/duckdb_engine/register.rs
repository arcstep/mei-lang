use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use datafusion::prelude::{ParquetReadOptions, SessionContext};
use mei_lang_kernel::{
    parquet_snapshot_path, resolve_data_snapshot_import_entry, ColumnSchema,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use super::connection::{block_on, with_app_session};
use super::sql::{sql_cast_type, quote_ident};

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
        bail!("parquet schema returned no data columns for {}", path.display());
    }
    Ok(columns)
}
