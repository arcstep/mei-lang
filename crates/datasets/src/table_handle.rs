//! Lazy table handle: DataFusion view over parquet (no whole-table Vec JSON resident).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{Context, Result};
use mei_lang_kernel::{
    resolve_data_snapshot_import_entry, resolve_versioned_source_identifier,
    source_file_content_signature, ColumnSchema,
};
use moka::sync::Cache;
use serde_json::Value;

use crate::query_engine::{
    ensure_parquet_view, query_parquet_page, resolve_parquet_file_for_source, ParquetPageQuery,
};
use crate::types::DatasetQueryOptions;

const TABLE_HANDLE_CACHE_TTL_MS: u64 = 300_000;
const TABLE_HANDLE_CACHE_MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct TableHandle {
    pub columns: Vec<String>,
    /// Registered DataFusion view name (keyed by parquet path hash).
    pub view_name: String,
    pub parquet_path: PathBuf,
    app_root: PathBuf,
}

fn table_handle_cache() -> &'static Cache<String, Arc<TableHandle>> {
    static CACHE: OnceLock<Cache<String, Arc<TableHandle>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Cache::builder()
            .max_capacity(TABLE_HANDLE_CACHE_MAX_BYTES)
            .weigher(|key: &String, handle: &Arc<TableHandle>| {
                key.len()
                    .saturating_add(handle.view_name.len())
                    .saturating_add(handle.parquet_path.to_string_lossy().len())
                    .saturating_add(handle.columns.iter().map(String::len).sum::<usize>())
                    .clamp(1, u32::MAX as usize) as u32
            })
            .time_to_live(Duration::from_millis(TABLE_HANDLE_CACHE_TTL_MS))
            .build()
    })
}

pub fn table_handle_cache_key(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Result<String> {
    let header_row = header_row.max(1);
    if let Some(entry) =
        resolve_data_snapshot_import_entry(app_root, source_path, sheet, header_row)
    {
        return Ok(format!(
            "import:{}|sheet={}|header={}",
            entry.content_signature,
            sheet.unwrap_or("").trim(),
            header_row
        ));
    }
    let resolved = resolve_versioned_source_identifier(app_root, source_path);
    let absolute = app_root.join(&resolved);
    let sig = source_file_content_signature(absolute.as_path(), resolved.as_str());
    Ok(format!(
        "source:{sig}|sheet={}|header={header_row}",
        sheet.unwrap_or("").trim()
    ))
}

pub fn load_table_handle(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Result<(Arc<TableHandle>, bool)> {
    let table_key = table_handle_cache_key(app_root, source_path, sheet, header_row)?;
    if let Some(handle) = take_cached_table_handle(&table_key) {
        return Ok((handle, true));
    }
    let parquet = resolve_parquet_file_for_source(app_root, source_path, sheet, header_row)
        .with_context(|| {
            format!(
                "parquet snapshot missing for `{source_path}` (sheet={:?}, header_row={header_row}); run prebuild/import",
                sheet
            )
        })?;
    let physical_columns =
        resolve_data_snapshot_import_entry(app_root, source_path, sheet, header_row)
            .map(|e| e.columns)
            .filter(|c| !c.is_empty());
    let (view_name, columns) = ensure_parquet_view(
        app_root,
        parquet.as_path(),
        &[],
        physical_columns.as_deref(),
    )?;
    let handle = Arc::new(TableHandle {
        columns,
        view_name,
        parquet_path: parquet,
        app_root: app_root.to_path_buf(),
    });
    store_cached_table_handle(table_key, handle.clone());
    Ok((handle, false))
}

/// Materialize rows via the query engine (DataFusion) (prefer paginated options). Avoids keeping a full Vec on the handle.
pub fn materialize_rows_from_handle(
    handle: &TableHandle,
    schema: &[ColumnSchema],
) -> Result<(Vec<String>, Vec<Value>)> {
    let options = DatasetQueryOptions {
        collect_all: true,
        page: 1,
        page_size: 0,
        ..DatasetQueryOptions::default()
    };
    let page = query_parquet_page(
        handle.app_root.as_path(),
        ParquetPageQuery {
            parquet_path: handle.parquet_path.as_path(),
            schema,
            physical_columns: Some(handle.columns.as_slice()),
            normalize: &BTreeMap::new(),
            options: &options,
        },
    )
    .with_context(|| {
        format!(
            "materialize rows from query-engine view `{}` ({})",
            handle.view_name,
            handle.parquet_path.display()
        )
    })?;
    Ok((page.columns, page.rows))
}

fn take_cached_table_handle(key: &str) -> Option<Arc<TableHandle>> {
    table_handle_cache().get(key)
}

fn store_cached_table_handle(key: String, handle: Arc<TableHandle>) {
    table_handle_cache().insert(key, handle);
}

pub(crate) fn clear_table_handle_cache() -> usize {
    let cache = table_handle_cache();
    let cleared = cache.entry_count() as usize;
    cache.invalidate_all();
    cache.run_pending_tasks();
    cleared
}

pub(crate) fn clear_table_handle_cache_for_app(app_root: &Path) -> usize {
    let canonical = app_root
        .canonicalize()
        .unwrap_or_else(|_| app_root.to_path_buf());
    let cache = table_handle_cache();
    let keys = cache
        .iter()
        .filter_map(|(key, handle)| {
            let entry_root = handle
                .app_root
                .canonicalize()
                .unwrap_or_else(|_| handle.app_root.clone());
            (entry_root == canonical).then(|| key.as_ref().clone())
        })
        .collect::<Vec<_>>();
    for key in &keys {
        cache.invalidate(key);
    }
    cache.run_pending_tasks();
    keys.len()
}
