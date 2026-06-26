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

