//! App-level cache generation tokens for idempotent eval dedup and data-source invalidation.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::mei_config::RuntimeConfig;
use crate::model::DatasetView;

pub const CACHE_GENERATION_SCHEMA_VERSION: &str = "mei-cache-generation-v1";
pub const DEFAULT_DATABASE_TTL_MS: u64 = 43_200_000; // 12 hours
pub const CACHE_GENERATION_REL: &str = "var/active/cache-generation.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheGenerationRecord {
    #[serde(default, rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(default, rename = "appId")]
    pub app_id: String,
    #[serde(default, rename = "dataGeneration")]
    pub data_generation: String,
    #[serde(default)]
    pub sources: BTreeMap<String, SourceGenerationRecord>,
    #[serde(default, rename = "updatedAtMs")]
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceGenerationRecord {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub generation: String,
    #[serde(default, rename = "bumpedAtMs")]
    pub bumped_at_ms: u64,
    #[serde(default, rename = "ttlMs")]
    pub ttl_ms: Option<u64>,
}

impl Default for CacheGenerationRecord {
    fn default() -> Self {
        Self {
            schema_version: CACHE_GENERATION_SCHEMA_VERSION.to_string(),
            app_id: String::new(),
            data_generation: "gen:0".to_string(),
            sources: BTreeMap::new(),
            updated_at_ms: 0,
        }
    }
}

pub fn cache_generation_path(app_root: &Path) -> PathBuf {
    crate::mei_config::resolve_app_var_root(app_root).join("cache-generation.json")
}

pub fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn load_cache_generation(app_root: &Path, app_id: &str) -> CacheGenerationRecord {
    let path = cache_generation_path(app_root);
    let Ok(raw) = fs::read_to_string(&path) else {
        return default_record(app_id);
    };
    serde_json::from_str(&raw).unwrap_or_else(|_| default_record(app_id))
}

pub fn save_cache_generation(app_root: &Path, record: &CacheGenerationRecord) -> Result<()> {
    let path = cache_generation_path(app_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create cache generation dir {}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(record)?)
        .with_context(|| format!("write cache generation tmp {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("rename cache generation {}", path.display()))?;
    Ok(())
}

fn default_record(app_id: &str) -> CacheGenerationRecord {
    CacheGenerationRecord {
        schema_version: CACHE_GENERATION_SCHEMA_VERSION.to_string(),
        app_id: app_id.to_string(),
        data_generation: "gen:0".to_string(),
        sources: BTreeMap::new(),
        updated_at_ms: current_time_ms(),
    }
}

fn next_generation_token(prefix: &str) -> String {
    format!("{prefix}:{}", current_time_ms())
}

fn source_mode_for_dataset(dataset: &DatasetView, runtime: &RuntimeConfig) -> String {
    let kind = dataset.source.kind.trim().to_ascii_lowercase();
    if kind.contains("db") || kind.contains("sql") || kind == "database" {
        "ttl".to_string()
    } else if runtime
        .cache_generation
        .sources
        .file
        .mode
        .trim()
        .eq_ignore_ascii_case("manual_reload")
        || runtime.cache_generation.sources.file.mode.is_empty()
    {
        "manual_reload".to_string()
    } else {
        runtime.cache_generation.sources.file.mode.clone()
    }
}

fn source_ttl_ms(dataset: &DatasetView, runtime: &RuntimeConfig) -> u64 {
    let kind = dataset.source.kind.trim().to_ascii_lowercase();
    if kind.contains("db") || kind.contains("sql") || kind == "database" {
        runtime
            .cache_generation
            .sources
            .database
            .ttl_ms
            .unwrap_or(DEFAULT_DATABASE_TTL_MS)
    } else {
        runtime.cache_generation.sources.file.ttl_ms.unwrap_or(0)
    }
}

fn effective_source_generation(
    record: &CacheGenerationRecord,
    dataset_id: &str,
    dataset: &DatasetView,
    runtime: &RuntimeConfig,
    now_ms: u64,
) -> String {
    let mode = source_mode_for_dataset(dataset, runtime);
    let entry = record.sources.get(dataset_id);
    if mode == "ttl" {
        let ttl_ms = source_ttl_ms(dataset, runtime).max(1);
        let bucket = now_ms / ttl_ms;
        return format!("ttl:{dataset_id}:{bucket}");
    }
    entry
        .map(|value| value.generation.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "gen:0".to_string())
}

/// Resolve the app-wide data generation fingerprint used in idempotent cache keys.
pub fn resolve_app_data_generation(
    app_root: &Path,
    app_id: &str,
    datasets: &[&DatasetView],
    runtime: &RuntimeConfig,
) -> String {
    let mut record = load_cache_generation(app_root, app_id);
    let now_ms = current_time_ms();
    let mut parts = Vec::new();
    for dataset in datasets {
        parts.push(effective_source_generation(
            &record,
            dataset.id.as_str(),
            dataset,
            runtime,
            now_ms,
        ));
    }
    parts.sort();
    parts.dedup();
    let composite = if parts.is_empty() {
        record.data_generation.clone()
    } else {
        format!("{}|{}", record.data_generation, parts.join("|"))
    };
    if record.app_id.is_empty() {
        record.app_id = app_id.to_string();
        record.updated_at_ms = now_ms;
        let _ = save_cache_generation(app_root, &record);
    }
    composite
}

/// Bump global and optional per-source generations after manual data reload.
pub fn bump_cache_generation(
    app_root: &Path,
    app_id: &str,
    source_ids: Option<&[String]>,
) -> Result<CacheGenerationRecord> {
    let mut record = load_cache_generation(app_root, app_id);
    record.app_id = app_id.to_string();
    record.data_generation = next_generation_token("gen");
    record.updated_at_ms = current_time_ms();
    if let Some(ids) = source_ids {
        for source_id in ids {
            let id = source_id.trim();
            if id.is_empty() {
                continue;
            }
            record.sources.insert(
                id.to_string(),
                SourceGenerationRecord {
                    mode: "manual_reload".to_string(),
                    generation: next_generation_token("src"),
                    bumped_at_ms: record.updated_at_ms,
                    ttl_ms: None,
                },
            );
        }
    }
    save_cache_generation(app_root, &record)?;
    Ok(record)
}

pub fn is_file_source_dataset(dataset: &DatasetView) -> bool {
    let kind = dataset.source.kind.trim().to_ascii_lowercase();
    !(kind.contains("db") || kind.contains("sql") || kind == "database")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_changes_global_generation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let app_root = dir.path();
        let before = load_cache_generation(app_root, "demo");
        let after = bump_cache_generation(app_root, "demo", None).expect("bump");
        assert_ne!(before.data_generation, after.data_generation);
    }
}
