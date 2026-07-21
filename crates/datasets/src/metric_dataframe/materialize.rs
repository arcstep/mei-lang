fn paginate_materialized_metric_dataframe(
    materialized: &MaterializedMetricDataframe,
    meta: &super::types::SourceMeta,
    options: &DatasetQueryOptions,
    response_cache_key: &str,
    response_cache_lookup_ms: u64,
    from_materialized_cache: bool,
    metric_dataframe_eval_ms: Option<u64>,
) -> DatasetQueryResult {
    let default_page_size = meta
        .lazy
        .default_page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .max(1);
    let max_page_size = meta
        .lazy
        .max_page_size
        .unwrap_or(MAX_PAGE_SIZE)
        .max(default_page_size);
    let collect_all = options.collect_all;
    let page = if collect_all { 1 } else { options.page.max(1) };
    let page_size = if collect_all {
        0
    } else if options.page_size == 0 {
        default_page_size
    } else {
        options.page_size.clamp(1, max_page_size)
    };
    let normalized_options = DatasetQueryOptions {
        page,
        page_size,
        search: options.search.clone(),
        filters: options.filters.clone(),
        group: options.group.clone(),
        time_range: options.time_range.clone(),
        collect_all,
        sort: options.sort.clone(),
        column_state: options.column_state.clone(),
        summary: options.summary,
        facet_columns: options.facet_columns.clone(),
    };

    let mut result = paginate_rows(
        materialized.rows.clone(),
        &materialized.columns,
        &materialized.normalize,
        &normalized_options,
        true,
    );
    result.rows = coerce_calendar_columns_in_rows(
        std::mem::take(&mut result.rows),
        &result.columns,
        &materialized.row_schema,
    );
    if !materialized.row_schema.is_empty() {
        result.column_meta = column_meta_for_row_schema(&materialized.row_schema, &result.columns);
    }
    result.perf.extend(materialized.base_perf.clone());
    result.perf.insert("response_cache_hit".to_string(), 0);
    result.perf.insert("result_artifact_hit".to_string(), 0);
    result.perf.insert(
        "materialized_cache_hit".to_string(),
        u64::from(from_materialized_cache),
    );
    result.perf.insert(
        "response_cache_key_hash".to_string(),
        hash_fingerprint(response_cache_key),
    );
    result.perf.insert(
        "response_cache_lookup_ms".to_string(),
        response_cache_lookup_ms,
    );
    if let Some(eval_ms) = metric_dataframe_eval_ms {
        result
            .perf
            .insert("metric_dataframe_eval_ms".to_string(), eval_ms);
    }
    result
}

fn extract_dataframe_rows(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(rows) => rows.clone(),
        Value::Object(map) => {
            if let Some(rows) = map.get("rows").and_then(Value::as_array) {
                rows.clone()
            } else if let Some(rows) = map.get("value").and_then(Value::as_array) {
                rows.clone()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::extract_dataframe_rows;
    use serde_json::json;

    #[test]
    fn extract_dataframe_rows_from_array_and_wrappers() {
        assert_eq!(
            extract_dataframe_rows(&json!([{"a": 1}])),
            vec![json!({"a": 1})]
        );
        assert_eq!(
            extract_dataframe_rows(&json!({"rows": [{"a": 2}]})),
            vec![json!({"a": 2})]
        );
        assert_eq!(
            extract_dataframe_rows(&json!({"value": [{"a": 3}]})),
            vec![json!({"a": 3})]
        );
        assert!(extract_dataframe_rows(&json!(42)).is_empty());
    }
}
