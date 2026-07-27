mod agg_result_cache;
mod artifact_kv;
mod cache_partition;
mod csv_dataset;
mod dataset_rows_cache;
mod db_dataset;
mod postgres_dataset;
mod query_engine;
mod eval_artifact;
mod eval_cache_invalidation;
mod eval_cache_io_stats;
mod eval_execute;
mod file_cache;
mod geojson_dataset;
mod idempotency_key;
mod json_dataset;
mod l1_project;
mod metric_access;
mod metric_cache_key;
mod metric_dataframe;
mod metric_eval_inflight;
mod metric_hydrate;
mod metric_locate;
mod metric_pack_resolve;
mod metric_response_cache;
mod paginate;
mod paths;
mod query;
mod result_artifact;
pub mod schema_contract;
pub mod serde_lenient;
pub mod table_contract;
mod table_handle;
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

pub use artifact_kv::{
    clear_small_artifacts, load_small_artifact, remove_small_artifact,
    remove_small_artifacts_with_prefix, retain_small_artifact_keys,
    small_artifact_build_store_path, small_artifact_store_path,
    snapshot_small_artifact_store_stats, store_small_artifact, store_small_artifact_batch,
    SmallArtifactStoreStats,
};
pub use cache_partition::{partition_cache_key, partition_matches_key, partition_prefix};
pub use dataset_rows_cache::fallback_materialization_peak_bytes;
pub use eval_cache_invalidation::{
    invalidate_stale_eval_artifacts, metric_eval_artifact_reusable, EvalCacheInvalidationPlan,
    EvalCacheInvalidationReport,
};
pub use eval_cache_io_stats::{
    record_content_hash_dedupe_skips, reset_eval_cache_io_stats_for_tests, snapshot_eval_cache_io,
    take_eval_cache_io_delta, EvalCacheIoSnapshot,
};
pub use idempotency_key::{
    canonical_metric_idempotency_key, canonical_metric_shared_cache_key,
    metric_shared_cache_key_with_data_generation, resolve_metric_data_generation,
    resolve_metric_data_generation_with_runtime,
};
pub use metric_access::{
    build_compiled_datasets_map, collect_all_query_options, evaluate_runtime_metrics,
    evaluate_runtime_metrics_from_plan, materialize_query_options, runtime_metric_scope_requested,
    RuntimeMetricEvalMode, RuntimeMetricEvalOutcome,
};
pub use postgres_dataset::{clear_postgres_pool, is_postgres_kind, resolve_connection_dsn};
pub use metric_dataframe::metric_dataframe_result_cache_key;
pub use metric_eval_inflight::{
    reset_metric_eval_singleflight_stats_for_tests, run_metric_eval_singleflight,
    run_metric_response_artifact_load_singleflight, run_whole_eval_singleflight,
    snapshot_metric_eval_singleflight_stats, MetricEvalSingleflightStats, SingleflightOutcome,
    SingleflightRole,
};
pub use metric_locate::{
    locate_runtime_metric_resource, metric_ids_visible_for_dataset,
    plan_access_metric_eval_for_ids, AccessMetricEvalPlan,
};
pub use metric_pack_resolve::{try_load_disk_metric_response, DiskMetricResponseHit};
pub use metric_response_cache::{
    cached_metric_response_covers_request, clear_all_metric_caches,
    clear_demand_metric_response_cache, clear_metric_response_cache,
    clear_metric_response_cache_for_partition, configure_l1_pin_policy,
    configure_metric_response_cache_ttl_ms, current_l1_pin_policy, enforce_memory_pin_limits,
    enforce_memory_pin_limits_for_artifact, evict_metric_response_cache_key, last_l1_project_stats,
    mark_smart_warmup_triggered, memory_pinned_bytes, metric_contract_eligible_for_node_pack,
    metric_id_eligible_for_node_pack, metric_id_is_scalar_rowset,
    metric_response_cache_key_partitioned, metric_response_cache_scope_key,
    metric_response_prebuild_dataset_key, metric_response_prebuild_shared_key,
    populate_l1_from_loaded_metric_artifact, prebuild_metric_response_key_matches_dataset_query,
    project_metrics_map_for_l1, record_scope_cache_miss, request_needs_bulk_l1_metrics,
    should_trigger_smart_warmup, snapshot_moka_l1_stats, store_cached_metric_response,
    store_cached_metric_response_aliases, store_demand_metric_response,
    take_cached_metric_response, take_demand_metric_response, warm_from_artifact,
    CachedMetricResponse, L1PinPolicy, L1ProjectStats, MokaL1Stats,
};
pub use query::query_dataset_rows;
pub use result_artifact::{
    default_result_artifact_scope, load_metric_dataframe_result_artifact,
    load_metric_response_lite_artifact, load_metric_response_result_artifact,
    metric_dataframe_result_artifact_exists, metric_response_result_artifact_exists,
    snapshot_lite_artifact_io_stats, store_metric_dataframe_result_artifact,
    store_metric_response_lite_only, store_metric_response_result_artifact,
    take_lite_artifact_io_stats, take_metric_response_index_stats, LiteArtifactIoStats,
    LoadedMetricResponseArtifact, MetricResponseIndexStats,
};
pub use table_contract::{
    apply_table_request_fields, enrich_table_result, QueryStateEcho, TableColumnState,
    TableSortSpec,
};
pub use types::{
    DatasetQueryOptions, DatasetQueryResult, TableColumnFacet, TableColumnMeta, TableSummary,
};

/// 将 query_state 中的逻辑 filter 维度（如 agencies）映射为数据集列名（如 检查机构）。
pub fn map_dataset_query_filters(
    state: &QueryState,
    dataset: &DatasetView,
) -> BTreeMap<String, String> {
    metric_hydrate::resolve_dataset_query_bindings_from_state(state, dataset).mapped_filters
}

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

