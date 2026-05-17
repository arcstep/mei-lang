use std::{path::Path, sync::Arc, time::Instant};

use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook, Reader, Xls, Xlsx};
use mei_lang_kernel::SourceDecl;
use serde_json::Value;

use super::file_cache::{
    estimate_dataset_bytes, external_file_cache_key, file_revision, insert_cached_external_dataset,
    should_cache_external_file, try_get_cached_external_dataset, CachedExternalDataset,
    ExternalFileCacheSettings,
};
use super::paginate::{apply_normalize, empty_result, paginate_rows, row_matches, QueryWindow};
use super::paths::resolve_source_path;
use super::types::{DatasetQueryOptions, DatasetQueryResult, SourceMeta};
use super::util::elapsed_ms;
use super::xlsx_format::{xlsx_cell, xlsx_header};

pub(crate) fn query_xlsx_rows(
    app_root: &Path,
    source: &SourceDecl,
    meta: &SourceMeta,
    options: &DatasetQueryOptions,
    cache_settings: &ExternalFileCacheSettings,
) -> Result<DatasetQueryResult> {
    let path = resolve_source_path(app_root, source.path.as_str());
    let sheet = meta.sheet.as_deref();
    let header_row = meta.header_row.unwrap_or(1).max(1) as usize;
    if let Some(revision) = file_revision(&path) {
        if should_cache_external_file(revision.size_bytes, cache_settings) {
            let cache_key =
                external_file_cache_key("xlsx", &path, meta.sheet.as_deref(), meta.header_row);
            let lookup_started = Instant::now();
            if let Some(cached) = try_get_cached_external_dataset(&cache_key, revision) {
                let lookup_ms = elapsed_ms(lookup_started);
                let paginate_started = Instant::now();
                let mut result =
                    paginate_rows(cached.rows.clone(), &cached.columns, &meta.normalize, options, true);
                result.perf.insert("file_cache_hit".to_string(), 1);
                result
                    .perf
                    .insert("file_cache_lookup_ms".to_string(), lookup_ms);
                result
                    .perf
                    .insert("file_cache_paginate_ms".to_string(), elapsed_ms(paginate_started));
                return Ok(result);
            }
            let lookup_ms = elapsed_ms(lookup_started);
            let load_started = Instant::now();
            let (columns, rows) = load_xlsx_rows(&path, source.path.ends_with(".xls"), sheet, header_row)?;
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
            result
                .perf
                .insert("file_cache_paginate_ms".to_string(), elapsed_ms(paginate_started));
            result
                .perf
                .insert("file_cache_evict_count".to_string(), evicted as u64);
            return Ok(result);
        }
    }
    let open_started = Instant::now();
    if source.path.ends_with(".xls") {
        let mut workbook: Xls<_> = open_workbook(&path)
            .with_context(|| format!("failed to open legacy xls {}", path.display()))?;
        let mut result =
            paginate_xlsx_reader(&mut workbook, sheet, header_row, &meta.normalize, options)?;
        result
            .perf
            .insert("xlsx_open_ms".to_string(), elapsed_ms(open_started));
        return Ok(result);
    }
    let mut workbook: Xlsx<_> = open_workbook(&path)
        .with_context(|| format!("failed to open xlsx {}", path.display()))?;
    let mut result =
        paginate_xlsx_reader(&mut workbook, sheet, header_row, &meta.normalize, options)?;
    result
        .perf
        .insert("xlsx_open_ms".to_string(), elapsed_ms(open_started));
    Ok(result)
}

