//! L3：独立数据物化缓存，解耦 scene compile 与 xlsx/csv 读表。

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use serde_json::Value;

use super::decls::LegacySourceDecl;
use super::scene_payload_cache::file_mtime_ms;

#[derive(Debug, Clone)]
pub(super) struct LegacyRowsSnapshot {
    pub rows: Vec<Value>,
    pub truncated: bool,
}

pub const DATASET_MATERIALIZE_CACHE_VERSION: u32 = 1;

pub fn dataset_materialize_cache_epoch() -> String {
    format!("l3v{DATASET_MATERIALIZE_CACHE_VERSION}")
}

static LEGACY_ROWS_CACHE: Mutex<BTreeMap<String, LegacyRowsSnapshot>> = Mutex::new(BTreeMap::new());
const MAX_LEGACY_ROWS_CACHE_ENTRIES: usize = 96;

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
    let data_path = if source_path.is_empty() {
        None
    } else {
        Some(app_root.join(source_path))
    };
    let data_mtime = data_path
        .as_ref()
        .filter(|p| p.is_file())
        .map(|p| file_mtime_ms(p))
        .unwrap_or(0);
    let preview_rows = source.preview_rows.unwrap_or(1000).max(1);
    let header_row = source.header_row.unwrap_or(1).max(1);
    let kind = source.kind.as_deref().unwrap_or("");
    let connection = source.connection.as_deref().unwrap_or("");
    let query = source.query.as_deref().unwrap_or("");
    let table = source.table.as_deref().unwrap_or("");
    let sheet = source.sheet.as_deref().unwrap_or("");
    Some(format!(
        "rows|v{DATASET_MATERIALIZE_CACHE_VERSION}|{}|{source_path}|{data_mtime}|{kind}|{sheet}|{header_row}|{preview_rows}|{connection}|{query}|{table}",
        app_root.display(),
    ))
}

fn store_rows_cache(key: String, snapshot: LegacyRowsSnapshot) {
    if let Ok(mut cache) = LEGACY_ROWS_CACHE.lock() {
        if cache.len() >= MAX_LEGACY_ROWS_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(key, snapshot);
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
            return Ok(snapshot);
        }
        let snapshot = load()?;
        store_rows_cache(key, snapshot.clone());
        return Ok(snapshot);
    }
    load()
}

/// 供测试：清空 L3 缓存。
pub(crate) fn clear_materialize_cache_for_tests() {
    if let Ok(mut c) = LEGACY_ROWS_CACHE.lock() {
        c.clear();
    }
}

pub(crate) fn legacy_rows_cache_len_for_tests() -> usize {
    LEGACY_ROWS_CACHE.lock().map(|c| c.len()).unwrap_or(0)
}
