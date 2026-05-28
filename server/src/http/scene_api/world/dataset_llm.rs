use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{locate_dataset_resource, DatasetView};
use serde_json::{json, Value};

use crate::http::datasets::{query_dataset_rows, DatasetQueryOptions};

use super::bundle::load_world_runtime_bundle;
use super::json_shrink::{json_serialized_len, shrink_json_for_llm};
use super::summaries::summarize_filters_decl;
use super::util::{normalize_limit, normalize_path};
use crate::http::scene_api::types::WorldScope;

pub(crate) const DATASET_QUERY_DEFAULT_LIMIT: usize = 10;
pub(crate) const DATASET_QUERY_MAX_LIMIT: usize = 50;
pub(crate) const DATASET_QUERY_DEFAULT_COLUMNS: usize = 10;
pub(crate) const DATASET_QUERY_MAX_COLUMNS: usize = 10;
pub(crate) const DATASET_QUERY_MAX_CELL_CHARS: usize = 50;
const DATASET_QUERY_TOTAL_CHAR_BUDGET: usize = 12_000;

fn normalize_dataset_limit(limit: Option<usize>) -> usize {
    normalize_limit(limit, DATASET_QUERY_DEFAULT_LIMIT, DATASET_QUERY_MAX_LIMIT)
}

fn dataset_available_columns(dataset: &DatasetView) -> Vec<String> {
    if !dataset.columns.is_empty() {
        return dataset.columns.clone();
    }
    dataset.schema.iter().map(|c| c.name.clone()).collect()
}

pub(crate) fn normalize_dataset_columns(
    dataset: &DatasetView,
    requested: Option<&[String]>,
) -> Vec<String> {
    let available = dataset_available_columns(dataset);
    let available_set = available.iter().cloned().collect::<BTreeSet<_>>();
    let mut selected = Vec::new();

    if let Some(req) = requested {
        for col in req {
            let name = col.trim();
            if name.is_empty() {
                continue;
            }
            if available_set.contains(name) && !selected.iter().any(|v| v == name) {
                selected.push(name.to_string());
            }
            if selected.len() >= DATASET_QUERY_MAX_COLUMNS {
                break;
            }
        }
    }

    if selected.is_empty() {
        selected = available
            .into_iter()
            .take(DATASET_QUERY_DEFAULT_COLUMNS)
            .collect();
    }
    selected
}

fn truncate_text_chars(input: &str, max_chars: usize) -> (String, bool) {
    if input.chars().count() <= max_chars {
        return (input.to_string(), false);
    }
    let mut out = input.chars().take(max_chars).collect::<String>();
    out.push('…');
    (out, true)
}

pub(super) fn bounded_cell_value(value: &Value, truncated_cells: &mut usize) -> Value {
    match value {
        Value::String(s) => {
            let (text, changed) = truncate_text_chars(s, DATASET_QUERY_MAX_CELL_CHARS);
            if changed {
                *truncated_cells += 1;
            }
            Value::String(text)
        }
        Value::Array(_) | Value::Object(_) => {
            let raw = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
            let (text, changed) = truncate_text_chars(&raw, DATASET_QUERY_MAX_CELL_CHARS);
            if changed {
                *truncated_cells += 1;
            }
            Value::String(text)
        }
        other => other.clone(),
    }
}

pub(crate) fn project_dataset_row(
    row: &Value,
    selected_columns: &[String],
    truncated_cells: &mut usize,
) -> Value {
    let mut out = serde_json::Map::new();
    if let Some(obj) = row.as_object() {
        for col in selected_columns {
            let value = obj
                .get(col)
                .map(|v| bounded_cell_value(v, truncated_cells))
                .unwrap_or(Value::Null);
            out.insert(col.clone(), value);
        }
        return Value::Object(out);
    }
    out.insert("_raw".to_string(), bounded_cell_value(row, truncated_cells));
    Value::Object(out)
}

fn build_schema_preview(dataset: &DatasetView, selected_columns: &[String]) -> Vec<Value> {
    let schema_map = dataset
        .schema
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect::<BTreeMap<_, _>>();
    selected_columns
        .iter()
        .map(|name| {
            if let Some(col) = schema_map.get(name.as_str()) {
                json!({
                    "name": col.name,
                    "type": col.type_name,
                    "source": col.source,
                    "optional": col.optional,
                })
            } else {
                json!({
                    "name": name,
                    "type": "unknown",
                })
            }
        })
        .collect()
}

