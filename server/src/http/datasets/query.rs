use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use mei_lang_kernel::DatasetView;

use super::csv_dataset;
use super::db_dataset;
use super::file_cache::resolve_external_file_cache_settings;
use super::json_dataset;
use super::paginate::paginate_rows;
use super::types::{parse_source_meta, DatasetQueryOptions, DatasetQueryResult, SourceMeta};
use super::util::elapsed_ms;
use super::xlsx_dataset;

pub fn query_dataset_rows(
    app_root: &Path,
    dataset: &DatasetView,
    options: DatasetQueryOptions,
) -> Result<DatasetQueryResult> {
    let query_total_started = Instant::now();
    let meta = parse_source_meta(dataset.source.content.as_deref());
    let cache_settings = resolve_external_file_cache_settings(app_root);
    let default_page_size = meta.lazy.default_page_size.unwrap_or(100).max(1);
    let max_page_size = meta.lazy.max_page_size.unwrap_or(1000).max(default_page_size);
    let collect_all = options.collect_all;
    let page = if collect_all { 1 } else { options.page.max(1) };
    let requested_page_size = if collect_all {
        0
    } else if options.page_size == 0 {
        default_page_size
    } else {
        options.page_size
    };
    let page_size = if collect_all {
        0
    } else {
        requested_page_size.clamp(1, max_page_size)
    };
    let normalized_options = DatasetQueryOptions {
        page,
        page_size,
        search: options.search,
        filters: options.filters,
        collect_all,
    };

    let file_backed = file_backed_for_lazy_query(dataset, &meta);
    // 仅无外部可解析源时对物化 rows 分页；外部文件/DB 一律走查询管线。
    let use_external_query_path = file_backed;
    if !use_external_query_path {
        let mut result = paginate_rows(
            dataset.rows.clone(),
            &dataset.columns,
            &meta.normalize,
            &normalized_options,
            false,
        );
        result
            .perf
            .insert("query_total_ms".to_string(), elapsed_ms(query_total_started));
        return Ok(result);
    }

    let source_kind = source_kind(dataset);
    let mut result = match source_kind.as_str() {
        "csv" => csv_dataset::query_csv_rows(
            app_root,
            &dataset.source,
            &meta,
            &normalized_options,
            &cache_settings,
        ),
        "json" => json_dataset::query_json_rows(
            app_root,
            &dataset.source,
            &meta,
            &normalized_options,
            &cache_settings,
        ),
        "xlsx" | "xls" => xlsx_dataset::query_xlsx_rows(
            app_root,
            &dataset.source,
            &meta,
            &normalized_options,
            &cache_settings,
        ),
        "db" => db_dataset::query_db_rows(app_root, &dataset.source, &meta, &normalized_options),
        _ => Ok(paginate_rows(
            dataset.rows.clone(),
            &dataset.columns,
            &meta.normalize,
            &normalized_options,
            false,
        )),
    }?;
    result
        .perf
        .insert("query_total_ms".to_string(), elapsed_ms(query_total_started));
    Ok(result)
}

fn file_backed_for_lazy_query(dataset: &DatasetView, meta: &SourceMeta) -> bool {
    let path = dataset.source.path.trim();
    if path.is_empty() || path.starts_with("dataset_view:") {
        return false;
    }
    let kind = dataset.source.kind.trim();
    if kind.eq_ignore_ascii_case("derived") {
        return false;
    }
    if kind.eq_ignore_ascii_case("db")
        || meta.connection.is_some()
        || meta.table.is_some()
        || meta.query.is_some()
    {
        return true;
    }
    matches!(
        source_kind(dataset).as_str(),
        "csv" | "json" | "xlsx" | "xls"
    )
}

fn source_kind(dataset: &DatasetView) -> String {
    let kind = dataset.source.kind.trim();
    if !kind.is_empty() {
        return kind.to_string();
    }
    if dataset.source.path.ends_with(".xlsx") || dataset.source.path.ends_with(".xls") {
        "xlsx".to_string()
    } else if dataset.source.path.ends_with(".json") {
        "json".to_string()
    } else {
        "csv".to_string()
    }
}
