//! 表格 runtime query contract V1：列元数据、摘要、query state 回显。

use std::collections::BTreeMap;

use mei_lang_kernel::{ColumnSchema, DatasetView};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types::{DatasetQueryOptions, DatasetQueryResult, TableColumnMeta, TableSummary};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableSortSpec {
    pub field: String,
    #[serde(default = "default_sort_direction")]
    pub direction: String,
}

fn default_sort_direction() -> String {
    "asc".to_string()
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryStateEcho {
    pub page: usize,
    pub page_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    pub filters: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<TableSortSpec>,
}

pub fn column_meta_from_dataset(dataset: &DatasetView, columns: &[String]) -> Vec<TableColumnMeta> {
    let schema_by_name: BTreeMap<String, &ColumnSchema> = dataset
        .schema
        .iter()
        .map(|col| (col.name.clone(), col))
        .collect();
    let names = if columns.is_empty() {
        dataset.columns.clone()
    } else {
        columns.to_vec()
    };
    names
        .into_iter()
        .map(|name| {
            if let Some(schema) = schema_by_name.get(name.as_str()) {
                TableColumnMeta {
                    name: schema.name.clone(),
                    type_name: schema.type_name.clone(),
                    sortable: true,
                    filterable: true,
                }
            } else {
                TableColumnMeta {
                    name: name.clone(),
                    type_name: "string".to_string(),
                    sortable: true,
                    filterable: true,
                }
            }
        })
        .collect()
}

pub fn enrich_table_result(
    dataset: &DatasetView,
    options: &DatasetQueryOptions,
    mut result: DatasetQueryResult,
) -> DatasetQueryResult {
    result.column_meta = column_meta_from_dataset(dataset, &result.columns);
    if options.summary {
        result.summary = Some(TableSummary {
            total: result.total,
        });
    }
    result.query_state_echo = Some(QueryStateEcho {
        page: result.page,
        page_size: result.page_size,
        search: options
            .search
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        filters: options.filters.clone(),
        sort: options.sort.clone(),
    });
    result
}

/// 请求体中的 sort / column_state / summary 解析为内部 options 字段。
pub fn apply_table_request_fields(
    options: &mut DatasetQueryOptions,
    sort: Vec<TableSortSpec>,
    column_state: Option<Value>,
    summary: bool,
) {
    options.sort = sort
        .into_iter()
        .filter(|spec| !spec.field.trim().is_empty())
        .collect();
    options.column_state = column_state;
    options.summary = summary;
}
