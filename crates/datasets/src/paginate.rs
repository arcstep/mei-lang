use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Utc};
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
    paginate_rows_iter(rows, columns_hint, normalize, options, lazy)
}

pub(crate) fn paginate_rows_iter<I>(
    rows: I,
    columns_hint: &[String],
    normalize: &BTreeMap<String, String>,
    options: &DatasetQueryOptions,
    lazy: bool,
) -> DatasetQueryResult
where
    I: IntoIterator<Item = Value>,
{
    if !options.sort.is_empty() {
        return paginate_rows_sorted_iter(rows, columns_hint, normalize, options, lazy);
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

fn paginate_rows_sorted_iter<I>(
    rows: I,
    columns_hint: &[String],
    normalize: &BTreeMap<String, String>,
    options: &DatasetQueryOptions,
    lazy: bool,
) -> DatasetQueryResult
where
    I: IntoIterator<Item = Value>,
{
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

pub(crate) fn normalize_search(search: Option<&str>) -> Option<String> {
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
    if let (Some(lhs), Some(rhs)) = (
        sort_number(left, &left_text),
        sort_number(right, &right_text),
    ) {
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
    for fmt in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M:%S",
        "%Y/%m/%d %H:%M",
    ] {
        if let Ok(datetime) = NaiveDateTime::parse_from_str(text, fmt) {
            return Some(
                DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc).timestamp_millis(),
            );
        }
    }
    for fmt in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(date) = NaiveDate::parse_from_str(text, fmt) {
            let datetime = date.and_hms_opt(0, 0, 0)?;
            return Some(
                DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc).timestamp_millis(),
            );
        }
    }
    None
}

