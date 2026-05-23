//! 将 runtime metric（dataframe shape）物化后走统一分页/过滤管线。

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    evaluate_runtime_metric_defs, locate_dataset_resource, CompiledApp, MetricShape,
};
use serde_json::Value;

use super::paginate::paginate_rows;
use super::query::query_dataset_rows;
use super::types::{parse_source_meta, DatasetQueryOptions, DatasetQueryResult};
use super::util::elapsed_ms;

const DEFAULT_PAGE_SIZE: usize = 20;
const MAX_PAGE_SIZE: usize = 1000;

pub fn query_metric_dataframe(
    compiled: &CompiledApp,
    app_root: &Path,
    dataset_id: &str,
    metric_id: &str,
    options: DatasetQueryOptions,
) -> Result<DatasetQueryResult> {
    let resource =
        locate_dataset_resource(compiled, dataset_id).map_err(|error| anyhow!("{error}"))?;
    let dataset = resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_id}` is not a dataset"))?;
    if dataset.runtime_metric_defs.is_empty() {
        return Err(anyhow!("dataset `{dataset_id}` has no runtime metric defs"));
    }
    if !dataset.runtime_metric_defs.contains_key(metric_id) {
        return Err(anyhow!(
            "metric `{metric_id}` not found on dataset `{dataset_id}`"
        ));
    }

    let base_query = DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: options.search.clone(),
        filters: options.filters.clone(),
        collect_all: true,
    };
    let base_started = Instant::now();
    let filtered_rows = query_dataset_rows(app_root, dataset, base_query)?;
    let base_query_ms = elapsed_ms(base_started);

    let mut runtime_dataset = dataset.clone();
    runtime_dataset.rows = filtered_rows.rows.clone();
    if !filtered_rows.columns.is_empty() {
        runtime_dataset.columns = filtered_rows.columns.clone();
    }

    let mut datasets = compiled
        .resources
        .iter()
        .filter_map(|entry| entry.dataset.clone().map(|view| (entry.id.clone(), view)))
        .collect::<BTreeMap<_, _>>();
    datasets.insert(resource.id.clone(), runtime_dataset.clone());

    let metric_key = metric_id.to_string();
    let metric_started = Instant::now();
    let metrics_map = evaluate_runtime_metric_defs(
        &dataset.runtime_metric_defs,
        &runtime_dataset.rows,
        &datasets,
        Some(&[metric_key]),
    )?;
    let metric_eval_ms = elapsed_ms(metric_started);

    let metric = metrics_map
        .get(metric_id)
        .ok_or_else(|| anyhow!("metric `{metric_id}` evaluation returned nothing"))?;
    if metric.shape != MetricShape::Dataframe {
        return Err(anyhow!(
            "metric `{metric_id}` shape is {:?}, expected dataframe",
            metric.shape
        ));
    }

    let columns = metric
        .schema
        .iter()
        .map(|column| column.name.clone())
        .collect::<Vec<_>>();
    let rows = extract_dataframe_rows(&metric.value);

    let meta = parse_source_meta(dataset.source.content.as_deref());
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
        search: options.search,
        filters: options.filters,
        collect_all,
    };

    let mut result = paginate_rows(rows, &columns, &meta.normalize, &normalized_options, true);
    result.perf.extend(filtered_rows.perf);
    result
        .perf
        .insert("base_query_ms".to_string(), base_query_ms);
    result
        .perf
        .insert("metric_eval_ms".to_string(), metric_eval_ms);
    Ok(result)
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
