use std::{fs, path::Path, sync::Arc, time::Instant};

use anyhow::{Context, Result};
use mei_lang_kernel::SourceDecl;
use serde_json::Value;

use super::file_cache::{
    estimate_dataset_bytes, external_file_cache_key, file_revision, insert_cached_external_dataset,
    should_cache_external_file, try_get_cached_external_dataset, CachedExternalDataset,
    ExternalFileCacheSettings,
};
use super::paginate::{infer_columns, paginate_rows};
use super::paths::resolve_source_path;
use super::types::{DatasetQueryOptions, DatasetQueryResult, SourceMeta};
use super::util::elapsed_ms;

pub(crate) fn query_json_rows(
    app_root: &Path,
    source: &SourceDecl,
    meta: &SourceMeta,
    options: &DatasetQueryOptions,
    cache_settings: &ExternalFileCacheSettings,
) -> Result<DatasetQueryResult> {
    let path = resolve_source_path(app_root, source.path.as_str());
    if let Some(revision) = file_revision(&path) {
        if should_cache_external_file(revision.size_bytes, cache_settings) {
            let cache_key = external_file_cache_key("json", &path, None, None);
            let lookup_started = Instant::now();
            if let Some(cached) = try_get_cached_external_dataset(&cache_key, revision) {
                let lookup_ms = elapsed_ms(lookup_started);
                let paginate_started = Instant::now();
                let mut result = paginate_rows(
                    cached.rows.clone(),
                    &cached.columns,
                    &meta.normalize,
                    options,
                    true,
                );
                result.perf.insert("file_cache_hit".to_string(), 1);
                result
                    .perf
                    .insert("file_cache_lookup_ms".to_string(), lookup_ms);
                result.perf.insert(
                    "file_cache_paginate_ms".to_string(),
                    elapsed_ms(paginate_started),
                );
                return Ok(result);
            }
            let lookup_ms = elapsed_ms(lookup_started);
            let load_started = Instant::now();
            let (columns, rows) = load_json_rows(&path)?;
            let load_ms = elapsed_ms(load_started);
            let dataset = Arc::new(CachedExternalDataset {
                columns: columns.clone(),
                rows: rows.clone(),
            });
            let estimated_bytes = estimate_dataset_bytes(&dataset.columns, &dataset.rows);
            let evicted = insert_cached_external_dataset(
                cache_key.as_str(),
                revision,
                dataset,
                estimated_bytes,
                cache_settings,
            );
            let paginate_started = Instant::now();
            let mut result = paginate_rows(rows, &columns, &meta.normalize, options, true);
            result.perf.insert("file_cache_hit".to_string(), 0);
            result
                .perf
                .insert("file_cache_lookup_ms".to_string(), lookup_ms);
            result
                .perf
                .insert("file_cache_load_ms".to_string(), load_ms);
            result.perf.insert(
                "file_cache_paginate_ms".to_string(),
                elapsed_ms(paginate_started),
            );
            result
                .perf
                .insert("file_cache_evict_count".to_string(), evicted as u64);
            return Ok(result);
        }
    }
    let read_started = Instant::now();
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read json dataset {}", path.display()))?;
    let json: Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid json dataset {}", path.display()))?;
    let read_parse_ms = elapsed_ms(read_started);
    let paginate_started = Instant::now();
    let rows = json.as_array().cloned().unwrap_or_default();
    let columns = infer_columns(&rows);
    let mut result = paginate_rows(rows, &columns, &meta.normalize, options, true);
    result
        .perf
        .insert("json_read_parse_ms".to_string(), read_parse_ms);
    result
        .perf
        .insert("json_paginate_ms".to_string(), elapsed_ms(paginate_started));
    Ok(result)
}

fn load_json_rows(path: &Path) -> Result<(Vec<String>, Vec<Value>)> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read json dataset {}", path.display()))?;
    let json: Value = serde_json::from_str(&raw)
        .with_context(|| format!("invalid json dataset {}", path.display()))?;
    let rows = json.as_array().cloned().unwrap_or_default();
    let columns = infer_columns(&rows);
    Ok((columns, rows))
}
