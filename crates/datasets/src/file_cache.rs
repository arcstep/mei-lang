use std::{
    fs,
    path::Path,
    sync::Arc,
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
        // Prefer DataFusion+parquet for hot paths; keep a small legacy JSON-row
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
    let _legacy_limits = (
        size_bytes,
        settings.max_file_bytes,
        settings.max_entries,
        settings.max_total_bytes,
    );
    false
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
    _key: &str,
    _revision: FileRevision,
) -> Option<Arc<CachedExternalDataset>> {
    None
}

pub(crate) fn insert_cached_external_dataset(
    _key: &str,
    _revision: FileRevision,
    _data: Arc<CachedExternalDataset>,
    _bytes: usize,
    _settings: &ExternalFileCacheSettings,
) -> usize {
    0
}

pub(crate) fn estimate_dataset_bytes(columns: &[String], rows: &[Value]) -> usize {
    let columns_bytes: usize = columns.iter().map(|value| value.len()).sum();
    let rows_bytes: usize = rows.iter().map(estimate_value_bytes).sum();
    columns_bytes.saturating_add(rows_bytes)
}

pub(crate) fn clear_external_file_cache_for_app(_app_root: &Path) -> usize {
    0
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
