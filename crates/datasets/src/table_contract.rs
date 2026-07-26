//! 表格 runtime query contract V1：列元数据、摘要、query state 回显。

use std::collections::BTreeMap;

use mei_lang_kernel::{
    coerce_calendar_columns_in_rows, coerce_rows_to_schema, ColumnSchema, DatasetView,
    QueryTimeRange, SourceDecl,
};
use serde::{Deserialize, Serialize};

use super::serde_lenient;
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QueryStateEcho {
    pub page: usize,
    pub page_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    pub filters: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub group: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_range: Option<QueryTimeRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<TableSortSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_state: Option<TableColumnState>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct TableColumnState {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<TableColumnStateItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
pub struct TableColumnStateItem {
    pub key: String,
    #[serde(
        default,
        deserialize_with = "serde_lenient::bool_default_false",
        skip_serializing_if = "is_false"
    )]
    pub hidden: bool,
    #[serde(
        default,
        deserialize_with = "serde_lenient::opt_i64",
        skip_serializing_if = "Option::is_none"
    )]
    pub order: Option<i64>,
    #[serde(
        default,
        deserialize_with = "serde_lenient::opt_usize",
        skip_serializing_if = "Option::is_none"
    )]
    pub width: Option<usize>,
    #[serde(
        default,
        deserialize_with = "serde_lenient::opt_usize",
        skip_serializing_if = "Option::is_none"
    )]
    pub min_width: Option<usize>,
    #[serde(
        default,
        deserialize_with = "serde_lenient::opt_usize",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_width: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valign: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_valign: Option<String>,
    #[serde(
        default,
        deserialize_with = "serde_lenient::opt_bool",
        skip_serializing_if = "Option::is_none"
    )]
    pub wrap: Option<bool>,
    #[serde(
        default,
        deserialize_with = "serde_lenient::opt_bool",
        skip_serializing_if = "Option::is_none"
    )]
    pub header_wrap: Option<bool>,
}

fn is_false(value: &bool) -> bool {
    !*value
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

/// 在多个 dataset schema 中，为结果列挑选匹配列数最多的 schema 子集（用于 metric dataframe 明细）。
/// 同时按 `column.source`（如 `__EMPTY`）匹配，避免懒加载行键未 remap 时选不到逻辑列 schema。
pub fn resolve_row_schema_for_columns(
    columns: &[String],
    datasets: &BTreeMap<String, DatasetView>,
) -> Vec<ColumnSchema> {
    if columns.is_empty() {
        return Vec::new();
    }
    let mut best_schema: Option<&[ColumnSchema]> = None;
    let mut best_matched = 0usize;
    for view in datasets.values() {
        if view.schema.is_empty() {
            continue;
        }
        let matched = columns
            .iter()
            .filter(|name| schema_matches_column_or_source(&view.schema, name.as_str()))
            .count();
        if matched > best_matched {
            best_matched = matched;
            best_schema = Some(view.schema.as_slice());
        }
    }
    let Some(schema) = best_schema else {
        return Vec::new();
    };
    columns
        .iter()
        .filter_map(|name| find_schema_column(schema, name.as_str()).cloned())
        .collect()
}

fn schema_matches_column_or_source(schema: &[ColumnSchema], name: &str) -> bool {
    find_schema_column(schema, name).is_some()
}

fn find_schema_column<'a>(schema: &'a [ColumnSchema], name: &str) -> Option<&'a ColumnSchema> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    schema.iter().find(|col| col.name == trimmed).or_else(|| {
        schema.iter().find(|col| {
            col.source
                .as_deref()
                .map(str::trim)
                .is_some_and(|source| source == trimmed)
        })
    })
}

/// 选对当前行键覆盖度最高的完整 dataset schema（用于 source→name remap）。
fn resolve_best_full_schema_for_rows(
    columns: &[String],
    rows: &[serde_json::Value],
    datasets: &BTreeMap<String, DatasetView>,
) -> Vec<ColumnSchema> {
    let probe_keys: Vec<String> = if !columns.is_empty() {
        columns.to_vec()
    } else {
        rows.first()
            .and_then(|row| row.as_object())
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    };
    if probe_keys.is_empty() {
        return Vec::new();
    }
    let mut best: Option<&[ColumnSchema]> = None;
    let mut best_matched = 0usize;
    for view in datasets.values() {
        if view.schema.is_empty() {
            continue;
        }
        let matched = probe_keys
            .iter()
            .filter(|name| schema_matches_column_or_source(&view.schema, name.as_str()))
            .count();
        // Prefer schemas that actually declare source aliases when tied.
        let alias_bonus = view
            .schema
            .iter()
            .filter(|col| {
                col.source
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|source| !source.is_empty() && source != col.name)
            })
            .count();
        let score = matched.saturating_mul(100).saturating_add(alias_bonus);
        if score > best_matched {
            best_matched = score;
            best = Some(view.schema.as_slice());
        }
    }
    best.map(|schema| schema.to_vec()).unwrap_or_default()
}

pub fn format_rows_with_dataset_schema(
    columns: &[String],
    rows: Vec<serde_json::Value>,
    datasets: &BTreeMap<String, DatasetView>,
) -> (Vec<ColumnSchema>, Vec<serde_json::Value>) {
    // 先按完整 dataset schema 做 source→name（`__EMPTY`→`序号` 等），再按请求列裁剪 meta。
    let full_schema = resolve_best_full_schema_for_rows(columns, &rows, datasets);
    let rows = if full_schema.is_empty() {
        rows
    } else {
        coerce_rows_to_schema(rows, &full_schema)
    };
    let schema = resolve_row_schema_for_columns(columns, datasets);
    if schema.is_empty() {
        // 请求列仍是源键时，回退为完整 schema（已 remap 后的逻辑列）。
        return (full_schema, rows);
    }
    (schema, rows)
}

