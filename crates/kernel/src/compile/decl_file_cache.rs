use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use anyhow::Result;
use serde_json::Value;

use crate::eval::evaluate_mei_file;

use super::scene_payload_cache::file_mtime_ms;

static DECL_FILE_CACHE: Mutex<BTreeMap<String, Value>> = Mutex::new(BTreeMap::new());
static DECL_FILE_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static DECL_FILE_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
const MAX_DECL_FILE_CACHE_ENTRIES: usize = 512;

fn decl_file_cache_key(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    Some(format!("{}|{}", path.display(), file_mtime_ms(path)))
}

pub(super) fn evaluate_mei_file_cached(path: &Path) -> Result<Value> {
    let Some(key) = decl_file_cache_key(path) else {
        DECL_FILE_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
        return evaluate_mei_file(path);
    };
    if let Ok(cache) = DECL_FILE_CACHE.lock() {
        if let Some(value) = cache.get(&key).cloned() {
            DECL_FILE_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
            return Ok(value);
        }
    }
    DECL_FILE_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
    let value = evaluate_mei_file(path)?;
    if let Ok(mut cache) = DECL_FILE_CACHE.lock() {
        if cache.len() >= MAX_DECL_FILE_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(key, value.clone());
    }
    Ok(value)
}

pub(super) fn decl_file_cache_metrics_snapshot() -> (u64, u64) {
    (
        DECL_FILE_CACHE_HITS.load(Ordering::Relaxed),
        DECL_FILE_CACHE_MISSES.load(Ordering::Relaxed),
    )
}

#[cfg(test)]
pub(crate) fn decl_file_cache_metrics_snapshot_for_tests() -> (u64, u64) {
    decl_file_cache_metrics_snapshot()
}

#[cfg(test)]
pub(crate) fn clear_decl_file_cache_for_tests() {
    if let Ok(mut cache) = DECL_FILE_CACHE.lock() {
        cache.clear();
    }
}
