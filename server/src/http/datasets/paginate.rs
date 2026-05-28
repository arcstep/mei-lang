use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde_json::Value;

use super::table_contract::TableSortSpec;
use super::types::{DatasetQueryOptions, DatasetQueryResult};
use super::util::value_to_text;

pub(crate) fn paginate_rows(
    rows: Vec<Value>,
    columns_hint: &[String],
    normalize: &BTreeMap<String, String>,
    options: &DatasetQueryOptions,
    lazy: bool,
) -> DatasetQueryResult {
    if !options.sort.is_empty() {
        return paginate_rows_sorted(rows, columns_hint, normalize, options, lazy);
    }
    let collect_all = options.collect_all;
    let offset = if collect_all {
        0
    } else {
        options.page.saturating_sub(1) * options.page_size
    };
    let search = normalize_search(options.search.as_deref());
    let mut total = 0usize;
    let mut rows_page = Vec::new();
    let mut columns = if columns_hint.is_empty() {
        Vec::new()
    } else {
        columns_hint.to_vec()
    };
    for row in rows {
        let mut normalized = apply_normalize(row, normalize);
        if !row_matches(&normalized, &options.filters, search.as_deref()) {
            continue;
        }
        total += 1;
        if !collect_all && (total <= offset || rows_page.len() >= options.page_size) {
            continue;
        }
        if columns.is_empty() {
            if let Some(map) = normalized.as_object() {
                columns = map.keys().cloned().collect::<Vec<_>>();
            }
        }
        rows_page.push(std::mem::take(&mut normalized));
    }
    if columns.is_empty() {
        columns = infer_columns(&rows_page);
    }
    columns = output_columns(&columns, normalize);
    let has_more = if collect_all {
        false
    } else {
        total > offset + rows_page.len()
    };
    DatasetQueryResult {
        page: if collect_all { 1 } else { options.page },
        page_size: if collect_all {
            rows_page.len()
        } else {
            options.page_size
        },
        total,
        has_more,
        columns,
        rows: rows_page,
        lazy,
        perf: std::collections::BTreeMap::new(),
        column_meta: Vec::new(),
        summary: None,
        query_state_echo: None,
    }
}

fn paginate_rows_sorted(
    rows: Vec<Value>,
    columns_hint: &[String],
    normalize: &BTreeMap<String, String>,
    options: &DatasetQueryOptions,
    lazy: bool,
) -> DatasetQueryResult {
    let search = normalize_search(options.search.as_deref());
    let mut matched = Vec::new();
    let mut columns = if columns_hint.is_empty() {
        Vec::new()
    } else {
        columns_hint.to_vec()
    };
    for row in rows {
        let normalized = apply_normalize(row, normalize);
        if !row_matches(&normalized, &options.filters, search.as_deref()) {
            continue;
        }
        if columns.is_empty() {
            if let Some(map) = normalized.as_object() {
                columns = map.keys().cloned().collect::<Vec<_>>();
            }
        }
        matched.push(normalized);
    }
    matched.sort_by(|left, right| compare_rows(left, right, &options.sort));
    if columns.is_empty() {
        columns = infer_columns(&matched);
    }
    columns = output_columns(&columns, normalize);
    let total = matched.len();
    let collect_all = options.collect_all;
    let offset = if collect_all {
        0
    } else {
        options.page.saturating_sub(1) * options.page_size
    };
    let rows_page = if collect_all {
        matched
    } else {
        matched
            .into_iter()
            .skip(offset)
            .take(options.page_size)
            .collect()
    };
    let has_more = if collect_all {
        false
    } else {
        total > offset + rows_page.len()
    };
    DatasetQueryResult {
        page: if collect_all { 1 } else { options.page },
        page_size: if collect_all {
            rows_page.len()
        } else {
            options.page_size
        },
        total,
        has_more,
        columns,
        rows: rows_page,
        lazy,
        perf: std::collections::BTreeMap::new(),
        column_meta: Vec::new(),
        summary: None,
        query_state_echo: None,
    }
}

fn normalize_search(search: Option<&str>) -> Option<String> {
    search
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}

fn compare_rows(left: &Value, right: &Value, sort: &[TableSortSpec]) -> Ordering {
    let left_map = left.as_object();
    let right_map = right.as_object();
    for spec in sort {
        let field = spec.field.trim();
        if field.is_empty() {
            continue;
        }
        let left_value = left_map
            .and_then(|map| map.get(field))
            .unwrap_or(&Value::Null);
        let right_value = right_map
            .and_then(|map| map.get(field))
            .unwrap_or(&Value::Null);
        let ordering = compare_sort_values(left_value, right_value);
        if ordering != Ordering::Equal {
            return if spec.direction.eq_ignore_ascii_case("desc") {
                ordering.reverse()
            } else {
                ordering
            };
        }
    }
    Ordering::Equal
}

fn compare_sort_values(left: &Value, right: &Value) -> Ordering {
    let left_text = value_to_text(left);
    let right_text = value_to_text(right);
    if left_text.is_empty() && right_text.is_empty() {
        return Ordering::Equal;
    }
    if left_text.is_empty() {
        return Ordering::Greater;
    }
    if right_text.is_empty() {
        return Ordering::Less;
    }
    if let (Some(lhs), Some(rhs)) = (sort_number(left, &left_text), sort_number(right, &right_text)) {
        if let Some(ordering) = lhs.partial_cmp(&rhs) {
            return ordering;
        }
    }
    if let (Some(lhs), Some(rhs)) = (sort_datetime(&left_text), sort_datetime(&right_text)) {
        return lhs.cmp(&rhs);
    }
    left_text.cmp(&right_text)
}

