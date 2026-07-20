use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use mei_lang_kernel::{
    parquet_snapshot_path, resolve_data_snapshot_import_entry, ColumnSchema,
};

use super::connection::with_app_connection;
use super::sql::{duck_cast_type, quote_ident, quote_path};

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

/// Ensure a CAST view over `read_parquet` exists; returns view name + logical columns.
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
    let path_sql = quote_path(&abs.to_string_lossy());

    with_app_connection(app_root, |conn| {
        // Physical names must come from the parquet file (or import manifest), never from
        // declared schema sources — a missing source like `视频路径` would otherwise emit
        // TRY_CAST("视频路径" AS …) AS "视频路径" and DuckDB binder fails.
        let columns = if let Some(cols) = physical_columns.filter(|c| !c.is_empty()) {
            cols.iter()
                .filter(|c| !is_parquet_metadata_column(c))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            discover_parquet_columns(conn, &path_sql)?
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
                    let cast_ty = duck_cast_type(col.type_name.as_str());
                    let alias = quote_ident(col.name.as_str())?;
                    if columns.iter().any(|c| c == physical) {
                        let src = quote_ident(physical)?;
                        Ok(format!("TRY_CAST({src} AS {cast_ty}) AS {alias}"))
                    } else {
                        Ok(format!("CAST(NULL AS {cast_ty}) AS {alias}"))
                    }
                })
                .collect::<Result<Vec<_>>>()?
                .join(", ")
        };

        let view_ident = quote_ident(&view)?;
        let ddl = format!(
            "CREATE OR REPLACE VIEW {view_ident} AS SELECT {select_list} FROM read_parquet({path_sql})"
        );
        conn.execute_batch(&ddl).with_context(|| {
            format!(
                "create parquet view {} path={} ddl={}",
                view,
                abs.display(),
                ddl
            )
        })?;

        let out_columns = if schema.is_empty() {
            columns
        } else {
            schema.iter().map(|c| c.name.clone()).collect()
        };
        Ok((view, out_columns))
    })
}

fn is_parquet_metadata_column(name: &str) -> bool {
    matches!(name, "arrow_schema" | "duckdb_schema")
}

fn discover_parquet_columns(conn: &duckdb::Connection, path_sql: &str) -> Result<Vec<String>> {
    // Prefer parquet_schema(): duckdb-rs `column_names()` panics on some read_parquet prepares.
    let sql = format!("SELECT name FROM parquet_schema({path_sql})");
    let mut stmt = conn
        .prepare(&sql)
        .with_context(|| format!("prepare parquet_schema {path_sql}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .with_context(|| format!("query parquet_schema {path_sql}"))?;
    let mut columns = Vec::new();
    for row in rows {
        let name = row.context("read parquet_schema row")?;
        if !is_parquet_metadata_column(&name) {
            columns.push(name);
        }
    }
    if columns.is_empty() {
        bail!("parquet_schema returned no data columns for {path_sql}");
    }
    Ok(columns)
}
