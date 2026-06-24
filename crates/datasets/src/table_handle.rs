//! Columnar table handle backed by parquet import artifacts (LRU + TTL).

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use mei_lang_kernel::{
    resolve_data_snapshot_import_entry, resolve_versioned_source_identifier,
    source_file_content_signature, try_load_xlsx_parquet_snapshot,
};
use serde_json::Value;

const TABLE_HANDLE_CACHE_TTL_MS: u64 = 300_000;
const TABLE_HANDLE_CACHE_PRUNE_INTERVAL_MS: u64 = 5_000;
const MAX_TABLE_HANDLE_CACHE_ENTRIES: usize = 64;

#[derive(Clone)]
pub struct TableHandle {
    pub columns: Vec<String>,
    rows: Vec<Value>,
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
    let snapshot = try_load_xlsx_parquet_snapshot(app_root, source_path, sheet, header_row)
        .with_context(|| {
            format!(
                "load parquet table handle for `{source_path}` (sheet={:?}, header_row={header_row})",
                sheet
            )
        })?;
    let handle = Arc::new(TableHandle {
        columns: snapshot.columns,
        rows: snapshot.rows,
    });
    store_cached_table_handle(table_key, handle.clone());
    Ok((handle, false))
}

pub fn materialize_rows_from_handle(
    handle: &TableHandle,
    schema: &[mei_lang_kernel::ColumnSchema],
) -> (Vec<String>, Vec<Value>) {
    let columns = if handle.columns.is_empty() {
        schema.iter().map(|col| col.name.clone()).collect()
    } else {
        handle.columns.clone()
    };
    let rows = if schema.is_empty() {
        handle.rows.clone()
    } else {
        handle
            .rows
            .iter()
            .map(|row| mei_lang_kernel::coerce_row_to_schema(row, schema))
            .collect()
    };
    (columns, rows)
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
