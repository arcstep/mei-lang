use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use mei_lang_kernel::load_mei_config_for_app;
use serde_json::Value;

#[derive(Debug, Clone)]
pub(crate) struct ExternalFileCacheSettings {
    max_file_bytes: usize,
    max_entries: usize,
    max_total_bytes: usize,
}

impl Default for ExternalFileCacheSettings {
    fn default() -> Self {
        // Prefer DuckDB+parquet for hot paths; keep a small legacy JSON-row
        // fallback cache only for sources without snapshots (0528).
        Self {
            max_file_bytes: 2 * 1024 * 1024,
            max_entries: 16,
            max_total_bytes: 32 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileRevision {
    pub(crate) size_bytes: u64,
    modified_ms: u128,
}

#[derive(Debug, Clone)]
pub(crate) struct CachedExternalDataset {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Value>,
}

#[derive(Debug, Clone)]
struct ExternalFileCacheEntry {
    revision: FileRevision,
    bytes: usize,
    data: Arc<CachedExternalDataset>,
}

#[derive(Debug, Default)]
struct ExternalFileDatasetCache {
    entries: HashMap<String, ExternalFileCacheEntry>,
    lru: VecDeque<String>,
    total_bytes: usize,
}

static EXTERNAL_FILE_CACHE: OnceLock<Mutex<ExternalFileDatasetCache>> = OnceLock::new();

impl ExternalFileDatasetCache {
    fn get(&mut self, key: &str, revision: FileRevision) -> Option<Arc<CachedExternalDataset>> {
        let stale = self
            .entries
            .get(key)
            .map(|entry| entry.revision != revision)
            .unwrap_or(false);
        if stale {
            self.remove_key(key);
            return None;
        }
        let data = self.entries.get(key).map(|entry| entry.data.clone())?;
        self.touch(key);
        Some(data)
    }

    fn insert(
        &mut self,
        key: String,
        revision: FileRevision,
        data: Arc<CachedExternalDataset>,
        bytes: usize,
        settings: &ExternalFileCacheSettings,
    ) -> usize {
        if let Some(previous) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(previous.bytes);
            self.lru.retain(|value| value != &key);
        }
        self.entries.insert(
            key.clone(),
            ExternalFileCacheEntry {
                revision,
                bytes,
                data,
            },
        );
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.touch(&key);
        self.enforce_limits(settings)
    }

    fn touch(&mut self, key: &str) {
        self.lru.retain(|value| value != key);
        self.lru.push_front(key.to_string());
    }

    fn remove_key(&mut self, key: &str) {
        if let Some(removed) = self.entries.remove(key) {
            self.total_bytes = self.total_bytes.saturating_sub(removed.bytes);
        }
        self.lru.retain(|value| value != key);
    }

    fn enforce_limits(&mut self, settings: &ExternalFileCacheSettings) -> usize {
        let mut evicted = 0usize;
        while self.entries.len() > settings.max_entries
            || self.total_bytes > settings.max_total_bytes
        {
            let Some(oldest) = self.lru.pop_back() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.total_bytes = self.total_bytes.saturating_sub(removed.bytes);
                evicted += 1;
            }
        }
        evicted
    }

    fn clear_for_app_root(&mut self, app_root: &Path) -> usize {
        let prefix = app_root.to_string_lossy().replace('\\', "/");
        let keys = self
            .entries
            .keys()
            .filter(|key| key.contains(prefix.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let removed = keys.len();
        for key in keys {
            self.remove_key(key.as_str());
        }
        removed
    }
}

pub(crate) fn resolve_external_file_cache_settings(app_root: &Path) -> ExternalFileCacheSettings {
    let mut settings = ExternalFileCacheSettings::default();
    let cache = load_mei_config_for_app(app_root, None)
        .runtime
        .file_cache
        .to_cache_settings();
    settings.max_file_bytes = cache.max_file_bytes;
    settings.max_entries = cache.max_entries;
    settings.max_total_bytes = cache.max_total_bytes;
    if let Some(value) = env_usize("MEI_FILE_CACHE_MAX_FILE_MB") {
        settings.max_file_bytes = value.saturating_mul(1024 * 1024).max(1);
    }
    if let Some(value) = env_usize("MEI_FILE_CACHE_MAX_ENTRIES") {
        settings.max_entries = value;
    }
    if let Some(value) = env_usize("MEI_FILE_CACHE_MAX_TOTAL_MB") {
        settings.max_total_bytes = value.saturating_mul(1024 * 1024).max(1);
    }
    settings
}

pub(crate) fn should_cache_external_file(
    size_bytes: u64,
    settings: &ExternalFileCacheSettings,
) -> bool {
    size_bytes > 0 && (size_bytes as usize) <= settings.max_file_bytes
}

pub(crate) fn external_file_cache_key(
    kind: &str,
    path: &Path,
    sheet: Option<&str>,
    header_row: Option<i64>,
) -> String {
    let normalized_path = path.to_string_lossy().replace('\\', "/");
    let sheet = sheet.unwrap_or("");
    let header_row = header_row.unwrap_or(1);
    format!("{kind}|{normalized_path}|sheet={sheet}|header_row={header_row}")
}

pub(crate) fn file_revision(path: &Path) -> Option<FileRevision> {
    let meta = fs::metadata(path).ok()?;
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(unix_timestamp_ms)
        .unwrap_or(0);
    Some(FileRevision {
        size_bytes: meta.len(),
        modified_ms,
    })
}

pub(crate) fn try_get_cached_external_dataset(
    key: &str,
    revision: FileRevision,
) -> Option<Arc<CachedExternalDataset>> {
    let Ok(mut cache) = external_file_cache().lock() else {
        tracing::warn!("external file cache lock poisoned on get");
        return None;
    };
    cache.get(key, revision)
}

pub(crate) fn insert_cached_external_dataset(
    key: &str,
    revision: FileRevision,
    data: Arc<CachedExternalDataset>,
    bytes: usize,
    settings: &ExternalFileCacheSettings,
) -> usize {
    if settings.max_entries == 0 || settings.max_total_bytes == 0 || bytes == 0 {
        return 0;
    }
    if bytes > settings.max_total_bytes {
        return 0;
    }
    let Ok(mut cache) = external_file_cache().lock() else {
        tracing::warn!("external file cache lock poisoned on insert");
        return 0;
    };
    cache.insert(key.to_string(), revision, data, bytes, settings)
}

pub(crate) fn estimate_dataset_bytes(columns: &[String], rows: &[Value]) -> usize {
    let columns_bytes: usize = columns.iter().map(|value| value.len()).sum();
    let rows_bytes: usize = rows.iter().map(estimate_value_bytes).sum();
    columns_bytes.saturating_add(rows_bytes)
}

pub(crate) fn clear_external_file_cache_for_app(app_root: &Path) -> usize {
    let Ok(mut cache) = external_file_cache().lock() else {
        tracing::warn!("external file cache lock poisoned on app clear");
        return 0;
    };
    cache.clear_for_app_root(app_root)
}

fn external_file_cache() -> &'static Mutex<ExternalFileDatasetCache> {
    EXTERNAL_FILE_CACHE.get_or_init(|| Mutex::new(ExternalFileDatasetCache::default()))
}

fn env_usize(key: &str) -> Option<usize> {
    std::env::var(key).ok()?.trim().parse::<usize>().ok()
}

fn estimate_value_bytes(value: &Value) -> usize {
    match value {
        Value::Null => 1,
        Value::Bool(_) => 1,
        Value::Number(_) => 8,
        Value::String(text) => text.len(),
        Value::Array(items) => items.iter().map(estimate_value_bytes).sum(),
        Value::Object(map) => map
            .iter()
            .map(|(key, val)| key.len().saturating_add(estimate_value_bytes(val)))
            .sum(),
    }
}

fn unix_timestamp_ms(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis())
}
