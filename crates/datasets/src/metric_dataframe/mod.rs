//! 将 runtime metric（dataframe shape）物化后走统一分页/过滤管线。

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use mei_lang_kernel::{
    coerce_calendar_columns_in_rows, locate_dataset_resource, resolve_runtime_metric_def_key,
    runtime_eval_node_cache_enabled, ColumnSchema, CompiledApp, DatasetView, EvalPlanNodeKind,
    FilterIntent, MetricContract, MetricShape, QueryState,
};
use serde_json::Value;

use super::eval_artifact::{
    eval_artifact_hydrate_dataset_ids, load_or_build_runtime_metric_workset_artifact,
};
use super::eval_execute::execute_runtime_eval_plan_artifacts;
use super::metric_hydrate::hydrate_file_backed_datasets_for_metric_defs;
use super::metric_hydrate::{resolve_dataset_query_bindings_from_state, unique_dataset_views};
use super::metric_locate::locate_runtime_metric_resource;
use super::paginate::{infer_columns, paginate_rows};
use super::query::query_dataset_rows;
use super::result_artifact::{
    default_result_artifact_scope, load_metric_dataframe_result_artifact,
    store_metric_dataframe_result_artifact,
};
use super::table_contract::{column_meta_for_row_schema, format_rows_with_dataset_schema};
use super::types::{parse_source_meta, DatasetQueryOptions, DatasetQueryResult};
use super::util::elapsed_ms;
use super::{
    build_compiled_datasets_map, metric_dataframe_artifact_lookup_cache_keys,
    metric_request_revision_fingerprint_for_compiled, metric_scope_cache_key,
    query_state_from_request, runtime_metric_eval_scope, serialize_cache_value,
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

include!("cache.rs");
include!("query.rs");
include!("materialize.rs");
