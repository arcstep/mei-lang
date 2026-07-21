//! Request-only dataset row materialization helpers.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use mei_lang_kernel::{
    resolve_data_snapshot_import_entry, resolve_versioned_source_identifier,
    source_file_content_signature, DatasetView,
};
use serde_json::Value;

use crate::paginate::paginate_rows;
use crate::paths::resolve_source_path;
use crate::serialize_cache_value;
use crate::types::{DatasetQueryOptions, DatasetQueryResult, SourceMeta};
use crate::util::elapsed_ms;

static FALLBACK_MATERIALIZATION_PEAK_BYTES: AtomicU64 = AtomicU64::new(0);

fn approximate_value_bytes(value: &Value) -> usize {
    match value {
        Value::Null | Value::Bool(_) => 1,
        Value::Number(_) => 8,
        Value::String(text) => text.len(),
        Value::Array(values) => values.iter().map(approximate_value_bytes).sum(),
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| key.len().saturating_add(approximate_value_bytes(value)))
            .sum(),
    }
}

pub(crate) fn record_fallback_materialization_rows(rows: &[Value]) {
    let bytes = rows
        .iter()
        .map(approximate_value_bytes)
        .sum::<usize>()
        .min(u64::MAX as usize) as u64;
    FALLBACK_MATERIALIZATION_PEAK_BYTES.fetch_max(bytes, Ordering::Relaxed);
}

pub fn fallback_materialization_peak_bytes() -> u64 {
    FALLBACK_MATERIALIZATION_PEAK_BYTES.load(Ordering::Relaxed)
}

#[derive(Clone)]
pub(crate) struct MaterializedDatasetRows {
    columns: Vec<String>,
    rows: Vec<Value>,
    lazy: bool,
}

fn source_data_stamp(app_root: &Path, dataset: &DatasetView) -> Option<String> {
    let path = resolve_source_path(app_root, dataset.source.path.as_str());
    if !path.is_file() {
        return None;
    }
    if let Some(imported) = resolve_data_snapshot_import_entry(
        app_root,
        dataset.source.path.as_str(),
        dataset.source.sheet.as_deref(),
        dataset.source.header_row.unwrap_or(1).max(1) as usize,
    ) {
        return Some(format!("import={}", imported.content_signature));
    }
    let resolved = resolve_versioned_source_identifier(app_root, dataset.source.path.as_str());
    Some(format!(
        "source={}",
        source_file_content_signature(path.as_path(), resolved.as_str())
    ))
}

pub(crate) fn dataset_rows_scope_cache_key(
    app_root: &Path,
    dataset: &DatasetView,
    meta: &SourceMeta,
    options: &DatasetQueryOptions,
) -> Option<String> {
    let data_revision = source_data_stamp(app_root, dataset)?;
    let sheet = meta.sheet.as_deref().unwrap_or("");
    let header_row = meta.header_row.unwrap_or(1);
    let schema_key = serialize_cache_value(
        &dataset
            .schema
            .iter()
            .map(|column| format!("{}:{}", column.name, column.type_name))
            .collect::<Vec<_>>(),
    );
    Some(format!(
        "rows|{}|{}|{}|{}|rev={}|sheet={}|header={}|schema={}|search={}|filters={}|group={}|time_range={}",
        app_root.display(),
        dataset.id,
        dataset.source.kind,
        dataset.source.path,
        data_revision,
        sheet,
        header_row,
        schema_key,
        options.search.as_deref().unwrap_or(""),
        serialize_cache_value(&options.filters),
        serialize_cache_value(&options.group),
        serialize_cache_value(&options.time_range),
    ))
}

pub(crate) fn take_materialized_dataset_rows(_key: &str) -> Option<MaterializedDatasetRows> {
    None
}

pub(crate) fn store_materialized_dataset_rows(
    _key: String,
    _columns: Vec<String>,
    _rows: Vec<Value>,
    _lazy: bool,
) {
    // Full JSON rowsets are never retained across requests.
}

pub(crate) fn paginate_materialized_dataset_rows(
    materialized: &MaterializedDatasetRows,
    normalize: &BTreeMap<String, String>,
    options: &DatasetQueryOptions,
    lookup_started: Instant,
) -> DatasetQueryResult {
    let mut result = paginate_rows(
        materialized.rows.clone(),
        &materialized.columns,
        normalize,
        options,
        materialized.lazy,
    );
    result.perf.insert("dataset_rows_cache_hit".to_string(), 1);
    result.perf.insert(
        "dataset_rows_cache_lookup_ms".to_string(),
        elapsed_ms(lookup_started),
    );
    result.perf.insert(
        "dataset_rows_cache_rows".to_string(),
        materialized.rows.len() as u64,
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dataset_rows_cache_does_not_retain_materialized_scope() {
        clear_dataset_rows_cache();
        let key = "test-scope-key".to_string();
        let rows = (1..=150).map(|id| json!({"id": id})).collect::<Vec<_>>();
        store_materialized_dataset_rows(key.clone(), vec!["id".to_string()], rows, true);
        assert!(take_materialized_dataset_rows(&key).is_none());
    }

    #[test]
    fn paginate_rows_eager_materialize_collects_full_scope() {
        let rows = (1..=200).map(|id| json!({"id": id})).collect::<Vec<_>>();
        let options = DatasetQueryOptions {
            page: 1,
            page_size: 20,
            collect_all: false,
            ..Default::default()
        };
        let (page, materialized) = paginate_rows_eager_materialize(
            rows,
            &["id".to_string()],
            &BTreeMap::new(),
            &options,
            true,
        );
        assert_eq!(page.rows.len(), 20);
        assert_eq!(page.total, 200);
        assert!(materialized.is_none());
    }
}

pub(crate) fn clear_dataset_rows_cache() -> usize {
    0
}

/// 单次扫描并分页；不返回跨请求物化副本。
pub(crate) fn paginate_rows_eager_materialize(
    rows: Vec<Value>,
    columns_hint: &[String],
    normalize: &BTreeMap<String, String>,
    options: &DatasetQueryOptions,
    lazy: bool,
) -> (DatasetQueryResult, Option<(Vec<String>, Vec<Value>)>) {
    record_fallback_materialization_rows(&rows);
    (
        paginate_rows(rows, columns_hint, normalize, options, lazy),
        None,
    )
}
