//! 将 runtime metric（dataframe shape）物化后走统一分页/过滤管线。

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use mei_lang_kernel::{
    evaluate_runtime_metric_defs_with_scope_and_dag, locate_dataset_resource,
    resolve_runtime_metric_def_key, runtime_eval_node_cache_enabled, CompiledApp, MetricShape,
};
use serde_json::Value;

use super::metric_hydrate::hydrate_file_backed_datasets_for_metric_defs;
use super::paginate::paginate_rows;
use super::query::query_dataset_rows;
use super::types::{parse_source_meta, DatasetQueryOptions, DatasetQueryResult};
use super::util::elapsed_ms;
use super::{
    metric_request_revision_fingerprint, metric_scope_cache_key, normalize_query_filters,
    normalize_query_search, select_metric_defs,
    serialize_cache_value, runtime_metric_eval_scope,
};

const DEFAULT_PAGE_SIZE: usize = 20;
const MAX_PAGE_SIZE: usize = 1000;
const METRIC_DATAFRAME_CACHE_TTL_MS: u64 = 1500;

#[derive(Clone)]
struct CachedMetricDataframeResult {
    expires_at: Instant,
    result: DatasetQueryResult,
}

fn metric_dataframe_result_cache() -> &'static Mutex<BTreeMap<String, CachedMetricDataframeResult>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, CachedMetricDataframeResult>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn metric_dataframe_cache_ttl() -> Duration {
    Duration::from_millis(METRIC_DATAFRAME_CACHE_TTL_MS)
}

fn metric_dataframe_cache_key(
    app_root: &Path,
    scene_id: Option<&str>,
    target: Option<&str>,
    dataset_id: &str,
    metric_id: &str,
    options: &DatasetQueryOptions,
    compile_revision: &str,
    dependency_revision_key: &str,
) -> String {
    let sort = serialize_cache_value(&options.sort);
    let column_state = serialize_cache_value(&options.column_state);
    format!(
        "{}|compile={}|{}|scene={}|target={}|{}|{}|page={}|page_size={}|full={}|search={}|filters={}|sort={}|column_state={}|summary={}",
        app_root.display(),
        compile_revision,
        dependency_revision_key,
        scene_id.unwrap_or("").trim(),
        target.unwrap_or("").trim(),
        dataset_id,
        metric_id,
        options.page,
        options.page_size,
        options.collect_all,
        options.search.as_deref().unwrap_or(""),
        serialize_cache_value(&options.filters),
        sort,
        column_state,
        options.summary
    )
}

fn take_cached_metric_dataframe_result(key: &str) -> Option<DatasetQueryResult> {
    let Ok(mut cache) = metric_dataframe_result_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.retain(|_, entry| entry.expires_at > now);
    cache.get(key).map(|entry| entry.result.clone())
}

fn store_cached_metric_dataframe_result(key: String, result: &DatasetQueryResult) {
    let Ok(mut cache) = metric_dataframe_result_cache().lock() else {
        return;
    };
    cache.retain(|_, entry| entry.expires_at > Instant::now());
    cache.insert(
        key,
        CachedMetricDataframeResult {
            expires_at: Instant::now() + metric_dataframe_cache_ttl(),
            result: result.clone(),
        },
    );
}

pub(crate) fn clear_metric_dataframe_result_cache() -> usize {
    let Ok(mut cache) = metric_dataframe_result_cache().lock() else {
        return 0;
    };
    let removed = cache.len();
    cache.clear();
    removed
}

