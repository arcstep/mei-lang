use std::{collections::BTreeMap, path::Path, time::Instant};

use anyhow::Result;
use mei_lang_kernel::{
    cached_load_xlsx_table_snapshot, coerce_calendar_columns_in_rows, coerce_row_to_schema,
    resolve_data_snapshot_import_entry, ColumnSchema, DatasetView,
};

use super::dataset_rows_cache::{
    dataset_rows_scope_cache_key, paginate_rows_eager_materialize, store_materialized_dataset_rows,
};
use super::duckdb_engine::{query_parquet_page, resolve_parquet_file_for_source, DuckdbPageQuery};
use super::file_cache::ExternalFileCacheSettings;
use super::table_handle::{load_table_handle, materialize_rows_from_handle};
use super::types::{DatasetQueryOptions, DatasetQueryResult, SourceMeta};
use super::util::elapsed_ms;

pub(crate) fn query_xlsx_rows(
    app_root: &Path,
    dataset: &DatasetView,
    meta: &SourceMeta,
    options: &DatasetQueryOptions,
    _cache_settings: &ExternalFileCacheSettings,
    schema: &[ColumnSchema],
) -> Result<DatasetQueryResult> {
    let source = &dataset.source;
    let sheet = meta.sheet.as_deref();
    let header_row = meta.header_row.unwrap_or(1).max(1) as usize;

    // Prefer DuckDB over parquet snapshot (no whole-table JSON materialization).
    if let Some(parquet) =
        resolve_parquet_file_for_source(app_root, source.path.as_str(), sheet, header_row)
    {
        let physical = resolve_data_snapshot_import_entry(
            app_root,
            source.path.as_str(),
            sheet,
            header_row,
        )
        .map(|e| e.columns)
        .filter(|c| !c.is_empty());
        let started = Instant::now();
        let page = query_parquet_page(
            app_root,
            DuckdbPageQuery {
                parquet_path: parquet.as_path(),
                schema,
                physical_columns: physical.as_deref(),
                normalize: &meta.normalize,
                options,
            },
        )?;
        let result = DatasetQueryResult {
            page: page.page,
            page_size: page.page_size,
            total: page.total,
            has_more: page.has_more,
            columns: page.columns,
            rows: page.rows,
            lazy: true,
            perf: BTreeMap::from([
                ("duckdb_query_ms".to_string(), page.duckdb_query_ms),
                (
                    "rows_materialized".to_string(),
                    page.rows_materialized as u64,
                ),
                ("dataset_import_artifact_hit".to_string(), 1),
                ("table_handle_hit".to_string(), 0),
                (
                    "file_cache_load_ms".to_string(),
                    elapsed_ms(started),
                ),
            ]),
            column_meta: Vec::new(),
            summary: None,
            query_state_echo: None,
        };
        // Only cache small pages — never whole-table collect_all into rows cache.
        if !options.collect_all {
            if let Some(scope_key) = dataset_rows_scope_cache_key(app_root, dataset, meta, options)
            {
                store_materialized_dataset_rows(
                    scope_key,
                    result.columns.clone(),
                    result.rows.clone(),
                    true,
                );
            }
        }
        return Ok(result);
    }

    // Fallback: no parquet yet — legacy L3 / calamine path (prebuild should create snapshots).
    let import_entry =
        resolve_data_snapshot_import_entry(app_root, source.path.as_str(), sheet, header_row);
    let snapshot_started = Instant::now();
    let (columns, coerced_source_rows, cache_hit, table_handle_hit) = if import_entry.is_some() {
        let (handle, handle_cache_hit) =
            load_table_handle(app_root, source.path.as_str(), sheet, header_row)?;
        let (columns, rows) = materialize_rows_from_handle(handle.as_ref(), schema)?;
        (columns, rows, handle_cache_hit, true)
    } else {
        let (snapshot, cache_hit) =
            cached_load_xlsx_table_snapshot(app_root, source.path.as_str(), sheet, header_row)?;
        let rows = if schema.is_empty() {
            coerce_calendar_columns_in_rows(snapshot.rows.clone(), &snapshot.columns, &[])
        } else {
            snapshot
                .rows
                .iter()
                .map(|row| coerce_row_to_schema(row, schema))
                .collect()
        };
        (snapshot.columns.clone(), rows, cache_hit, false)
    };
    let snapshot_ms = elapsed_ms(snapshot_started);
    if can_return_snapshot_directly(meta, options, schema) {
        let row_count = coerced_source_rows.len();
        let mut result = DatasetQueryResult {
            page: 1,
            page_size: row_count,
            total: row_count,
            has_more: false,
            columns,
            rows: coerced_source_rows,
            lazy: true,
            perf: Default::default(),
            column_meta: Vec::new(),
            summary: None,
            query_state_echo: None,
        };
        result
            .perf
            .insert("file_cache_hit".to_string(), u64::from(cache_hit));
        result
            .perf
            .insert("table_handle_hit".to_string(), u64::from(table_handle_hit));
        result
            .perf
            .insert("file_cache_load_ms".to_string(), snapshot_ms);
        result.perf.insert("file_cache_paginate_ms".to_string(), 0);
        result
            .perf
            .insert("file_cache_direct_snapshot".to_string(), 1);
        result.perf.insert(
            "dataset_import_artifact_hit".to_string(),
            u64::from(import_entry.is_some()),
        );
        return Ok(result);
    }
    let paginate_started = Instant::now();
    let (mut result, materialized) = paginate_rows_eager_materialize(
        coerced_source_rows,
        &columns,
        &meta.normalize,
        options,
        true,
    );
    if let Some((columns, rows)) = materialized {
        if !options.collect_all {
            if let Some(scope_key) = dataset_rows_scope_cache_key(app_root, dataset, meta, options)
            {
                store_materialized_dataset_rows(scope_key, columns, rows, true);
            }
        }
    }
    result
        .perf
        .insert("file_cache_hit".to_string(), u64::from(cache_hit));
    result
        .perf
        .insert("table_handle_hit".to_string(), u64::from(table_handle_hit));
    result
        .perf
        .insert("file_cache_load_ms".to_string(), snapshot_ms);
    result.perf.insert(
        "file_cache_paginate_ms".to_string(),
        elapsed_ms(paginate_started),
    );
    result.perf.insert(
        "dataset_import_artifact_hit".to_string(),
        u64::from(import_entry.is_some()),
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
