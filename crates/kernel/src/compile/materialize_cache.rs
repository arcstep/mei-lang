//! L3：独立数据物化缓存，解耦 scene compile 与 xlsx/csv 读表。

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use serde_json::Value;

use super::data_snapshot::{
    access_parquet_import_required, parquet_sidecar_write_allowed, source_file_content_signature,
    try_load_xlsx_parquet_snapshot, write_xlsx_parquet_snapshot,
};
use super::decls::LegacySourceDecl;
use super::loaders::{load_xlsx_table_snapshot, XlsxTableSnapshot};
use super::scene_payload_cache::file_mtime_ms;
use super::xlsx_singleflight::{
    finish_xlsx_inflight, register_xlsx_inflight, wait_for_xlsx_inflight, xlsx_singleflight_enabled,
};
use crate::resolve_versioned_source_identifier;

#[derive(Debug, Clone)]
pub struct LegacyRowsSnapshot {
    pub rows: Vec<Value>,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub enum TableSnapshot {
    LegacyRows(LegacyRowsSnapshot),
    Xlsx(Arc<XlsxTableSnapshot>),
}

#[derive(Debug, Clone)]
pub struct TableSnapshotKey {
    pub source_kind: String,
    pub resolved_identifier: String,
    pub data_mtime: u128,
    pub sheet: String,
    pub header_row: usize,
}

pub const DATASET_MATERIALIZE_CACHE_VERSION: u32 = 3;

pub fn dataset_materialize_cache_epoch() -> String {
    format!("l3v{DATASET_MATERIALIZE_CACHE_VERSION}")
}

static LEGACY_ROWS_CACHE: Mutex<BTreeMap<String, LegacyRowsSnapshot>> = Mutex::new(BTreeMap::new());
static LEGACY_ROWS_CACHE_LRU: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static XLSX_TABLE_SNAPSHOT_CACHE: Mutex<BTreeMap<String, Arc<XlsxTableSnapshot>>> =
    Mutex::new(BTreeMap::new());
static XLSX_TABLE_SNAPSHOT_CACHE_LRU: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static LEGACY_ROWS_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static LEGACY_ROWS_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
const MAX_LEGACY_ROWS_CACHE_ENTRIES: usize = 96;
const MAX_XLSX_TABLE_SNAPSHOT_CACHE_ENTRIES: usize = 64;

fn touch_lru(lru: &mut VecDeque<String>, key: &str) {
    lru.retain(|value| value != key);
    lru.push_front(key.to_string());
}

fn evict_lru<V>(cache: &mut BTreeMap<String, V>, lru: &mut VecDeque<String>, max_entries: usize) {
    while cache.len() > max_entries {
        let Some(oldest) = lru.pop_back() else {
            break;
        };
        cache.remove(&oldest);
    }
}

fn resolve_table_snapshot_key(
    app_root: &Path,
    source_kind: &str,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Option<TableSnapshotKey> {
    let source_path = source_path.trim();
    if source_path.is_empty() {
        return None;
    }
    let resolved_identifier = resolve_versioned_source_identifier(app_root, source_path);
    let absolute_path = app_root.join(&resolved_identifier);
    let data_mtime = if absolute_path.is_file() {
        file_mtime_ms(&absolute_path)
    } else {
        0
    };
    Some(TableSnapshotKey {
        source_kind: source_kind.trim().to_string(),
        resolved_identifier,
        data_mtime,
        sheet: sheet.unwrap_or("").trim().to_string(),
        header_row: header_row.max(1),
    })
}

fn source_rows_cache_key(app_root: &Path, source: &LegacySourceDecl) -> Option<String> {
    let source_path = source
        .file
        .as_deref()
        .or(source.path.as_deref())
        .unwrap_or("")
        .trim();
    if source_path.is_empty() && source.connection.is_none() {
        return None;
    }
    let resolved_path = if source_path.is_empty() {
        String::new()
    } else {
        resolve_versioned_source_identifier(app_root, source_path)
    };
    let data_path = if resolved_path.is_empty() {
        None
    } else {
        Some(app_root.join(&resolved_path))
    };
    let data_mtime = data_path
        .as_ref()
        .filter(|p| p.is_file())
        .map(|p| file_mtime_ms(p))
        .unwrap_or(0);
    let preview_rows = source.preview_rows.unwrap_or(1000).max(1);
    let header_row = source.header_row.unwrap_or(1).max(1) as usize;
    let kind = source.kind.as_deref().unwrap_or("");
    let connection = source.connection.as_deref().unwrap_or("");
    let query = source.query.as_deref().unwrap_or("");
    let table = source.table.as_deref().unwrap_or("");
    let sheet = source.sheet.as_deref().unwrap_or("");
    let table_key = resolve_table_snapshot_key(
        app_root,
        kind,
        source_path,
        source.sheet.as_deref(),
        header_row,
    );
    let (resolved_identifier, normalized_mtime, normalized_sheet, normalized_header_row) =
        if let Some(key) = table_key {
            (
                key.resolved_identifier,
                key.data_mtime,
                key.sheet,
                key.header_row,
            )
        } else {
            (resolved_path, data_mtime, sheet.to_string(), header_row)
        };
    Some(format!(
        "rows|v{DATASET_MATERIALIZE_CACHE_VERSION}|{}|{resolved_identifier}|{normalized_mtime}|{kind}|{normalized_sheet}|{normalized_header_row}|{preview_rows}|{connection}|{query}|{table}",
        app_root.display(),
    ))
}

fn store_rows_cache(key: String, snapshot: LegacyRowsSnapshot) {
    if let Ok(mut cache) = LEGACY_ROWS_CACHE.lock() {
        if let Ok(mut lru) = LEGACY_ROWS_CACHE_LRU.lock() {
            lru.retain(|value| value != &key);
            cache.insert(key.clone(), snapshot);
            touch_lru(&mut lru, key.as_str());
            evict_lru(&mut cache, &mut lru, MAX_LEGACY_ROWS_CACHE_ENTRIES);
        }
    }
}

fn take_rows_cache(key: &str) -> Option<LegacyRowsSnapshot> {
    LEGACY_ROWS_CACHE
        .lock()
        .ok()
        .and_then(|c| c.get(key).cloned())
}

pub(super) fn cached_load_legacy_rows_from_source(
    app_root: &Path,
    source: &LegacySourceDecl,
    load: impl FnOnce() -> Result<LegacyRowsSnapshot>,
) -> Result<LegacyRowsSnapshot> {
    if let Some(key) = source_rows_cache_key(app_root, source) {
        if let Some(snapshot) = take_rows_cache(&key) {
            LEGACY_ROWS_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
            return Ok(snapshot);
        }
        LEGACY_ROWS_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
        let snapshot = load()?;
        store_rows_cache(key, snapshot.clone());
        return Ok(snapshot);
    }
    LEGACY_ROWS_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    load()
}

fn xlsx_table_snapshot_cache_key(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Option<String> {
    let key = resolve_table_snapshot_key(app_root, "xlsx", source_path, sheet, header_row)?;
    let absolute_path = app_root.join(&key.resolved_identifier);
    let content_sig = if absolute_path.is_file() {
        source_file_content_signature(absolute_path.as_path(), key.resolved_identifier.as_str())
    } else {
        "missing".to_string()
    };
    let parquet_token = super::data_snapshot::parquet_snapshot_cache_token(
        app_root,
        source_path,
        sheet,
        header_row,
    );
    Some(format!(
        "xlsx-table|v{DATASET_MATERIALIZE_CACHE_VERSION}|{}|{}|{}|{}|{}|{content_sig}|{parquet_token}",
        app_root.display(),
        key.resolved_identifier,
        key.data_mtime,
        key.sheet,
        key.header_row
    ))
}

fn store_xlsx_table_snapshot_cache(key: &str, snapshot: Arc<XlsxTableSnapshot>) {
    if let Ok(mut cache) = XLSX_TABLE_SNAPSHOT_CACHE.lock() {
        if let Ok(mut lru) = XLSX_TABLE_SNAPSHOT_CACHE_LRU.lock() {
            lru.retain(|value| value != key);
            cache.insert(key.to_string(), snapshot);
            touch_lru(&mut lru, key);
            evict_lru(&mut cache, &mut lru, MAX_XLSX_TABLE_SNAPSHOT_CACHE_ENTRIES);
        }
    }
}

fn take_xlsx_table_snapshot_cache(key: &str) -> Option<Arc<XlsxTableSnapshot>> {
    let snapshot = XLSX_TABLE_SNAPSHOT_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(key).cloned())?;
    if let Ok(mut lru) = XLSX_TABLE_SNAPSHOT_CACHE_LRU.lock() {
        touch_lru(&mut lru, key);
    }
    Some(snapshot)
}

fn parquet_sidecar_write_enabled() -> bool {
    parquet_sidecar_write_allowed()
}

fn load_xlsx_table_snapshot_arc(
    app_root: &Path,
    absolute_path: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Result<Arc<XlsxTableSnapshot>> {
    if let Some(snapshot) = try_load_xlsx_parquet_snapshot(app_root, source_path, sheet, header_row)
    {
        return Ok(Arc::new(snapshot));
    }
    if access_parquet_import_required() {
        anyhow::bail!(
            "missing parquet data snapshot import for `{source_path}` (sheet={:?}, header_row={header_row}); run `mei-toolchain prebuild` or publish data snapshots before serving access traffic",
            sheet
        );
    }
    let snapshot =
        load_xlsx_table_snapshot(absolute_path, source_path, sheet, header_row.max(1), None)?;
    if parquet_sidecar_write_enabled() {
        let _ = write_xlsx_parquet_snapshot(app_root, source_path, sheet, header_row);
    }
    Ok(Arc::new(snapshot))
}

fn cached_load_xlsx_table_snapshot_with_key(
    app_root: &Path,
    key: &str,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Result<(Arc<XlsxTableSnapshot>, bool)> {
    if let Some(snapshot) = take_xlsx_table_snapshot_cache(key) {
        XLSX_TABLE_SNAPSHOT_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
        return Ok((snapshot, true));
    }

    if xlsx_singleflight_enabled() {
        if let Some((entry, is_leader)) = register_xlsx_inflight(key) {
            if !is_leader {
                let snapshot = wait_for_xlsx_inflight(&entry)?;
                return Ok((snapshot, true));
            }

            // Leader: double-check cache after registration.
            if let Some(snapshot) = take_xlsx_table_snapshot_cache(key) {
                finish_xlsx_inflight(key, &entry, Ok(snapshot.clone()));
                return Ok((snapshot, true));
            }

            let resolved_identifier = resolve_versioned_source_identifier(app_root, source_path);
            let absolute_path = app_root.join(&resolved_identifier);
            let load_result = load_xlsx_table_snapshot_arc(
                app_root,
                absolute_path.as_path(),
                source_path,
                sheet,
                header_row,
            )
            .map_err(|error| error.to_string());
            match load_result {
                Ok(snapshot) => {
                    store_xlsx_table_snapshot_cache(key, snapshot.clone());
                    finish_xlsx_inflight(key, &entry, Ok(snapshot.clone()));
                    return Ok((snapshot, false));
                }
                Err(error) => {
                    finish_xlsx_inflight(key, &entry, Err(error.clone()));
                    return Err(anyhow::anyhow!(error));
                }
            }
        }
    }

    let resolved_identifier = resolve_versioned_source_identifier(app_root, source_path);
    let absolute_path = app_root.join(&resolved_identifier);
    let snapshot = load_xlsx_table_snapshot_arc(
        app_root,
        absolute_path.as_path(),
        source_path,
        sheet,
        header_row,
    )?;
    store_xlsx_table_snapshot_cache(key, snapshot.clone());
    Ok((snapshot, false))
}

pub fn cached_load_xlsx_table_snapshot(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Result<(Arc<XlsxTableSnapshot>, bool)> {
    let Some(key) = xlsx_table_snapshot_cache_key(app_root, source_path, sheet, header_row) else {
        let snapshot = load_xlsx_table_snapshot(
            &app_root.join(source_path),
            source_path,
            sheet,
            header_row.max(1),
            None,
        )?;
        return Ok((Arc::new(snapshot), false));
    };
    cached_load_xlsx_table_snapshot_with_key(app_root, &key, source_path, sheet, header_row)
}

pub fn try_get_cached_xlsx_table_snapshot(
    app_root: &Path,
    source_path: &str,
    sheet: Option<&str>,
    header_row: usize,
) -> Option<Arc<XlsxTableSnapshot>> {
    let key = xlsx_table_snapshot_cache_key(app_root, source_path, sheet, header_row)?;
    XLSX_TABLE_SNAPSHOT_CACHE
        .lock()
        .ok()
        .and_then(|cache| cache.get(&key).cloned())
}

pub(crate) fn dataset_materialize_cache_metrics_snapshot() -> (u64, u64) {
    (
        LEGACY_ROWS_CACHE_HITS.load(Ordering::Relaxed),
        LEGACY_ROWS_CACHE_MISSES.load(Ordering::Relaxed),
    )
}

pub fn dataset_materialize_cache_hit_count() -> u64 {
    LEGACY_ROWS_CACHE_HITS.load(Ordering::Relaxed)
        + XLSX_TABLE_SNAPSHOT_CACHE_HITS.load(Ordering::Relaxed)
}

static XLSX_TABLE_SNAPSHOT_CACHE_HITS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn clear_materialize_cache() {
    if let Ok(mut c) = LEGACY_ROWS_CACHE.lock() {
        c.clear();
    }
    if let Ok(mut lru) = LEGACY_ROWS_CACHE_LRU.lock() {
        lru.clear();
    }
    if let Ok(mut c) = XLSX_TABLE_SNAPSHOT_CACHE.lock() {
        c.clear();
    }
    if let Ok(mut lru) = XLSX_TABLE_SNAPSHOT_CACHE_LRU.lock() {
        lru.clear();
    }
    super::xlsx_singleflight::clear_xlsx_inflight_for_tests();
}