fn locate_runtime_metric_resource<'a>(
    compiled: &'a CompiledApp,
    dataset_id: &str,
    metric_id: &str,
) -> Result<(&'a mei_lang_kernel::LoadedResource, String)> {
    let primary =
        locate_dataset_resource(compiled, dataset_id).map_err(|error| anyhow!("{error}"))?;
    if let Some(dataset) = primary.dataset.as_ref() {
        if !dataset.runtime_metric_defs.is_empty() {
            if let Some(resolved) = resolve_runtime_metric_def_key(
                &primary.id,
                metric_id,
                &dataset.runtime_metric_defs,
            ) {
                return Ok((primary, resolved));
            }
        }
    }
    for resource in &compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if dataset.runtime_metric_defs.is_empty() {
            continue;
        }
        if let Some(resolved) =
            resolve_runtime_metric_def_key(&resource.id, metric_id, &dataset.runtime_metric_defs)
        {
            return Ok((resource, resolved));
        }
    }
    if primary
        .dataset
        .as_ref()
        .is_some_and(|dataset| dataset.runtime_metric_defs.is_empty())
    {
        return Err(anyhow!("dataset `{dataset_id}` has no runtime metric defs"));
    }
    Err(anyhow!(
        "metric `{metric_id}` not found on dataset `{dataset_id}`"
    ))
}