fn sort_number(value: &Value, text: &str) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(raw) => raw.replace(',', "").parse::<f64>().ok(),
        _ => text.replace(',', "").parse::<f64>().ok(),
    }
}

fn sort_datetime(text: &str) -> Option<i64> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(text) {
        return Some(datetime.timestamp_millis());
    }
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y/%m/%d %H:%M:%S", "%Y/%m/%d %H:%M"] {
        if let Ok(datetime) = NaiveDateTime::parse_from_str(text, fmt) {
            return Some(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc).timestamp_millis());
        }
    }
    for fmt in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(date) = NaiveDate::parse_from_str(text, fmt) {
            let datetime = date.and_hms_opt(0, 0, 0)?;
            return Some(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc).timestamp_millis());
        }
    }
    None
}

pub(crate) fn row_matches(
    row: &Value,
    filters: &BTreeMap<String, String>,
    search: Option<&str>,
) -> bool {
    let Some(map) = row.as_object() else {
        return false;
    };
    for (key, expected) in filters {
        let expected = expected.trim();
        if expected.is_empty() {
            continue;
        }
        let actual = value_to_text(map.get(key).unwrap_or(&Value::Null));
        if !actual.contains(expected) {
            return false;
        }
    }
    if let Some(keyword) = search {
        let keyword = keyword.trim();
        if keyword.is_empty() {
            return true;
        }
        return map
            .values()
            .any(|value| value_to_text(value).to_lowercase().contains(keyword));
    }
    true
}

/// 查询结果列名：行经 `normalize` 后已是逻辑名，若 hint 仍是源表头则映射为逻辑名。
pub(crate) fn output_columns(
    columns_hint: &[String],
    normalize: &BTreeMap<String, String>,
) -> Vec<String> {
    if normalize.is_empty() {
        return columns_hint.to_vec();
    }
    if columns_hint.is_empty() {
        return normalize.values().cloned().collect();
    }
    if columns_hint
        .iter()
        .any(|name| normalize.contains_key(name.as_str()))
    {
        return columns_hint
            .iter()
            .filter_map(|name| normalize.get(name).cloned())
            .collect();
    }
    columns_hint.to_vec()
}

pub(crate) fn apply_normalize(row: Value, normalize: &BTreeMap<String, String>) -> Value {
    if normalize.is_empty() {
        return row;
    }
    let mut out = row.as_object().cloned().unwrap_or_default();
    for (source, target) in normalize {
        if source == target {
            continue;
        }
        if let Some(value) = out.remove(source) {
            out.insert(target.clone(), value);
        }
    }
    Value::Object(out)
}

pub(crate) fn infer_columns(rows: &[Value]) -> Vec<String> {
    let mut columns = BTreeSet::new();
    for row in rows {
        let Some(map) = row.as_object() else {
            continue;
        };
        for key in map.keys() {
            columns.insert(key.clone());
        }
    }
    columns.into_iter().collect()
}

pub(crate) fn empty_result(options: &DatasetQueryOptions, lazy: bool) -> DatasetQueryResult {
    DatasetQueryResult {
        page: if options.collect_all { 1 } else { options.page },
        page_size: if options.collect_all {
            0
        } else {
            options.page_size
        },
        total: 0,
        has_more: false,
        columns: Vec::new(),
        rows: Vec::new(),
        lazy,
        perf: std::collections::BTreeMap::new(),
        column_meta: Vec::new(),
        summary: None,
        query_state_echo: None,
    }
}

pub(crate) struct QueryWindow {
    page: usize,
    page_size: usize,
    offset: usize,
    matched: usize,
    rows: Vec<Value>,
    has_more: bool,
    collect_all: bool,
}

impl QueryWindow {
    pub(crate) fn new(options: &DatasetQueryOptions) -> Self {
        Self {
            page: if options.collect_all { 1 } else { options.page },
            page_size: options.page_size,
            offset: if options.collect_all {
                0
            } else {
                options.page.saturating_sub(1) * options.page_size
            },
            matched: 0,
            rows: Vec::new(),
            has_more: false,
            collect_all: options.collect_all,
        }
    }

    pub(crate) fn push(&mut self, row: Value) {
        self.matched += 1;
        if !self.collect_all && self.matched <= self.offset {
            return;
        }
        if self.collect_all || self.rows.len() < self.page_size {
            self.rows.push(row);
            return;
        }
        self.has_more = true;
    }

    pub(crate) fn should_stop(&self) -> bool {
        !self.collect_all && self.has_more
    }

    pub(crate) fn finish(self, columns: Vec<String>, lazy: bool) -> DatasetQueryResult {
        let total = if self.collect_all {
            self.rows.len()
        } else if self.has_more {
            self.offset + self.rows.len() + 1
        } else {
            self.matched
        };
        DatasetQueryResult {
            page: self.page,
            page_size: if self.collect_all {
                self.rows.len()
            } else {
                self.page_size
            },
            total,
            has_more: if self.collect_all {
                false
            } else {
                self.has_more
            },
            columns,
            rows: self.rows,
            lazy,
            perf: std::collections::BTreeMap::new(),
            column_meta: Vec::new(),
            summary: None,
            query_state_echo: None,
        }
    }
}
