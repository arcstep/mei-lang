mod csv_dataset;
mod dataset_rows_cache;
mod db_dataset;
mod file_cache;
mod geojson_dataset;
mod json_dataset;
mod metric_access;
mod metric_cache_key;
mod metric_dataframe;
mod metric_hydrate;
mod metric_locate;
mod metric_response_cache;
mod paginate;
mod paths;
mod query;
pub mod table_contract;
mod types;
mod util;
mod xlsx_dataset;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{
    CompiledApp, DatasetView, FilterIntent, LoadedResource, MetricContract, QueryState,
    RuntimeMetricEvalScope,
};
use serde::Serialize;
use serde_json::Value;

pub use metric_access::{
    build_compiled_datasets_map, collect_all_query_options, evaluate_runtime_metrics,
    evaluate_runtime_metrics_from_plan, runtime_metric_scope_requested, RuntimeMetricEvalMode,
    RuntimeMetricEvalOutcome,
};
pub use metric_locate::{
    locate_runtime_metric_resource, metric_ids_visible_for_dataset,
    plan_access_metric_eval_for_ids, AccessMetricEvalPlan,
};
pub use metric_response_cache::{
    cached_metric_response_covers_request, clear_all_metric_caches, clear_metric_response_cache,
    metric_response_cache_scope_key, store_cached_metric_response, take_cached_metric_response,
    CachedMetricResponse,
};
pub use query::query_dataset_rows;
pub use table_contract::{
    apply_table_request_fields, enrich_table_result, QueryStateEcho, TableColumnState,
    TableSortSpec,
};
pub use types::{DatasetQueryOptions, DatasetQueryResult, TableColumnMeta, TableSummary};

#[derive(Debug, Clone, Default)]
pub struct RuntimeMetricWorkset {
    pub closure_metric_ids: Vec<String>,
    pub eval_metric_ids: Option<Vec<String>>,
    pub defs_for_hydrate: BTreeMap<String, Value>,
}

pub fn clear_external_file_cache_for_app(app_root: &Path) -> usize {
    file_cache::clear_external_file_cache_for_app(app_root)
}

pub fn clear_metric_dataframe_result_cache() -> usize {
    metric_dataframe::clear_metric_dataframe_result_cache()
}

pub fn clear_dataset_rows_cache() -> usize {
    dataset_rows_cache::clear_dataset_rows_cache()
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
    metric_dataframe::query_metric_dataframe(
        compiled,
        app_root,
        dataset_id,
        metric_id,
        scene_id,
        target,
        compile_revision,
        options,
        query_state,
        filter_intents,
    )
}

pub fn normalize_query_search(search: Option<&str>) -> Option<String> {
    metric_cache_key::normalize_query_search(search)
}

pub fn normalize_query_filters(filters: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    metric_cache_key::normalize_query_filters(filters)
}

pub fn query_state_from_request(
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
    state: Option<&QueryState>,
) -> QueryState {
    metric_cache_key::query_state_from_request(filters, search, state)
}

pub fn serialize_cache_value<T: Serialize>(value: &T) -> String {
    metric_cache_key::serialize_cache_value(value)
}

pub fn metric_scope_cache_key(resolved_metric_ids: &[String]) -> String {
    metric_cache_key::metric_scope_cache_key(resolved_metric_ids)
}

pub fn metric_request_revision_fingerprint(
    app_root: &Path,
    datasets: &BTreeMap<String, DatasetView>,
    base_dataset_id: &str,
    metric_defs: &BTreeMap<String, Value>,
) -> String {
    metric_cache_key::metric_request_revision_fingerprint(
        app_root,
        datasets,
        base_dataset_id,
        metric_defs,
    )
}

pub fn metric_request_revision_fingerprint_for_compiled(
    app_root: &Path,
    compiled: &CompiledApp,
    base_dataset_id: &str,
    metric_defs: &BTreeMap<String, Value>,
) -> String {
    metric_cache_key::metric_request_revision_fingerprint_for_compiled(
        app_root,
        compiled,
        base_dataset_id,
        metric_defs,
    )
}

pub fn runtime_metric_eval_scope(
    binding_dataset: Option<&DatasetView>,
    base_dataset_id: &str,
    scene_id: &str,
    target: Option<&str>,
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
    query_state_override: Option<&QueryState>,
    filter_intents_override: &[FilterIntent],
    dependency_revision_key: &str,
    supplementary_binding_datasets: &[&DatasetView],
) -> Result<RuntimeMetricEvalScope> {
    let mut binding_datasets = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(primary) = binding_dataset {
        if seen.insert(primary.id.clone()) {
            binding_datasets.push(primary);
        }
    }
    for dataset in supplementary_binding_datasets {
        if seen.insert(dataset.id.clone()) {
            binding_datasets.push(*dataset);
        }
    }
    metric_cache_key::runtime_metric_eval_scope(
        &binding_datasets,
        base_dataset_id,
        scene_id,
        target,
        search,
        filters,
        query_state_override,
        filter_intents_override,
        dependency_revision_key,
    )
}

pub fn runtime_metric_workset(
    resource_id: &str,
    requested_metric_ids: &[String],
    dataset: &DatasetView,
) -> RuntimeMetricWorkset {
    let inner =
        metric_cache_key::runtime_metric_workset(resource_id, requested_metric_ids, dataset);
    RuntimeMetricWorkset {
        closure_metric_ids: inner.closure_metric_ids,
        eval_metric_ids: inner.eval_metric_ids,
        defs_for_hydrate: inner.defs_for_hydrate,
    }
}

pub fn eval_node_cache_key(expr_fingerprint: &str, scope: &RuntimeMetricEvalScope) -> String {
    metric_cache_key::eval_node_cache_key(expr_fingerprint, scope)
}

pub fn hydrate_file_backed_datasets_for_metric_defs(
    app_root: &Path,
    datasets: &mut BTreeMap<String, DatasetView>,
    metric_defs: &BTreeMap<String, Value>,
    query: &DatasetQueryOptions,
) -> Result<BTreeMap<String, u64>> {
    metric_hydrate::hydrate_file_backed_datasets_for_metric_defs(
        app_root,
        datasets,
        metric_defs,
        query,
    )
}

pub fn project_requested_metrics(
    resource_id: &str,
    request_metric_ids: &[String],
    runtime_metric_defs: &BTreeMap<String, Value>,
    metrics_map: &BTreeMap<String, MetricContract>,
) -> Vec<MetricContract> {
    if request_metric_ids.is_empty() {
        return metrics_map.values().cloned().collect();
    }
    request_metric_ids
        .iter()
        .filter_map(|metric_id| {
            let resolved = mei_lang_kernel::resolve_runtime_metric_def_key(
                resource_id,
                metric_id,
                runtime_metric_defs,
            )?;
            let mut metric = metrics_map.get(&resolved)?.clone();
            metric.id = metric_id.clone();
            Some(metric)
        })
        .collect()
}

pub fn dataset_resource<'a>(
    compiled: &'a CompiledApp,
    dataset_id: &str,
) -> Result<&'a LoadedResource> {
    mei_lang_kernel::locate_dataset_resource(compiled, dataset_id)
        .map_err(|error| anyhow::anyhow!("{error}"))
}
