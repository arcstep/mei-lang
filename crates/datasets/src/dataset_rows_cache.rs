//! 跨请求 dataset 行集物化缓存：同一外部源 + 筛选条件下复用内存行集，避免重复扫盘/扫表。

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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

const DATASET_ROWS_CACHE_TTL_MS: u64 = 300_000;
const DATASET_ROWS_CACHE_PRUNE_INTERVAL_MS: u64 = 5_000;
const MAX_DATASET_ROWS_CACHE_ENTRIES: usize = 48;
const MIN_ROWS_TO_CACHE: usize = 128;

#[derive(Clone)]
pub(crate) struct MaterializedDatasetRows {
    expires_at: Instant,
    columns: Vec<String>,
    rows: Vec<Value>,
    lazy: bool,
}

#[derive(Default)]
struct DatasetRowsCacheState {
    entries: BTreeMap<String, MaterializedDatasetRows>,
    lru: VecDeque<String>,
    next_prune_at: Option<Instant>,
}

fn dataset_rows_cache() -> &'static Mutex<DatasetRowsCacheState> {
    static CACHE: OnceLock<Mutex<DatasetRowsCacheState>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(DatasetRowsCacheState::default()))
}

fn cache_ttl() -> Duration {
    Duration::from_millis(DATASET_ROWS_CACHE_TTL_MS)
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

fn prune_if_due(state: &mut DatasetRowsCacheState, now: Instant) {
    if state.next_prune_at.is_some_and(|next| now < next) {
        return;
    }
    state.entries.retain(|_, entry| entry.expires_at > now);
    state.lru.retain(|key| state.entries.contains_key(key));
    state.next_prune_at = Some(now + Duration::from_millis(DATASET_ROWS_CACHE_PRUNE_INTERVAL_MS));
}

fn touch_lru(state: &mut DatasetRowsCacheState, key: &str) {
    state.lru.retain(|value| value != key);
    state.lru.push_front(key.to_string());
}

fn evict_if_needed(state: &mut DatasetRowsCacheState) {
    while state.entries.len() > MAX_DATASET_ROWS_CACHE_ENTRIES {
        let Some(oldest) = state.lru.pop_back() else {
            break;
        };
        state.entries.remove(&oldest);
    }
}

pub(crate) fn take_materialized_dataset_rows(key: &str) -> Option<MaterializedDatasetRows> {
    let Ok(mut cache) = dataset_rows_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    prune_if_due(&mut cache, now);
    if !cache.entries.contains_key(key) {
        return None;
    }
    if cache
        .entries
        .get(key)
        .is_some_and(|entry| entry.expires_at <= now)
    {
        cache.entries.remove(key);
        cache.lru.retain(|value| value != key);
        return None;
    }
    touch_lru(&mut cache, key);
    cache.entries.get(key).cloned()
}

pub(crate) fn store_materialized_dataset_rows(
    key: String,
    columns: Vec<String>,
    rows: Vec<Value>,
    lazy: bool,
) {
    if rows.len() < MIN_ROWS_TO_CACHE {
        return;
    }
    let Ok(mut cache) = dataset_rows_cache().lock() else {
        return;
    };
    let now = Instant::now();
    prune_if_due(&mut cache, now);
    cache.lru.retain(|value| value != &key);
    cache.entries.insert(
        key.clone(),
        MaterializedDatasetRows {
            expires_at: now + cache_ttl(),
            columns,
            rows,
            lazy,
        },
    );
    touch_lru(&mut cache, key.as_str());
    evict_if_needed(&mut cache);
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
    fn dataset_rows_cache_reuses_materialized_scope() {
        clear_dataset_rows_cache();
        let key = "test-scope-key".to_string();
        let options = DatasetQueryOptions {
            page: 2,
            page_size: 2,
            collect_all: false,
            ..Default::default()
        };
        let rows = (1..=150).map(|id| json!({"id": id})).collect::<Vec<_>>();
        store_materialized_dataset_rows(key.clone(), vec!["id".to_string()], rows, true);
        let materialized = take_materialized_dataset_rows(&key).expect("cached rows");
        let result = paginate_materialized_dataset_rows(
            &materialized,
            &BTreeMap::new(),
            &options,
            Instant::now(),
        );
        assert_eq!(result.page, 2);
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].get("id").and_then(|v| v.as_i64()), Some(3));
        assert_eq!(result.perf.get("dataset_rows_cache_hit"), Some(&1));
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
        let (_, matched) = materialized.expect("materialized rows");
        assert_eq!(matched.len(), 200);
    }
}

pub(crate) fn clear_dataset_rows_cache() -> usize {
    let Ok(mut cache) = dataset_rows_cache().lock() else {
        return 0;
    };
    let removed = cache.entries.len();
    cache.entries.clear();
    cache.lru.clear();
    cache.next_prune_at = None;
    removed
}

pub(crate) fn dataset_rows_scope_options(options: &DatasetQueryOptions) -> DatasetQueryOptions {
    DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: options.search.clone(),
        filters: options.filters.clone(),
        group: options.group.clone(),
        time_range: options.time_range.clone(),
        collect_all: true,
        sort: Vec::new(),
        column_state: None,
        summary: false,
    }
}

/// 单次扫描：收集全部匹配行并分页返回，供写入物化缓存。
pub(crate) fn paginate_rows_eager_materialize(
    rows: Vec<Value>,
    columns_hint: &[String],
    normalize: &BTreeMap<String, String>,
    options: &DatasetQueryOptions,
    lazy: bool,
) -> (DatasetQueryResult, Option<(Vec<String>, Vec<Value>)>) {
    if !options.sort.is_empty() {
        let result = paginate_rows(rows, columns_hint, normalize, options, lazy);
        return (result, None);
    }
    let scope_options = dataset_rows_scope_options(options);
    let materialized = paginate_rows(rows, columns_hint, normalize, &scope_options, lazy);
    if materialized.rows.len() < MIN_ROWS_TO_CACHE {
        let result = paginate_rows(
            materialized.rows.clone(),
            &materialized.columns,
            normalize,
            options,
            lazy,
        );
        return (result, None);
    }
    let result = paginate_rows(
        materialized.rows.clone(),
        &materialized.columns,
        normalize,
        options,
        lazy,
    );
    (
        result,
        Some((materialized.columns.clone(), materialized.rows)),
    )
}
