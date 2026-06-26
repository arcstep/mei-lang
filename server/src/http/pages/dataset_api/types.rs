use mei_lang_kernel::{FilterIntent, QueryState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::http::datasets::{
    serde_lenient,
    table_contract::{TableColumnState, TableSortSpec},
};

#[derive(Debug, Deserialize)]
pub struct DatasetQueryRequest {
    /// Scene anchor (preferred). `dataset_id` is local to this scene.
    #[serde(default)]
    pub scene_id: Option<String>,
    /// Legacy source locator; used when `scene_id` is absent.
    #[serde(default)]
    pub target: Option<String>,
    pub dataset_id: String,
    #[serde(default, deserialize_with = "serde_lenient::opt_usize")]
    pub page: Option<usize>,
    #[serde(default, deserialize_with = "serde_lenient::opt_usize")]
    pub page_size: Option<usize>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default, deserialize_with = "serde_lenient::string_map")]
    pub filters: BTreeMap<String, String>,
    #[serde(default)]
    pub query_state: Option<QueryState>,
    #[serde(default)]
    pub filter_intents: Vec<FilterIntent>,
    #[serde(default, deserialize_with = "serde_lenient::bool_default_false")]
    pub full: bool,
    /// 非空时对 runtime metric（dataframe）求值后分页，与 dataset 行集共用过滤/分页语义。
    #[serde(default)]
    pub metric_id: Option<String>,
    #[serde(default)]
    pub sort: Vec<TableSortSpec>,
    #[serde(default)]
    pub column_state: Option<TableColumnState>,
    #[serde(default, deserialize_with = "serde_lenient::bool_default_false")]
    pub summary: bool,
}

#[derive(Debug, Serialize)]
pub struct DatasetQueryResponse {
    pub scene_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_path: Option<String>,
    pub dataset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_id: Option<String>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub has_more: bool,
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    pub lazy: bool,
    pub perf: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_meta: Vec<crate::http::datasets::TableColumnMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<crate::http::datasets::TableSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_state_echo: Option<crate::http::datasets::table_contract::QueryStateEcho>,
}

#[derive(Debug, Deserialize)]
pub struct DatasetRecomputeRequest {
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    pub dataset_id: String,
    #[serde(default)]
    pub metric_id: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DatasetRecomputeResponse {
    pub scene_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_path: Option<String>,
    pub dataset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_id: Option<String>,
    pub mode: String,
    pub compile_cache_cleared: usize,
    pub compiled_app_artifacts_cleared: usize,
    pub file_cache_cleared: usize,
    pub import_artifacts_cleared: usize,
    pub dataset_rows_cache_cleared: usize,
    pub eval_artifacts_cleared: usize,
    pub kernel_caches_cleared: bool,
    pub warmed: bool,
    pub perf: BTreeMap<String, u64>,
}