pub(crate) fn paginate_xlsx_reader<R, RS>(
    workbook: &mut R,
    sheet: Option<&str>,
    header_row: usize,
    normalize: &std::collections::BTreeMap<String, String>,
    options: &DatasetQueryOptions,
) -> Result<DatasetQueryResult>
where
    R: Reader<RS>,
    RS: std::io::Read + std::io::Seek,
    <R as Reader<RS>>::Error: std::fmt::Display,
{
    let scan_started = Instant::now();
    let sheet_name = if let Some(name) = sheet.filter(|value| !value.trim().is_empty()) {
        name.to_string()
    } else {
        workbook
            .sheet_names()
            .first()
            .cloned()
            .unwrap_or_default()
    };
    if sheet_name.is_empty() {
        let mut result = empty_result(options, true);
        result
            .perf
            .insert("xlsx_scan_filter_ms".to_string(), elapsed_ms(scan_started));
        return Ok(result);
    }
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|error| anyhow!("failed to read excel worksheet `{sheet_name}`: {error}"))?;
    let mut rows_iter = range.rows();
    for _ in 0..header_row.saturating_sub(1) {
        rows_iter.next();
    }
    let headers = rows_iter
        .next()
        .map(|row| row.iter().map(xlsx_header).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut window = QueryWindow::new(options);
    for row in rows_iter {
        if window.should_stop() {
            break;
        }
        let mut map = serde_json::Map::new();
        for (index, header) in headers.iter().enumerate() {
            if header.is_empty() {
                continue;
            }
            let cell = row.get(index).map(xlsx_cell).unwrap_or(Value::Null);
            map.insert(header.clone(), cell);
        }
        if !map.values().any(|value| !value.is_null()) {
            continue;
        }
        let normalized = apply_normalize(Value::Object(map), normalize);
        if row_matches(&normalized, &options.filters, options.search.as_deref()) {
            window.push(normalized);
        }
    }
    let mut result = window.finish(headers, true);
    result
        .perf
        .insert("xlsx_scan_filter_ms".to_string(), elapsed_ms(scan_started));
    Ok(result)
}

fn load_xlsx_rows(
    path: &Path,
    is_xls: bool,
    sheet: Option<&str>,
    header_row: usize,
) -> Result<(Vec<String>, Vec<Value>)> {
    if is_xls {
        let mut workbook: Xls<_> = open_workbook(path)
            .with_context(|| format!("failed to open legacy xls {}", path.display()))?;
        return load_all_xlsx_reader(&mut workbook, sheet, header_row);
    }
    let mut workbook: Xlsx<_> =
        open_workbook(path).with_context(|| format!("failed to open xlsx {}", path.display()))?;
    load_all_xlsx_reader(&mut workbook, sheet, header_row)
}

fn load_all_xlsx_reader<R, RS>(
    workbook: &mut R,
    sheet: Option<&str>,
    header_row: usize,
) -> Result<(Vec<String>, Vec<Value>)>
where
    R: Reader<RS>,
    RS: std::io::Read + std::io::Seek,
    <R as Reader<RS>>::Error: std::fmt::Display,
{
    let sheet_name = if let Some(name) = sheet.filter(|value| !value.trim().is_empty()) {
        name.to_string()
    } else {
        workbook
            .sheet_names()
            .first()
            .cloned()
            .unwrap_or_default()
    };
    if sheet_name.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let range = workbook
        .worksheet_range(&sheet_name)
        .map_err(|error| anyhow!("failed to read excel worksheet `{sheet_name}`: {error}"))?;
    let mut rows_iter = range.rows();
    for _ in 0..header_row.saturating_sub(1) {
        rows_iter.next();
    }
    let headers = rows_iter
        .next()
        .map(|row| row.iter().map(xlsx_header).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut rows = Vec::new();
    for row in rows_iter {
        let mut map = serde_json::Map::new();
        for (index, header) in headers.iter().enumerate() {
            if header.is_empty() {
                continue;
            }
            let cell = row.get(index).map(xlsx_cell).unwrap_or(Value::Null);
            map.insert(header.clone(), cell);
        }
        if map.values().any(|value| !value.is_null()) {
            rows.push(Value::Object(map));
        }
    }
    Ok((headers, rows))
}
