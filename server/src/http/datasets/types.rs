use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct DatasetQueryOptions {
    pub page: usize,
    pub page_size: usize,
    pub search: Option<String>,
    pub filters: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatasetQueryResult {
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub has_more: bool,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub lazy: bool,
    pub perf: BTreeMap<String, u64>,
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
    pub(crate) table: Option<String>,
    #[serde(default)]
    pub(crate) query: Option<String>,
    #[serde(default)]
    pub(crate) connection: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct LazyMeta {
    #[serde(default)]
    pub(crate) enabled: Option<bool>,
    #[serde(default)]
    pub(crate) default_page_size: Option<usize>,
    #[serde(default)]
    pub(crate) max_page_size: Option<usize>,
}

pub(crate) fn parse_source_meta(raw: Option<&str>) -> SourceMeta {
    raw.and_then(|value| serde_json::from_str::<SourceMeta>(value).ok())
        .unwrap_or_default()
}
