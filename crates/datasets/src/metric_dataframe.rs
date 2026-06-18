//! 将 runtime metric（dataframe shape）物化后走统一分页/过滤管线。

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use mei_lang_kernel::{
    coerce_calendar_columns_in_rows, evaluate_runtime_metric_defs_with_scope_and_dag,
    runtime_eval_node_cache_enabled, ColumnSchema, CompiledApp, DatasetView, EvalPlanNodeKind,
    FilterIntent, MetricShape, QueryState,
};
use serde_json::Value;

use super::metric_hydrate::{resolve_dataset_query_bindings_from_state, unique_dataset_views};
use super::metric_hydrate::hydrate_file_backed_datasets_for_metric_defs;
use super::metric_locate::locate_runtime_metric_resource;
use super::paginate::{infer_columns, paginate_rows};
use super::query::query_dataset_rows;
use super::table_contract::{
    column_meta_for_row_schema, format_rows_with_dataset_schema,
};
use super::types::{parse_source_meta, DatasetQueryOptions, DatasetQueryResult};
use super::util::elapsed_ms;
use super::{
    build_compiled_datasets_map, metric_request_revision_fingerprint_for_compiled,
    metric_scope_cache_key, query_state_from_request, runtime_metric_eval_scope,
    runtime_metric_workset, serialize_cache_value,
};

const DEFAULT_PAGE_SIZE: usize = 20;
const MAX_PAGE_SIZE: usize = 1000;
const METRIC_DATAFRAME_CACHE_TTL_MS: u64 = 1500;
const METRIC_DATAFRAME_MATERIALIZED_CACHE_TTL_MS: u64 = 300_000;
const METRIC_DATAFRAME_CACHE_PRUNE_INTERVAL_MS: u64 = 5_000;
const MAX_METRIC_DATAFRAME_MATERIALIZED_ENTRIES: usize = 64;
/// 空行集不写入物化缓存，避免 composition 等依赖 rowset 的 metric 在并行冷启动时
/// 抢先缓存 0 行结果（TTL 5min），导致图表长期空白。
const MIN_MATERIALIZED_METRIC_ROWS_TO_CACHE: usize = 1;

#[derive(Clone)]
struct CachedMetricDataframeResult {
    expires_at: Instant,
    result: DatasetQueryResult,
}

#[derive(Clone)]
struct MaterializedMetricDataframe {
    expires_at: Instant,
    columns: Vec<String>,
    rows: Vec<Value>,
    row_schema: Vec<ColumnSchema>,
    normalize: BTreeMap<String, String>,
    base_perf: BTreeMap<String, u64>,
}

#[derive(Default)]
struct MetricDataframeCacheState {
    entries: BTreeMap<String, CachedMetricDataframeResult>,
    next_prune_at: Option<Instant>,
}

#[derive(Default)]
struct MetricDataframeMaterializedCacheState {
    entries: BTreeMap<String, MaterializedMetricDataframe>,
    next_prune_at: Option<Instant>,
}

impl MetricDataframeCacheState {
    fn prune_if_due(&mut self, now: Instant) {
        if self.next_prune_at.is_some_and(|next| now < next) {
            return;
        }
        self.entries.retain(|_, entry| entry.expires_at > now);
        self.next_prune_at =
            Some(now + Duration::from_millis(METRIC_DATAFRAME_CACHE_PRUNE_INTERVAL_MS));
    }
}

fn metric_dataframe_result_cache() -> &'static Mutex<MetricDataframeCacheState> {
    static CACHE: OnceLock<Mutex<MetricDataframeCacheState>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(MetricDataframeCacheState::default()))
}

fn metric_dataframe_materialized_cache() -> &'static Mutex<MetricDataframeMaterializedCacheState> {
    static CACHE: OnceLock<Mutex<MetricDataframeMaterializedCacheState>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(MetricDataframeMaterializedCacheState::default()))
}

fn metric_dataframe_cache_ttl() -> Duration {
    Duration::from_millis(METRIC_DATAFRAME_CACHE_TTL_MS)
}

fn metric_dataframe_materialized_cache_ttl() -> Duration {
    Duration::from_millis(METRIC_DATAFRAME_MATERIALIZED_CACHE_TTL_MS)
}

