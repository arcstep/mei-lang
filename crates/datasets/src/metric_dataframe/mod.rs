//! 将 runtime metric（dataframe shape）物化后走统一分页/过滤管线。

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use mei_lang_kernel::{
    coerce_calendar_columns_in_rows, locate_dataset_resource, resolve_runtime_metric_def_key,
    ColumnSchema, CompiledApp, FilterIntent, MetricContract, MetricShape, QueryState,
};
use moka::sync::Cache;
use serde_json::Value;

use super::eval_artifact::{
    eval_artifact_hydrate_dataset_ids, load_or_build_runtime_metric_workset_artifact,
};
use super::metric_locate::locate_runtime_metric_resource;
use super::paginate::{infer_columns, paginate_rows};
use super::result_artifact::{
    default_result_artifact_scope, load_metric_dataframe_result_artifact,
    store_metric_dataframe_result_artifact,
};
use super::table_contract::{column_meta_for_row_schema, format_rows_with_dataset_schema};
use super::types::{parse_source_meta, DatasetQueryOptions, DatasetQueryResult};
use super::util::elapsed_ms;
use super::{
    build_compiled_datasets_map, metric_dataframe_artifact_lookup_cache_keys, metric_scope_cache_key,
    query_state_from_request, serialize_cache_value,
};

pub const DEFAULT_PAGE_SIZE: usize = 20;
pub const MAX_PAGE_SIZE: usize = 1000;
const METRIC_DATAFRAME_CACHE_TTL_MS: u64 = 1500;
const METRIC_DATAFRAME_CACHE_MAX_BYTES: u64 = 16 * 1024 * 1024;
const METRIC_DATAFRAME_MAX_VALUE_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
struct MaterializedMetricDataframe {
    columns: Vec<String>,
    rows: Vec<Value>,
    row_schema: Vec<ColumnSchema>,
    normalize: BTreeMap<String, String>,
    base_perf: BTreeMap<String, u64>,
}

include!("cache.rs");
include!("query.rs");
include!("materialize.rs");