#[derive(Debug, Clone)]
enum FilterSpec {
    Contains(String),
    InValues(Vec<String>),
    Month(Vec<String>),
    MonthRange { start: String, end: String },
    DateRange { start: String, end: String },
    NumCompare { op: NumCompareOp, value: f64 },
    Not(Box<FilterSpec>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumCompareOp {
    Eq,
    Gt,
    Gte,
    Lt,
    Lte,
}

fn parse_filter_number(raw: &str) -> Option<f64> {
    let text = raw.trim().replace(',', "");
    if text.is_empty() {
        return None;
    }
    text.parse::<f64>().ok()
}

fn parse_filter_spec(expected: &str) -> FilterSpec {
    let trimmed = expected.trim();
    if trimmed.is_empty() {
        return FilterSpec::Contains(String::new());
    }
    if let Some(rest) = trimmed.strip_prefix("not:") {
        return FilterSpec::Not(Box::new(parse_filter_spec(rest)));
    }
    if let Some(rest) = trimmed.strip_prefix("eq:") {
        if let Some(value) = parse_filter_number(rest) {
            return FilterSpec::NumCompare {
                op: NumCompareOp::Eq,
                value,
            };
        }
    }
    if let Some(rest) = trimmed.strip_prefix("gte:") {
        if let Some(value) = parse_filter_number(rest) {
            return FilterSpec::NumCompare {
                op: NumCompareOp::Gte,
                value,
            };
        }
    }
    if let Some(rest) = trimmed.strip_prefix("gt:") {
        if let Some(value) = parse_filter_number(rest) {
            return FilterSpec::NumCompare {
                op: NumCompareOp::Gt,
                value,
            };
        }
    }
    if let Some(rest) = trimmed.strip_prefix("lte:") {
        if let Some(value) = parse_filter_number(rest) {
            return FilterSpec::NumCompare {
                op: NumCompareOp::Lte,
                value,
            };
        }
    }
    if let Some(rest) = trimmed.strip_prefix("lt:") {
        if let Some(value) = parse_filter_number(rest) {
            return FilterSpec::NumCompare {
                op: NumCompareOp::Lt,
                value,
            };
        }
    }
    if let Some(rest) = trimmed.strip_prefix("mrange:") {
        if let Some((start, end)) = rest.split_once("..") {
            let start = start.trim();
            let end = end.trim();
            if !start.is_empty() && !end.is_empty() {
                return FilterSpec::MonthRange {
                    start: start.to_string(),
                    end: end.to_string(),
                };
            }
        }
    }
    if let Some(rest) = trimmed.strip_prefix("drange:") {
        if let Some((start, end)) = rest.split_once("..") {
            let start = start.trim();
            let end = end.trim();
            if !start.is_empty() && !end.is_empty() {
                return FilterSpec::DateRange {
                    start: start.to_string(),
                    end: end.to_string(),
                };
            }
        }
    }
    if let Some(rest) = trimmed.strip_prefix("in:") {
        return FilterSpec::InValues(
            split_filter_values(rest)
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
    }
    if let Some(rest) = trimmed.strip_prefix("m:") {
        return FilterSpec::Month(
            split_filter_values(rest)
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
    }
    if let Some(rest) = trimmed.strip_prefix("contains:") {
        return FilterSpec::Contains(rest.to_string());
    }
    FilterSpec::Contains(trimmed.to_string())
}

fn eval_num_compare(actual: f64, op: NumCompareOp, expected: f64) -> bool {
    const EPS: f64 = 1e-9;
    match op {
        NumCompareOp::Eq => (actual - expected).abs() <= EPS,
        NumCompareOp::Gt => actual > expected,
        NumCompareOp::Gte => actual >= expected - EPS,
        NumCompareOp::Lt => actual < expected,
        NumCompareOp::Lte => actual <= expected + EPS,
    }
}

fn eval_filter_spec(actual: &str, spec: &FilterSpec) -> bool {
    match spec {
        FilterSpec::Not(inner) => !eval_filter_spec(actual, inner),
        FilterSpec::Contains(needle) => {
            if needle.is_empty() {
                return true;
            }
            actual.contains(needle.as_str())
        }
        FilterSpec::InValues(values) => values.iter().any(|part| actual == part.as_str()),
        FilterSpec::Month(values) => {
            let Some(actual_month) = extract_year_month(actual) else {
                return false;
            };
            values.iter().any(|part| actual_month == *part)
        }
        FilterSpec::MonthRange { start, end } => {
            let Some(actual_month) = extract_year_month(actual) else {
                return false;
            };
            actual_month.as_str() >= start.as_str() && actual_month.as_str() <= end.as_str()
        }
        FilterSpec::DateRange { start, end } => {
            let Some(actual_ord) = sort_datetime(actual) else {
                return false;
            };
            let Some(start_ord) = sort_datetime(start) else {
                return false;
            };
            let Some(end_ord) = sort_datetime(end) else {
                return false;
            };
            actual_ord >= start_ord && actual_ord <= end_ord
        }
        FilterSpec::NumCompare { op, value } => parse_filter_number(actual)
            .is_some_and(|actual_value| eval_num_compare(actual_value, *op, *value)),
    }
}

fn split_filter_values(raw: &str) -> Vec<&str> {
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn extract_year_month(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.len() >= 7 && trimmed.as_bytes().get(4) == Some(&b'-') {
        let prefix = &trimmed[..7];
        if prefix.chars().take(4).all(|ch| ch.is_ascii_digit())
            && prefix.as_bytes().get(4) == Some(&b'-')
            && prefix[5..].chars().all(|ch| ch.is_ascii_digit())
        {
            return Some(prefix.to_string());
        }
    }
    if let Some(ms) = sort_datetime(trimmed) {
        let datetime = DateTime::<Utc>::from_timestamp_millis(ms)?;
        return Some(format!("{:04}-{:02}", datetime.year(), datetime.month()));
    }
    None
}

fn row_matches_filter_value(actual: &str, expected: &str) -> bool {
    eval_filter_spec(actual, &parse_filter_spec(expected))
}

#[cfg(test)]
mod filter_spec_tests {
    use super::{eval_filter_spec, parse_filter_spec, FilterSpec};

    #[test]
    fn parse_not_in_values() {
        let spec = parse_filter_spec("not:in:红,黄");
        match &spec {
            FilterSpec::Not(inner) => match inner.as_ref() {
                FilterSpec::InValues(values) => assert_eq!(values, &vec!["红", "黄"]),
                _ => panic!("expected in values"),
            },
            _ => panic!("expected not"),
        }
        assert!(!eval_filter_spec("红", &spec));
        assert!(eval_filter_spec("蓝", &spec));
    }

    #[test]
    fn parse_numeric_gte() {
        let spec = parse_filter_spec("gte:10");
        assert!(eval_filter_spec("12", &spec));
        assert!(eval_filter_spec("10", &spec));
        assert!(!eval_filter_spec("9", &spec));
    }

    #[test]
    fn parse_month_range() {
        let spec = parse_filter_spec("mrange:2024-01..2024-06");
        assert!(eval_filter_spec("2024-03-15", &spec));
        assert!(!eval_filter_spec("2023-12-01", &spec));
    }

    #[test]
    fn parse_date_range() {
        let spec = parse_filter_spec("drange:2024-01-15..2024-06-30");
        assert!(eval_filter_spec("2024-03-15", &spec));
        assert!(eval_filter_spec("2024-01-15", &spec));
        assert!(eval_filter_spec("2024-06-30", &spec));
        assert!(!eval_filter_spec("2024-01-14", &spec));
        assert!(!eval_filter_spec("2024-07-01", &spec));
    }
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
        if !row_matches_filter_value(&actual, expected) {
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