pub fn query_metric_dataframe(
    compiled: &CompiledApp,
    app_root: &Path,
    dataset_id: &str,
    metric_id: &str,
    scene_id: Option<&str>,
    target: Option<&str>,
    compile_revision: &str,
    options: DatasetQueryOptions,
) -> Result<DatasetQueryResult> {
    let options = DatasetQueryOptions {
        search: normalize_query_search(options.search.as_deref()),
        filters: normalize_query_filters(&options.filters),
        ..options
    };
    let (resource, resolved_metric_id) =
        locate_runtime_metric_resource(compiled, dataset_id, metric_id)?;
    let dataset = resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{}` is not a dataset", resource.id))?;
    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|entry| entry.dataset.clone().map(|view| (entry.id.clone(), view)))
        .fold(BTreeMap::new(), |mut acc, (resource_id, dataset)| {
            acc.insert(resource_id, dataset.clone());
            acc.entry(dataset.id.clone()).or_insert(dataset);
            acc
        });
    let defs_for_hydrate = select_metric_defs(
        &dataset.runtime_metric_defs,
        std::slice::from_ref(&resolved_metric_id),
    );
    let dependency_revision_key =
        metric_request_revision_fingerprint(app_root, &datasets, &resource.id, &defs_for_hydrate);
    let response_cache_key = metric_dataframe_cache_key(
        app_root,
        scene_id,
        target,
        &resource.id,
        &metric_scope_cache_key(std::slice::from_ref(&resolved_metric_id)),
        &options,
        compile_revision,
        &dependency_revision_key,
    );
    let response_cache_lookup_started = Instant::now();
    if let Some(mut cached) = take_cached_metric_dataframe_result(&response_cache_key) {
        cached.perf = BTreeMap::from([
            ("response_cache_hit".to_string(), 1),
            (
                "response_cache_lookup_ms".to_string(),
                elapsed_ms(response_cache_lookup_started),
            ),
        ]);
        return Ok(cached);
    }

    let base_query = DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: options.search.clone(),
        filters: options.filters.clone(),
        collect_all: true,
        sort: Vec::new(),
        column_state: None,
        summary: false,
    };
    let base_started = Instant::now();
    let filtered_rows = query_dataset_rows(app_root, dataset, base_query.clone())?;
    let base_query_ms = elapsed_ms(base_started);

    let mut runtime_dataset = dataset.clone();
    runtime_dataset.rows = filtered_rows.rows.clone();
    if !filtered_rows.columns.is_empty() {
        runtime_dataset.columns = filtered_rows.columns.clone();
    }

    datasets.insert(resource.id.clone(), runtime_dataset.clone());

    let _ = hydrate_file_backed_datasets_for_metric_defs(
        app_root,
        &mut datasets,
        &defs_for_hydrate,
        &base_query,
    );

    let metric_started = Instant::now();
    let eval_scope = runtime_metric_eval_scope(
        &resource.id,
        scene_id.unwrap_or(""),
        target,
        base_query.search.as_deref(),
        &base_query.filters,
        &dependency_revision_key,
    );
    let (metrics_map, dag_metrics) = evaluate_runtime_metric_defs_with_scope_and_dag(
        &dataset.runtime_metric_defs,
        &runtime_dataset.rows,
        &datasets,
        Some(&[resolved_metric_id.clone()]),
        &eval_scope,
    )
    .with_context(|| {
        format!(
            "metric_eval_recursion_guard_tripped(dataframe): dataset={} metric={}",
            resource.id, resolved_metric_id
        )
    })?;
    let metric_eval_ms = elapsed_ms(metric_started);

    let metric = metrics_map
        .get(&resolved_metric_id)
        .ok_or_else(|| anyhow!("metric `{metric_id}` evaluation returned nothing"))?;
    if metric.shape != MetricShape::Dataframe {
        return Err(anyhow!(
            "metric `{metric_id}` shape is {:?}, expected dataframe",
            metric.shape
        ));
    }

    let columns = metric
        .schema
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let rows = extract_dataframe_rows(&metric.value);

    let meta = parse_source_meta(dataset.source.content.as_deref());
    let default_page_size = meta
        .lazy
        .default_page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .max(1);
    let max_page_size = meta
        .lazy
        .max_page_size
        .unwrap_or(MAX_PAGE_SIZE)
        .max(default_page_size);
    let collect_all = options.collect_all;
    let page = if collect_all { 1 } else { options.page.max(1) };
    let page_size = if collect_all {
        0
    } else if options.page_size == 0 {
        default_page_size
    } else {
        options.page_size.clamp(1, max_page_size)
    };
    let normalized_options = DatasetQueryOptions {
        page,
        page_size,
        search: options.search,
        filters: options.filters,
        collect_all,
        sort: options.sort.clone(),
        column_state: options.column_state.clone(),
        summary: options.summary,
    };

    let mut result = paginate_rows(rows, &columns, &meta.normalize, &normalized_options, true);
    result.perf.extend(filtered_rows.perf);
    result
        .perf
        .insert("response_cache_hit".to_string(), 0);
    result.perf.insert(
        "response_cache_lookup_ms".to_string(),
        elapsed_ms(response_cache_lookup_started),
    );
    result
        .perf
        .insert("base_query_ms".to_string(), base_query_ms);
    result
        .perf
        .insert("metric_eval_ms".to_string(), metric_eval_ms);
    result
        .perf
        .insert("request_dag_nodes".to_string(), dag_metrics.nodes as u64);
    result
        .perf
        .insert("request_dag_edges".to_string(), dag_metrics.edges as u64);
    result
        .perf
        .insert("request_dag_hits".to_string(), dag_metrics.hits);
    result
        .perf
        .insert("request_dag_misses".to_string(), dag_metrics.misses);
    result.perf.insert(
        "request_dag_request_cache_hits".to_string(),
        dag_metrics.request_cache_hits,
    );
    result.perf.insert(
        "request_dag_eval_node_cache_hits".to_string(),
        dag_metrics.eval_node_cache_hits,
    );
    result.perf.insert(
        "request_dag_eval_node_cache_misses".to_string(),
        dag_metrics.eval_node_cache_misses,
    );
    result.perf.insert(
        "eval_node_cache_enabled".to_string(),
        u64::from(runtime_eval_node_cache_enabled()),
    );
    store_cached_metric_dataframe_result(response_cache_key, &result);
    Ok(result)
}

fn extract_dataframe_rows(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(rows) => rows.clone(),
        Value::Object(map) => {
            if let Some(rows) = map.get("rows").and_then(Value::as_array) {
                rows.clone()
            } else if let Some(rows) = map.get("value").and_then(Value::as_array) {
                rows.clone()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::extract_dataframe_rows;
    use serde_json::json;

    #[test]
    fn extract_dataframe_rows_from_array_and_wrappers() {
        assert_eq!(
            extract_dataframe_rows(&json!([{"a": 1}])),
            vec![json!({"a": 1})]
        );
        assert_eq!(
            extract_dataframe_rows(&json!({"rows": [{"a": 2}]})),
            vec![json!({"a": 2})]
        );
        assert_eq!(
            extract_dataframe_rows(&json!({"value": [{"a": 3}]})),
            vec![json!({"a": 3})]
        );
        assert!(extract_dataframe_rows(&json!(42)).is_empty());
    }
}
