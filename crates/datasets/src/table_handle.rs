//! Lazy table handle: DuckDB view over parquet (no whole-table Vec JSON resident).

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use mei_lang_kernel::{
    resolve_data_snapshot_import_entry, resolve_versioned_source_identifier,
    source_file_content_signature, ColumnSchema,
};
use serde_json::Value;

use crate::duckdb_engine::{
    ensure_parquet_view, query_parquet_page, resolve_parquet_file_for_source, DuckdbPageQuery,
};
use crate::types::DatasetQueryOptions;

const TABLE_HANDLE_CACHE_TTL_MS: u64 = 300_000;
const TABLE_HANDLE_CACHE_PRUNE_INTERVAL_MS: u64 = 5_000;
const MAX_TABLE_HANDLE_CACHE_ENTRIES: usize = 64;

#[derive(Clone)]
pub struct TableHandle {
    pub columns: Vec<String>,
    /// Registered DuckDB view name (keyed by parquet path hash).
    pub view_name: String,
    pub parquet_path: PathBuf,
    app_root: PathBuf,
}

#[derive(Clone)]
struct CachedTableHandle {
    expires_at: Instant,
    handle: Arc<TableHandle>,
}

#[derive(Default)]
struct TableHandleCacheState {
    entries: BTreeMap<String, CachedTableHandle>,
    lru: VecDeque<String>,
    next_prune_at: Option<Instant>,
}

fn table_handle_cache() -> &'static Mutex<TableHandleCacheState> {
    static CACHE: OnceLock<Mutex<TableHandleCacheState>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(TableHandleCacheState::default()))
}

fn cache_ttl() -> Duration {
    Duration::from_millis(TABLE_HANDLE_CACHE_TTL_MS)
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
    let physical_columns = resolve_data_snapshot_import_entry(app_root, source_path, sheet, header_row)
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

/// Materialize rows via DuckDB (prefer paginated options). Avoids keeping a full Vec on the handle.
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
        DuckdbPageQuery {
            parquet_path: handle.parquet_path.as_path(),
            schema,
            physical_columns: Some(handle.columns.as_slice()),
            normalize: &BTreeMap::new(),
            options: &options,
        },
    )
    .with_context(|| {
        format!(
            "materialize rows from duckdb view `{}` ({})",
            handle.view_name,
            handle.parquet_path.display()
        )
    })?;
    Ok((page.columns, page.rows))
}

fn take_cached_table_handle(key: &str) -> Option<Arc<TableHandle>> {
    let mut guard = table_handle_cache().lock().ok()?;
    maybe_prune_table_handle_cache(&mut guard);
    let cached = guard.entries.get(key)?;
    if cached.expires_at <= Instant::now() {
        guard.entries.remove(key);
        guard.lru.retain(|value| value != key);
        return None;
    }
    let handle = cached.handle.clone();
    if let Some(pos) = guard.lru.iter().position(|value| value == key) {
        guard.lru.remove(pos);
    }
    guard.lru.push_back(key.to_string());
    Some(handle)
}

fn store_cached_table_handle(key: String, handle: Arc<TableHandle>) {
    let Ok(mut guard) = table_handle_cache().lock() else {
        return;
    };
    maybe_prune_table_handle_cache(&mut guard);
    guard.entries.insert(
        key.clone(),
        CachedTableHandle {
            expires_at: Instant::now() + cache_ttl(),
            handle,
        },
    );
    guard.lru.retain(|value| value != &key);
    guard.lru.push_back(key);
    while guard.entries.len() > MAX_TABLE_HANDLE_CACHE_ENTRIES {
        if let Some(oldest) = guard.lru.pop_front() {
            guard.entries.remove(&oldest);
        } else {
            break;
        }
    }
}

fn maybe_prune_table_handle_cache(state: &mut TableHandleCacheState) {
    let now = Instant::now();
    if state
        .next_prune_at
        .is_some_and(|next| now < next && state.entries.len() <= MAX_TABLE_HANDLE_CACHE_ENTRIES)
    {
        return;
    }
    state.entries.retain(|key, entry| {
        if entry.expires_at <= now {
            state.lru.retain(|value| value != key);
            false
        } else {
            true
        }
    });
    state.next_prune_at = Some(now + Duration::from_millis(TABLE_HANDLE_CACHE_PRUNE_INTERVAL_MS));
}

pub(crate) fn clear_table_handle_cache() -> usize {
    let Ok(mut guard) = table_handle_cache().lock() else {
        return 0;
    };
    let cleared = guard.entries.len();
    guard.entries.clear();
    guard.lru.clear();
    guard.next_prune_at = None;
    cleared
}