/// Clear `DatasetView.rows` JSON working sets (not the LRU row cache).
pub fn clear_dataset_view_rows(datasets: &mut BTreeMap<String, DatasetView>) -> usize {
    datasets
        .values_mut()
        .map(DatasetView::release_row_working_set)
        .sum()
}

/// Pack-First demand/warmup teardown: drop ephemeral row/table/file working sets.
///
/// Does **not** clear the per-app DataFusion session: concurrent metric requests
/// share registered `mei_pq_*` views, and dropping the session mid-flight causes
/// `table ... not found` planning errors. Session clear stays on full cache flush
/// (`clear_query_engine_sessions`) / app unload.
/// Does not clear L1 metric-response pins (already projected lite KPIs).
pub fn release_eval_working_set(app_root: &Path) -> ReleaseEvalWorkingSetReport {
    let report = ReleaseEvalWorkingSetReport {
        rows_cache: clear_dataset_rows_cache(),
        table_handles: table_handle::clear_table_handle_cache_for_app(app_root),
        dataframes: clear_metric_dataframe_result_cache(),
        external_files: clear_external_file_cache_for_app(app_root),
        df_sessions: 0,
        eval_nodes: mei_lang_kernel::clear_runtime_eval_node_cache(),
    };
    if report.touched() {
        tracing::debug!(
            rows_cache = report.rows_cache,
            table_handles = report.table_handles,
            dataframes = report.dataframes,
            external_files = report.external_files,
            df_sessions = report.df_sessions,
            eval_nodes = report.eval_nodes,
            "released eval working set"
        );
    }
    report
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReleaseEvalWorkingSetReport {
    pub rows_cache: usize,
    pub table_handles: usize,
    pub dataframes: usize,
    pub external_files: usize,
    pub df_sessions: usize,
    pub eval_nodes: usize,
}

impl ReleaseEvalWorkingSetReport {
    pub fn touched(self) -> bool {
        self.rows_cache > 0
            || self.table_handles > 0
            || self.dataframes > 0
            || self.external_files > 0
            || self.df_sessions > 0
            || self.eval_nodes > 0
    }
}

pub fn clear_agg_result_cache() -> usize {
    agg_result_cache::clear_agg_result_cache()
}

pub fn clear_table_handle_cache() -> usize {
    table_handle::clear_table_handle_cache()
}

pub fn clear_query_engine_sessions() -> usize {
    let n = query_engine::clear_query_engine_sessions();
    let _ = clear_postgres_pool(None);
    n
}

pub fn clear_query_engine_session_for_app(app_root: &Path) -> usize {
    let n = query_engine::clear_query_engine_session_for_app(app_root);
    let _ = clear_postgres_pool(Some(app_root));
    n
}

pub fn ensure_query_engine_session(app_root: &Path) -> Result<()> {
    query_engine::ensure_query_engine_session(app_root)
}

pub use query_engine::{
    resolve_parquet_file_for_source, snapshot_query_engine_io_stats, snapshot_pipeline_sql_stats,
    take_query_engine_io_stats, take_pipeline_sql_stats,
};
pub use schema_contract::{
    check_schema_physical_sources, ensure_schema_physical_sources, MissingPhysicalSource,
    SchemaPhysicalSourceCheck,
};

pub fn clear_eval_artifact_store(app_root: &Path) -> usize {
    eval_artifact::clear_eval_artifact_store(app_root)
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

pub fn metric_response_artifact_lookup_cache_keys(
    app_id: &str,
    app_root: &Path,
    compiled: &CompiledApp,
    scene_id: &str,
    scene_path: Option<&str>,
    primary_dataset_id: &str,
    owner_dataset: &DatasetView,
    query: &DatasetQueryOptions,
    compile_revision: &str,
    filter_intents: &[FilterIntent],
    prefer_prebuild_keys: bool,
    slot_revision: Option<&str>,
    dependency_metric_defs: Option<&BTreeMap<String, serde_json::Value>>,
) -> Vec<String> {
    metric_cache_key::metric_response_artifact_lookup_cache_keys(
        app_id,
        app_root,
        compiled,
        scene_id,
        scene_path,
        primary_dataset_id,
        owner_dataset,
        query,
        compile_revision,
        filter_intents,
        prefer_prebuild_keys,
        slot_revision,
        dependency_metric_defs,
    )
}

pub fn metric_dataframe_artifact_lookup_cache_keys(
    app_root: &Path,
    compiled: &CompiledApp,
    scene_id: Option<&str>,
    target: Option<&str>,
    primary_dataset_id: &str,
    owner_resource_id: &str,
    owner_dataset: &DatasetView,
    resolved_metric_id: &str,
    effective_metric_ids: &[String],
    options: &DatasetQueryOptions,
    compile_revision: &str,
    filter_intents: &[FilterIntent],
    defs_for_dependency: &BTreeMap<String, Value>,
) -> Vec<String> {
    metric_cache_key::metric_dataframe_artifact_lookup_cache_keys(
        app_root,
        compiled,
        scene_id,
        target,
        primary_dataset_id,
        owner_resource_id,
        owner_dataset,
        resolved_metric_id,
        effective_metric_ids,
        options,
        compile_revision,
        filter_intents,
        defs_for_dependency,
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
            let resolved = if !runtime_metric_defs.is_empty() {
                mei_lang_kernel::resolve_runtime_metric_def_key(
                    resource_id,
                    metric_id,
                    runtime_metric_defs,
                )
            } else {
                mei_lang_kernel::resolve_metric_contract_key(resource_id, metric_id, metrics_map)
            }?;
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