fn hash_fingerprint(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn metric_dataframe_scope_cache_key(
    app_root: &Path,
    scene_id: Option<&str>,
    target: Option<&str>,
    dataset_id: &str,
    metric_id: &str,
    options: &DatasetQueryOptions,
    compile_revision: &str,
    dependency_revision_key: &str,
    filter_intents: &[FilterIntent],
) -> String {
    let group = serialize_cache_value(&options.group);
    let time_range = serialize_cache_value(&options.time_range);
    format!(
        "{}|compile={}|{}|scene={}|target={}|{}|{}|search={}|filters={}|group={}|time_range={}|filter_intents={}",
        app_root.display(),
        compile_revision,
        dependency_revision_key,
        scene_id.unwrap_or("").trim(),
        target.unwrap_or("").trim(),
        dataset_id,
        metric_id,
        options.search.as_deref().unwrap_or(""),
        serialize_cache_value(&options.filters),
        group,
        time_range,
        serde_json::to_string(filter_intents).unwrap_or_else(|_| "[]".to_string()),
    )
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
    filter_intents: &[FilterIntent],
) -> String {
    let sort = serialize_cache_value(&options.sort);
    let column_state = serialize_cache_value(&options.column_state);
    format!(
        "{}|page={}|page_size={}|full={}|sort={}|column_state={}|summary={}",
        metric_dataframe_scope_cache_key(
            app_root,
            scene_id,
            target,
            dataset_id,
            metric_id,
            options,
            compile_revision,
            dependency_revision_key,
            filter_intents,
        ),
        options.page,
        options.page_size,
        options.collect_all,
        sort,
        column_state,
        options.summary
    )
}

impl MetricDataframeMaterializedCacheState {
    fn prune_if_due(&mut self, now: Instant) {
        if self.next_prune_at.is_some_and(|next| now < next) {
            return;
        }
        self.entries.retain(|_, entry| entry.expires_at > now);
        self.next_prune_at =
            Some(now + Duration::from_millis(METRIC_DATAFRAME_CACHE_PRUNE_INTERVAL_MS));
    }
}

fn take_cached_metric_dataframe_materialized(key: &str) -> Option<MaterializedMetricDataframe> {
    let Ok(mut cache) = metric_dataframe_materialized_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.prune_if_due(now);
    cache.entries.get(key).cloned()
}

fn store_cached_metric_dataframe_materialized(key: String, materialized: MaterializedMetricDataframe) {
    if materialized.rows.len() < MIN_MATERIALIZED_METRIC_ROWS_TO_CACHE {
        return;
    }
    let Ok(mut cache) = metric_dataframe_materialized_cache().lock() else {
        return;
    };
    cache.prune_if_due(Instant::now());
    if cache.entries.len() >= MAX_METRIC_DATAFRAME_MATERIALIZED_ENTRIES {
        cache.entries.clear();
    }
    cache.entries.insert(key, materialized);
}

fn take_cached_metric_dataframe_result(key: &str) -> Option<DatasetQueryResult> {
    let Ok(mut cache) = metric_dataframe_result_cache().lock() else {
        return None;
    };
    let now = Instant::now();
    cache.prune_if_due(now);
    cache.entries.get(key).map(|entry| entry.result.clone())
}

fn store_cached_metric_dataframe_result(key: String, result: &DatasetQueryResult) {
    if result.rows.is_empty() && result.total == 0 {
        return;
    }
    let Ok(mut cache) = metric_dataframe_result_cache().lock() else {
        return;
    };
    cache.prune_if_due(Instant::now());
    cache.entries.insert(
        key,
        CachedMetricDataframeResult {
            expires_at: Instant::now() + metric_dataframe_cache_ttl(),
            result: result.clone(),
        },
    );
}

pub(crate) fn clear_metric_dataframe_result_cache() -> usize {
    let mut removed = metric_dataframe_result_cache()
        .lock()
        .ok()
        .map(|mut cache| {
            let count = cache.entries.len();
            cache.entries.clear();
            cache.next_prune_at = None;
            count
        })
        .unwrap_or(0);
    if let Ok(mut materialized) = metric_dataframe_materialized_cache().lock() {
        removed = removed.saturating_add(materialized.entries.len());
        materialized.entries.clear();
        materialized.next_prune_at = None;
    }
    removed
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
    query_state: Option<QueryState>,
    filter_intents: Vec<FilterIntent>,
) -> Result<DatasetQueryResult> {
    let effective_query_state = query_state_from_request(
        &options.filters,
        options.search.as_deref(),
        query_state.as_ref(),
    );
    let options = DatasetQueryOptions {
        search: effective_query_state.search.clone(),
        filters: effective_query_state.filters.clone(),
        group: effective_query_state.group.clone(),
        time_range: effective_query_state.time_range.clone(),
        ..options
    };
    let (resource, resolved_metric_id) =
        locate_runtime_metric_resource(compiled, dataset_id, metric_id)?;
    let dataset = resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{}` is not a dataset", resource.id))?;
    let workset = runtime_metric_workset(
        &resource.id,
        std::slice::from_ref(&resolved_metric_id),
        dataset,
    );
    let effective_metric_ids = workset
        .eval_metric_ids
        .clone()
        .unwrap_or_else(|| vec![resolved_metric_id.clone()]);
    let defs_for_hydrate = workset.defs_for_hydrate.clone();
    let referenced_dataset_ids =
        super::metric_hydrate::collect_dataset_ids_from_metric_defs(&defs_for_hydrate);
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root,
        compiled,
        &resource.id,
        &defs_for_hydrate,
    );
    let response_cache_key = metric_dataframe_cache_key(
        app_root,
        scene_id,
        target,
        &resource.id,
        &metric_scope_cache_key(&effective_metric_ids),
        &options,
        compile_revision,
        &dependency_revision_key,
        &filter_intents,
    );
    let materialized_cache_key = metric_dataframe_scope_cache_key(
        app_root,
        scene_id,
        target,
        &resource.id,
        &metric_scope_cache_key(&effective_metric_ids),
        &options,
        compile_revision,
        &dependency_revision_key,
        &filter_intents,
    );
    let response_cache_lookup_started = Instant::now();
    if let Some(mut cached) = take_cached_metric_dataframe_result(&response_cache_key) {
        if cached.rows.is_empty() && cached.total == 0 {
            // 跳过竞态产生的空响应缓存，重新求值。
        } else {
            cached.perf = BTreeMap::from([
                ("response_cache_hit".to_string(), 1),
                (
                    "response_cache_key_hash".to_string(),
                    hash_fingerprint(&response_cache_key),
                ),
                ("request_dag_observed".to_string(), 0),
                ("eval_memo_hits".to_string(), 0),
                ("eval_memo_eval_node_cache_hits".to_string(), 0),
                ("eval_memo_eval_node_cache_misses".to_string(), 0),
                (
                    "response_cache_lookup_ms".to_string(),
                    elapsed_ms(response_cache_lookup_started),
                ),
            ]);
            return Ok(cached);
        }
    }

    let meta = parse_source_meta(dataset.source.content.as_deref());
    if let Some(materialized) = take_cached_metric_dataframe_materialized(&materialized_cache_key) {
        if materialized.rows.len() >= MIN_MATERIALIZED_METRIC_ROWS_TO_CACHE {
            let response_cache_lookup_ms = elapsed_ms(response_cache_lookup_started);
            let result = paginate_materialized_metric_dataframe(
                &materialized,
                &meta,
                &options,
                &response_cache_key,
                response_cache_lookup_ms,
                true,
                Some(0),
            );
            store_cached_metric_dataframe_result(response_cache_key, &result);
            return Ok(result);
        }
    }

    let response_cache_lookup_ms = elapsed_ms(response_cache_lookup_started);
    let eval_started = Instant::now();
    let primary_filters =
        resolve_dataset_query_bindings_from_state(&effective_query_state, dataset).mapped_filters;
    let base_query = DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: options.search.clone(),
        filters: primary_filters,
        group: options.group.clone(),
        time_range: options.time_range.clone(),
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

    let mut datasets = build_compiled_datasets_map(
        compiled,
        &resource.id,
        runtime_dataset.clone(),
        &referenced_dataset_ids,
    );

    hydrate_file_backed_datasets_for_metric_defs(
        app_root,
        &mut datasets,
        &defs_for_hydrate,
        &base_query,
    )
    .with_context(|| {
        format!(
            "metric_hydrate_binding_failed(dataframe): dataset={} metric={}",
            resource.id, resolved_metric_id
        )
    })?;

    let binding_datasets = unique_dataset_views(dataset, datasets.values());
    let supplementary_binding_datasets: Vec<&DatasetView> = binding_datasets
        .into_iter()
        .filter(|view| view.id != dataset.id)
        .collect();
    let metric_started = Instant::now();
    let eval_scope = runtime_metric_eval_scope(
        Some(dataset),
        &resource.id,
        scene_id.unwrap_or(""),
        target,
        effective_query_state.search.as_deref(),
        &effective_query_state.filters,
        Some(&effective_query_state),
        &filter_intents,
        &dependency_revision_key,
        &supplementary_binding_datasets,
    )
    .with_context(|| {
        format!(
            "metric_scope_binding_failed(dataframe): dataset={} metric={}",
            resource.id, resolved_metric_id
        )
    })?;
    let (metrics_map, eval_report) = evaluate_runtime_metric_defs_with_scope_and_dag(
        &dataset.runtime_metric_defs,
        &runtime_dataset.rows,
        &datasets,
        Some(effective_metric_ids.as_slice()),
        &eval_scope,
    )
    .with_context(|| {
        format!(
            "metric_eval_recursion_guard_tripped(dataframe): dataset={} metric={}",
            resource.id, resolved_metric_id
        )
    })?;
    let metric_eval_ms = elapsed_ms(metric_started);
    let dag_metrics = &eval_report.request_dag_metrics;
    let eval_plan = &eval_report.eval_plan;
    let eval_scope_key = format!(
        "{}|{}|{}",
        eval_scope.base_dataset_id,
        eval_scope.query_state.group_identity_key(),
        eval_scope.query_state.time_range_identity_key()
    );

    let metric = metrics_map
        .get(&resolved_metric_id)
        .ok_or_else(|| anyhow!("metric `{metric_id}` evaluation returned nothing"))?;
    if metric.shape != MetricShape::Dataframe {
        return Err(anyhow!(
            "metric `{metric_id}` shape is {:?}, expected dataframe",
            metric.shape
        ));
    }

    let mut columns = metric
        .schema
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let rows = extract_dataframe_rows(&metric.value);
    if columns.is_empty() && !rows.is_empty() {
        columns = infer_columns(&rows);
    }
    let (row_schema, rows) = format_rows_with_dataset_schema(&columns, rows, &datasets);

    let closure_set = effective_metric_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let closure_edges = dataset
        .runtime_analysis_graph
        .edges
        .iter()
        .filter(|edge| closure_set.contains(&edge.from) && closure_set.contains(&edge.to))
        .count() as u64;

    let materialized = MaterializedMetricDataframe {
        expires_at: Instant::now() + metric_dataframe_materialized_cache_ttl(),
        columns,
        rows,
        row_schema,
        normalize: meta.normalize.clone(),
        base_perf: BTreeMap::from([
            ("base_query_ms".to_string(), base_query_ms),
            ("metric_eval_ms".to_string(), metric_eval_ms),
            (
                "eval_plan_targets".to_string(),
                eval_plan.targets.len() as u64,
            ),
            (
                "eval_plan_nodes".to_string(),
                eval_plan.nodes.len() as u64,
            ),
            (
                "eval_plan_edges".to_string(),
                eval_plan.edges.len() as u64,
            ),
            (
                "eval_plan_metric_nodes".to_string(),
                eval_plan.node_count_by_kind(EvalPlanNodeKind::MetricEval) as u64,
            ),
            (
                "eval_plan_rowset_nodes".to_string(),
                eval_plan.node_count_by_kind(EvalPlanNodeKind::Rowset) as u64,
            ),
            (
                "eval_plan_scalar_nodes".to_string(),
                eval_plan.node_count_by_kind(EvalPlanNodeKind::ScalarExpr) as u64,
            ),
            (
                "eval_plan_hydrate_nodes".to_string(),
                eval_plan.node_count_by_kind(EvalPlanNodeKind::Hydrate) as u64,
            ),
            (
                "eval_scope_key_hash".to_string(),
                hash_fingerprint(&eval_scope_key),
            ),
            (
                "eval_scope_group_key_hash".to_string(),
                hash_fingerprint(&eval_scope.query_state.group_identity_key()),
            ),
            (
                "eval_scope_time_range_key_hash".to_string(),
                hash_fingerprint(&eval_scope.query_state.time_range_identity_key()),
            ),
            (
                "eval_scope_group_dimensions".to_string(),
                eval_scope.query_state.group.len() as u64,
            ),
            ("request_dag_nodes".to_string(), dag_metrics.nodes as u64),
            ("request_dag_edges".to_string(), dag_metrics.edges as u64),
            ("request_dag_hits".to_string(), dag_metrics.hits),
            ("request_dag_misses".to_string(), dag_metrics.misses),
            ("request_dag_observed".to_string(), 1),
            (
                "request_dag_request_cache_hits".to_string(),
                dag_metrics.request_cache_hits,
            ),
            (
                "request_dag_eval_node_cache_hits".to_string(),
                dag_metrics.eval_node_cache_hits,
            ),
            (
                "request_dag_eval_node_cache_misses".to_string(),
                dag_metrics.eval_node_cache_misses,
            ),
            (
                "eval_memo_hits".to_string(),
                dag_metrics.request_cache_hits,
            ),
            (
                "eval_memo_eval_node_cache_hits".to_string(),
                dag_metrics.eval_node_cache_hits,
            ),
            (
                "eval_memo_eval_node_cache_misses".to_string(),
                dag_metrics.eval_node_cache_misses,
            ),
            (
                "analysis_closure_nodes".to_string(),
                effective_metric_ids.len() as u64,
            ),
            ("analysis_closure_edges".to_string(), closure_edges),
            (
                "eval_node_cache_enabled".to_string(),
                u64::from(runtime_eval_node_cache_enabled()),
            ),
        ]),
    };
    store_cached_metric_dataframe_materialized(materialized_cache_key, materialized.clone());

    let metric_dataframe_eval_ms = elapsed_ms(eval_started);
    let mut result = paginate_materialized_metric_dataframe(
        &materialized,
        &meta,
        &options,
        &response_cache_key,
        response_cache_lookup_ms,
        false,
        Some(metric_dataframe_eval_ms),
    );
    result.perf.extend(filtered_rows.perf);
    store_cached_metric_dataframe_result(response_cache_key, &result);
    Ok(result)
}

fn paginate_materialized_metric_dataframe(
    materialized: &MaterializedMetricDataframe,
    meta: &super::types::SourceMeta,
    options: &DatasetQueryOptions,
    response_cache_key: &str,
    response_cache_lookup_ms: u64,
    from_materialized_cache: bool,
    metric_dataframe_eval_ms: Option<u64>,
) -> DatasetQueryResult {
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
        search: options.search.clone(),
        filters: options.filters.clone(),
        group: options.group.clone(),
        time_range: options.time_range.clone(),
        collect_all,
        sort: options.sort.clone(),
        column_state: options.column_state.clone(),
        summary: options.summary,
    };

    let mut result = paginate_rows(
        materialized.rows.clone(),
        &materialized.columns,
        &materialized.normalize,
        &normalized_options,
        true,
    );
    result.rows = coerce_calendar_columns_in_rows(
        std::mem::take(&mut result.rows),
        &result.columns,
        &materialized.row_schema,
    );
    if !materialized.row_schema.is_empty() {
        result.column_meta =
            column_meta_for_row_schema(&materialized.row_schema, &result.columns);
    }
    result.perf.extend(materialized.base_perf.clone());
    result.perf.insert("response_cache_hit".to_string(), 0);
    result.perf.insert(
        "materialized_cache_hit".to_string(),
        u64::from(from_materialized_cache),
    );
    result.perf.insert(
        "response_cache_key_hash".to_string(),
        hash_fingerprint(response_cache_key),
    );
    result.perf.insert(
        "response_cache_lookup_ms".to_string(),
        response_cache_lookup_ms,
    );
    if let Some(eval_ms) = metric_dataframe_eval_ms {
        result
            .perf
            .insert("metric_dataframe_eval_ms".to_string(), eval_ms);
    }
    result
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
