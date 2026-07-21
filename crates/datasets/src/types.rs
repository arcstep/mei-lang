use std::collections::BTreeMap;

use super::table_contract::{TableColumnState, TableSortSpec};
use mei_lang_kernel::QueryTimeRange;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct DatasetQueryOptions {
    pub page: usize,
    pub page_size: usize,
    pub search: Option<String>,
    pub filters: BTreeMap<String, String>,
    pub group: Vec<String>,
    pub time_range: Option<QueryTimeRange>,
    pub collect_all: bool,
    pub sort: Vec<TableSortSpec>,
    pub column_state: Option<TableColumnState>,
    pub summary: bool,
    /// When non-empty, query path computes SQL DISTINCT facet values for these columns
    /// (full filtered rowset — not limited to the returned page sample).
    pub facet_columns: Vec<String>,
}

impl Default for DatasetQueryOptions {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 0,
            search: None,
            filters: BTreeMap::new(),
            group: Vec::new(),
            time_range: None,
            collect_all: false,
            sort: Vec::new(),
            column_state: None,
            summary: false,
            facet_columns: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableColumnMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub sortable: bool,
    pub filterable: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableSummary {
    pub total: usize,
}

/// One facet bucket for filter-bar enum options (value + row count).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableColumnFacet {
    pub value: String,
    pub count: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatasetQueryResult {
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub has_more: bool,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub lazy: bool,
    pub perf: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_meta: Vec<TableColumnMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<TableSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_state_echo: Option<super::table_contract::QueryStateEcho>,
    /// Filtered-rowset facet buckets for requested `facet_columns` (top-N by count desc).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub column_facets: BTreeMap<String, Vec<TableColumnFacet>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct SourceMeta {
    #[serde(default)]
    pub(crate) lazy: LazyMeta,
    #[serde(default)]
    pub(crate) sheet: Option<String>,
    #[serde(default)]
    pub(crate) header_row: Option<i64>,
    #[serde(default)]
    pub(crate) normalize: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) filter_dimensions: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) table: Option<String>,
    #[serde(default)]
    pub(crate) query: Option<String>,
    #[serde(default)]
    pub(crate) connection: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct LazyMeta {
    #[serde(default)]
    pub(crate) default_page_size: Option<usize>,
    #[serde(default)]
    pub(crate) max_page_size: Option<usize>,
}

pub(crate) fn parse_source_meta(raw: Option<&str>) -> SourceMeta {
    raw.and_then(|value| serde_json::from_str::<SourceMeta>(value).ok())
        .unwrap_or_default()
}
