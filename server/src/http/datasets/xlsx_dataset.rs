use std::{path::Path, time::Instant};

use anyhow::Result;
use mei_lang_kernel::{
    cached_load_xlsx_table_snapshot, coerce_rows_to_schema, ColumnSchema, SourceDecl,
};

use super::file_cache::ExternalFileCacheSettings;
use super::paginate::paginate_rows;
use super::types::{DatasetQueryOptions, DatasetQueryResult, SourceMeta};
use super::util::elapsed_ms;

pub(crate) fn query_xlsx_rows(
    app_root: &Path,
    source: &SourceDecl,
    meta: &SourceMeta,
    options: &DatasetQueryOptions,
    _cache_settings: &ExternalFileCacheSettings,
    schema: &[ColumnSchema],
) -> Result<DatasetQueryResult> {
    let sheet = meta.sheet.as_deref();
    let header_row = meta.header_row.unwrap_or(1).max(1) as usize;
    let snapshot_started = Instant::now();
    let (snapshot, cache_hit) =
        cached_load_xlsx_table_snapshot(app_root, source.path.as_str(), sheet, header_row)?;
    let snapshot_ms = elapsed_ms(snapshot_started);
    let rows = if schema.is_empty() {
        snapshot.rows.clone()
    } else {
        coerce_rows_to_schema(snapshot.rows.clone(), schema)
    };
    let paginate_started = Instant::now();
    let mut result = paginate_rows(rows, &snapshot.columns, &meta.normalize, options, true);
    result
        .perf
        .insert("file_cache_hit".to_string(), u64::from(cache_hit));
    if cache_hit {
        result
            .perf
            .insert("file_cache_lookup_ms".to_string(), snapshot_ms);
    } else {
        result
            .perf
            .insert("file_cache_load_ms".to_string(), snapshot_ms);
    }
    result.perf.insert(
        "file_cache_paginate_ms".to_string(),
        elapsed_ms(paginate_started),
    );
    Ok(result)
}
