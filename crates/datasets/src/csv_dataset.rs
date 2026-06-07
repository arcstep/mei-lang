use std::{path::Path, sync::Arc, time::Instant};

use anyhow::{Context, Result};
use mei_lang_kernel::SourceDecl;
use serde_json::Value;

use super::file_cache::{
    estimate_dataset_bytes, external_file_cache_key, file_revision, insert_cached_external_dataset,
    should_cache_external_file, try_get_cached_external_dataset, CachedExternalDataset,
    ExternalFileCacheSettings,
};
use super::paginate::{apply_normalize, output_columns, paginate_rows, row_matches, QueryWindow};
use super::paths::resolve_source_path;
use super::types::{DatasetQueryOptions, DatasetQueryResult, SourceMeta};
use super::util::elapsed_ms;

pub(crate) fn query_csv_rows(
    app_root: &Path,
    source: &SourceDecl,
    meta: &SourceMeta,
    options: &DatasetQueryOptions,
    cache_settings: &ExternalFileCacheSettings,
) -> Result<DatasetQueryResult> {
    let path = resolve_source_path(app_root, source.path.as_str());
    if let Some(revision) = file_revision(&path) {
        if should_cache_external_file(revision.size_bytes, cache_settings) {
            let cache_key = external_file_cache_key("csv", &path, None, None);
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
            let (headers, rows) = load_csv_rows(&path)?;
            let load_ms = elapsed_ms(load_started);
            let dataset = Arc::new(CachedExternalDataset {
                columns: headers.clone(),
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
            let mut result = paginate_rows(rows, &headers, &meta.normalize, options, true);
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
    if !options.sort.is_empty() {
        let load_started = Instant::now();
        let (headers, rows) = load_csv_rows(&path)?;
        let load_ms = elapsed_ms(load_started);
        let paginate_started = Instant::now();
        let mut result = paginate_rows(rows, &headers, &meta.normalize, options, true);
        result.perf.insert("csv_full_load_ms".to_string(), load_ms);
        result.perf.insert(
            "csv_sort_paginate_ms".to_string(),
            elapsed_ms(paginate_started),
        );
        return Ok(result);
    }
    let open_started = Instant::now();
    let mut reader = csv::Reader::from_path(&path)
        .with_context(|| format!("failed to open dataset {}", path.display()))?;
    let headers = reader
        .headers()
        .context("failed to read csv headers")?
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let open_ms = elapsed_ms(open_started);
    let scan_started = Instant::now();
    let mut window = QueryWindow::new(options);
    for record in reader.records() {
        if window.should_stop() {
            break;
        }
        let record = record.context("failed to read csv row")?;
        let mut map = serde_json::Map::new();
        for (idx, header) in headers.iter().enumerate() {
            map.insert(
                header.clone(),
                Value::String(record.get(idx).unwrap_or_default().to_string()),
            );
        }
        let normalized = apply_normalize(Value::Object(map), &meta.normalize);
        if row_matches(&normalized, &options.filters, options.search.as_deref()) {
            window.push(normalized);
        }
    }
    let mut result = window.finish(output_columns(&headers, &meta.normalize), true);
    result.perf.insert("csv_open_ms".to_string(), open_ms);
    result
        .perf
        .insert("csv_scan_filter_ms".to_string(), elapsed_ms(scan_started));
    Ok(result)
}

fn load_csv_rows(path: &Path) -> Result<(Vec<String>, Vec<Value>)> {
    let mut reader = csv::Reader::from_path(path)
        .with_context(|| format!("failed to open dataset {}", path.display()))?;
    let headers = reader
        .headers()
        .context("failed to read csv headers")?
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.context("failed to read csv row")?;
        let mut map = serde_json::Map::new();
        for (idx, header) in headers.iter().enumerate() {
            map.insert(
                header.clone(),
                Value::String(record.get(idx).unwrap_or_default().to_string()),
            );
        }
        rows.push(Value::Object(map));
    }
    Ok((headers, rows))
}
