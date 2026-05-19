use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::types::{DatasetQueryOptions, DatasetQueryResult};
use super::util::value_to_text;

pub(crate) fn paginate_rows(
    rows: Vec<Value>,
    columns_hint: &[String],
    normalize: &BTreeMap<String, String>,
    options: &DatasetQueryOptions,
    lazy: bool,
) -> DatasetQueryResult {
    let collect_all = options.collect_all;
    let offset = if collect_all {
        0
    } else {
        options.page.saturating_sub(1) * options.page_size
    };
    let search = options
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase());
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
        }
    }
}
