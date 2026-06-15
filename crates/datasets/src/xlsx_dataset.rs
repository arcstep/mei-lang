use std::{path::Path, time::Instant};

use anyhow::Result;
use mei_lang_kernel::{
    cached_load_xlsx_table_snapshot, coerce_calendar_columns_in_rows, coerce_row_to_schema,
    ColumnSchema, SourceDecl,
};

use super::file_cache::ExternalFileCacheSettings;
use super::paginate::paginate_rows_iter;
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
    if can_return_snapshot_directly(meta, options, schema) {
        let row_count = snapshot.rows.len();
        let rows = if schema.is_empty() {
            coerce_calendar_columns_in_rows(
                snapshot.rows.clone(),
                &snapshot.columns,
                &[],
            )
        } else {
            snapshot
                .rows
                .iter()
                .map(|row| coerce_row_to_schema(row, schema))
                .collect()
        };
        let mut result = DatasetQueryResult {
            page: 1,
            page_size: row_count,
            total: row_count,
            has_more: false,
            columns: snapshot.columns.clone(),
            rows,
            lazy: true,
            perf: Default::default(),
            column_meta: Vec::new(),
            summary: None,
            query_state_echo: None,
        };
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
        result.perf.insert("file_cache_paginate_ms".to_string(), 0);
        result
            .perf
            .insert("file_cache_direct_snapshot".to_string(), 1);
        return Ok(result);
    }
    let paginate_started = Instant::now();
    let mut result = if schema.is_empty() {
        paginate_rows_iter(
            snapshot.rows.iter().cloned(),
            &snapshot.columns,
            &meta.normalize,
            options,
            true,
        )
    } else {
        paginate_rows_iter(
            snapshot
                .rows
                .iter()
                .map(|row| coerce_row_to_schema(row, schema)),
            &snapshot.columns,
            &meta.normalize,
            options,
            true,
        )
    };
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

fn can_return_snapshot_directly(
    meta: &SourceMeta,
    options: &DatasetQueryOptions,
    schema: &[ColumnSchema],
) -> bool {
    schema.is_empty()
        && options.collect_all
        && options.filters.is_empty()
        && options.group.is_empty()
        && options.time_range.is_none()
        && options.sort.is_empty()
        && options.column_state.is_none()
        && !options.summary
        && options
            .search
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
        && meta.normalize.is_empty()
}