pub(crate) fn query_world_dataset(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
    columns: Option<&[String]>,
    limit: Option<usize>,
) -> Result<Value> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let dataset_id = id.trim();
    if dataset_id.is_empty() {
        return Err(anyhow!("query parameter `id` is required"));
    }
    let loaded = locate_dataset_resource(&bundle.compiled, dataset_id)
        .map_err(|error| anyhow!("{error}"))?;
    let dataset = loaded
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_id}` is not a dataset"))?;

    let row_limit = normalize_dataset_limit(limit);
    let selected_columns = normalize_dataset_columns(dataset, columns);
    let app_root = source_root.join(app_id);
    let query_options = DatasetQueryOptions {
        page: 1,
        page_size: row_limit,
        search: search
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        filters: filters.clone(),
        collect_all: false,
        ..DatasetQueryOptions::default()
    };
    let query_result = query_dataset_rows(&app_root, dataset, query_options)?;

    let mut truncated_cells = 0usize;
    let sample_rows = query_result
        .rows
        .iter()
        .map(|row| project_dataset_row(row, &selected_columns, &mut truncated_cells))
        .collect::<Vec<_>>();

    let world_resource = bundle
        .contract
        .world
        .as_ref()
        .and_then(|w| w.resources.iter().find(|item| item.id == dataset_id));
    let metric_ids = world_resource
        .and_then(|item| item.metrics.as_ref())
        .map(|m| m.keys().take(64).cloned().collect::<Vec<_>>())
        .unwrap_or_else(|| dataset.metrics.keys().take(64).cloned().collect::<Vec<_>>());
    let filters_preview = world_resource
        .and_then(|item| item.filters.as_ref())
        .map(summarize_filters_decl)
        .unwrap_or(Value::Null);
    let schema_preview = build_schema_preview(dataset, &selected_columns);
    let schema_total_columns = if !dataset.schema.is_empty() {
        dataset.schema.len()
    } else {
        dataset.columns.len()
    };

    let mut payload = json!({
        "app_id": app_id,
        "scene_id": bundle.contract.scene.id,
        "id": dataset_id,
        "dataset": {
            "id": dataset.id.clone(),
            "title": dataset.title.clone(),
            "purpose": dataset.purpose.clone(),
            "source": {
                "kind": dataset.source.kind.clone(),
                "path": normalize_path(&dataset.source.path),
                "sheet": dataset.source.sheet.clone(),
            },
            "schema_preview": schema_preview,
            "schema_column_count": schema_total_columns,
            "filters": filters_preview,
            "metric_ids": metric_ids,
        },
        "sample_rows": sample_rows,
        "truncation": {
            "row_limit": row_limit,
            "column_limit": DATASET_QUERY_MAX_COLUMNS,
            "cell_char_limit": DATASET_QUERY_MAX_CELL_CHARS,
            "rows_returned": query_result.rows.len(),
            "columns_returned": selected_columns.len(),
            "cells_truncated": truncated_cells,
            "total_char_budget": DATASET_QUERY_TOTAL_CHAR_BUDGET,
            "total_chars_before_budget": 0,
            "total_chars_after_budget": 0,
        },
        "usage_hint": "若需更多数据，请在 dataset_query 中追加 filters/search/columns/limit；默认仅返回前10行与前10列的有界样例。",
    });
    let before = json_serialized_len(&payload);
    if let Some(v) = payload.pointer_mut("/truncation/total_chars_before_budget") {
        *v = json!(before);
    }
    if before > DATASET_QUERY_TOTAL_CHAR_BUDGET {
        payload = shrink_json_for_llm(&payload, DATASET_QUERY_TOTAL_CHAR_BUDGET);
    }
    let after = json_serialized_len(&payload);
    if let Some(v) = payload.pointer_mut("/truncation/total_chars_after_budget") {
        *v = json!(after);
    }
    Ok(payload)
}

pub(crate) fn query_world_dataset_metrics(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
    metric_ids: &[String],
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
) -> Result<Value> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let dataset_id = id.trim();
    if dataset_id.is_empty() {
        return Err(anyhow!("query parameter `id` is required"));
    }
    let loaded = locate_dataset_resource(&bundle.compiled, dataset_id)
        .map_err(|error| anyhow!("{error}"))?;
    let dataset = loaded
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_id}` is not a dataset"))?;
    if dataset.runtime_metric_defs.is_empty() {
        return Err(anyhow!("dataset `{dataset_id}` has no runtime metric defs"));
    }

    let app_root = source_root.join(app_id);
    let query_options = DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: search
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        filters: filters.clone(),
        collect_all: true,
        ..DatasetQueryOptions::default()
    };
    let filtered_rows = query_dataset_rows(&app_root, dataset, query_options)?;

    let mut runtime_dataset = dataset.clone();
    runtime_dataset.rows = filtered_rows.rows.clone();
    if !filtered_rows.columns.is_empty() {
        runtime_dataset.columns = filtered_rows.columns.clone();
    }

    let mut datasets = bundle
        .compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    datasets.insert(dataset_id.to_string(), runtime_dataset.clone());

    let metric_filter = if metric_ids.is_empty() {
        None
    } else {
        Some(metric_ids)
    };
    let metrics_map = mei_lang_kernel::evaluate_runtime_metric_defs(
        &dataset.runtime_metric_defs,
        &runtime_dataset.rows,
        &datasets,
        metric_filter,
    )?;
    let metrics = if metric_ids.is_empty() {
        metrics_map.into_values().collect::<Vec<_>>()
    } else {
        metric_ids
            .iter()
            .filter_map(|metric_id| metrics_map.get(metric_id).cloned())
            .collect::<Vec<_>>()
    };

    Ok(json!({
        "app_id": app_id,
        "scene_id": bundle.contract.scene.id,
        "dataset_id": dataset_id,
        "total_rows": runtime_dataset.rows.len(),
        "metrics": metrics,
        "usage_hint": "指标问答优先使用 dataset_metric；若要查看明细行，再改用 dataset_query。"
    }))
}