pub fn column_meta_for_row_schema(
    schema: &[ColumnSchema],
    columns: &[String],
) -> Vec<TableColumnMeta> {
    let view = DatasetView {
        id: String::new(),
        title: None,
        purpose: None,
        schema: schema.to_vec(),
        stage_schema: Vec::new(),
        columns: columns.to_vec(),
        rows: Vec::new(),
        source: SourceDecl {
            kind: String::new(),
            path: String::new(),
            sheet: None,
            header_row: None,
            preview_rows: None,
            page_size: None,
            max_page_size: None,
            table: None,
            query: None,
            connection: None,
            content: None,
        },
        sources: Vec::new(),
        metrics: BTreeMap::new(),
        runtime_metric_defs: BTreeMap::new(),
        runtime_analysis_graph: Default::default(),
        runtime_analysis_contracts: Default::default(),
    };
    column_meta_from_dataset(&view, columns)
}

pub fn enrich_table_result(
    dataset: &DatasetView,
    options: &DatasetQueryOptions,
    mut result: DatasetQueryResult,
) -> DatasetQueryResult {
    if result.column_meta.is_empty() {
        result.column_meta = column_meta_from_dataset(dataset, &result.columns);
    }
    result.rows = coerce_calendar_columns_in_rows(
        std::mem::take(&mut result.rows),
        &result.columns,
        &dataset.schema,
    );
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
        group: options.group.clone(),
        time_range: options.time_range.clone(),
        sort: options.sort.clone(),
        column_state: options.column_state.clone(),
    });
    result
}

/// 请求体中的 sort / column_state / summary 解析为内部 options 字段。
pub fn apply_table_request_fields(
    options: &mut DatasetQueryOptions,
    sort: Vec<TableSortSpec>,
    column_state: Option<TableColumnState>,
    summary: bool,
) {
    options.sort = sort
        .into_iter()
        .filter(|spec| !spec.field.trim().is_empty())
        .collect();
    options.column_state = column_state;
    options.summary = summary;
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{ColumnSchema, DatasetView, SourceDecl};
    use serde_json::json;

    fn sample_dataset(id: &str, schema: Vec<ColumnSchema>) -> DatasetView {
        DatasetView {
            id: id.to_string(),
            title: None,
            purpose: None,
            schema,
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "xlsx".to_string(),
                path: "demo.xlsx".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: Default::default(),
        }
    }

    #[test]
    fn resolve_row_schema_picks_best_matching_dataset() {
        let warning_schema = vec![
            ColumnSchema {
                name: "预警时间".to_string(),
                type_name: "date".to_string(),
                source: None,
                optional: false,
                unit: None,
            },
            ColumnSchema {
                name: "分办时间".to_string(),
                type_name: "date".to_string(),
                source: None,
                optional: false,
                unit: None,
            },
        ];
        let datasets = BTreeMap::from([
            (
                "warning_list".to_string(),
                sample_dataset("warning_list", warning_schema),
            ),
            (
                "metrics".to_string(),
                sample_dataset(
                    "metrics",
                    vec![ColumnSchema {
                        name: "value".to_string(),
                        type_name: "number".to_string(),
                        source: None,
                        optional: false,
                        unit: None,
                    }],
                ),
            ),
        ]);
        let columns = vec![
            "预警ID".to_string(),
            "预警时间".to_string(),
            "分办时间".to_string(),
        ];
        let schema = resolve_row_schema_for_columns(&columns, &datasets);
        assert_eq!(schema.len(), 2);
        assert_eq!(schema[0].name, "预警时间");
        assert_eq!(schema[0].type_name, "date");
    }

    #[test]
    fn format_rows_with_dataset_schema_coerces_calendar_dates() {
        let schema = vec![ColumnSchema {
            name: "办结时间".to_string(),
            type_name: "date".to_string(),
            source: None,
            optional: false,
            unit: None,
        }];
        let datasets = BTreeMap::from([(
            "warning_list".to_string(),
            sample_dataset("warning_list", schema),
        )]);
        let columns = vec!["办结时间".to_string()];
        let rows = vec![json!({"办结时间": "2025-10-01 00:00:00"})];
        let (row_schema, out) = format_rows_with_dataset_schema(&columns, rows, &datasets);
        assert_eq!(row_schema.len(), 1);
        assert_eq!(out[0]["办结时间"], "2025-10-01");
    }

    #[test]
    fn enrich_table_result_preserves_existing_column_meta() {
        use super::super::types::DatasetQueryResult;

        let dataset = sample_dataset("owner", Vec::new());
        let result = enrich_table_result(
            &dataset,
            &DatasetQueryOptions::default(),
            DatasetQueryResult {
                page: 1,
                page_size: 20,
                total: 0,
                has_more: false,
                columns: vec!["预警时间".to_string()],
                rows: Vec::new(),
                lazy: false,
                perf: BTreeMap::new(),
                column_meta: vec![TableColumnMeta {
                    name: "预警时间".to_string(),
                    type_name: "date".to_string(),
                    sortable: true,
                    filterable: true,
                }],
                summary: None,
                query_state_echo: None,
                column_facets: BTreeMap::new(),
            },
        );
        assert_eq!(result.column_meta[0].type_name, "date");
    }
}

#[cfg(test)]
mod sort_request_parse_tests {
    use super::TableSortSpec;
    use serde_json::json;

    #[test]
    fn parse_chinese_serial_sort() {
        let v = json!([{"field":"序号","direction":"asc"}]);
        let parsed: Vec<TableSortSpec> = serde_json::from_value(v).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].field, "序号");
        assert_eq!(parsed[0].direction, "asc");
    }
}
