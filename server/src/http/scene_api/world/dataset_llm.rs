use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    evaluate_runtime_metric_defs_with_scope, locate_dataset_resource, resolve_runtime_metric_def_key,
    DatasetView, MetricContract,
};
use serde_json::{json, Value};

use crate::http::datasets::{
    hydrate_file_backed_datasets_for_metric_defs, metric_ids_visible_for_dataset,
    metric_request_revision_fingerprint, normalize_query_filters, normalize_query_search,
    plan_access_metric_eval_for_ids, query_dataset_rows, query_state_from_request,
    runtime_metric_eval_scope, runtime_metric_workset, DatasetQueryOptions,
};

use super::analysis_contract_llm::{
    build_dataset_analysis_contracts_preview_for_access, build_metric_analysis_contract_attachments,
    contract_hint_when_empty, contract_hint_when_preview_empty,
};
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
    let metric_ids = metric_ids_visible_for_dataset(
        &bundle.compiled,
        dataset,
        world_resource.and_then(|item| item.metrics.as_ref()),
    );
    let analysis_contracts_preview = build_dataset_analysis_contracts_preview_for_access(
        &bundle.compiled,
        dataset,
        &loaded.id,
        &metric_ids,
        world_resource.and_then(|item| item.metrics.as_ref()),
    );
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

    let contract_hint = contract_hint_when_preview_empty(&analysis_contracts_preview);
    let mut payload = json!({
        "app_id": app_id,
        "scene_id": bundle.contract.scene.id,
        "id": dataset_id,
        "contract_hint": contract_hint,
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
            "analysis_contracts_preview": analysis_contracts_preview,
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
        "usage_hint": "若需更多数据，请在 dataset_query 中追加 filters/search/columns/limit；默认仅返回前10行与前10列的有界样例。带 explain 的指标请同时查看 dataset.analysis_contracts_preview，与宿主 UI 的 analysis_contract 同轨。",
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
    let eval_plan = plan_access_metric_eval_for_ids(&bundle.compiled, dataset_id, metric_ids)?;
    let primary_dataset = eval_plan.primary_dataset;
    let owner_dataset = eval_plan.owner_dataset;

    let app_root = source_root.join(app_id);
    let normalized_search = normalize_query_search(search);
    let normalized_filters = normalize_query_filters(filters);
    let effective_query_state = query_state_from_request(&normalized_filters, normalized_search.as_deref(), None);
    let query_options = DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: effective_query_state.search.clone(),
        filters: effective_query_state.filters.clone(),
        group: effective_query_state.group.clone(),
        time_range: effective_query_state.time_range.clone(),
        collect_all: true,
        ..DatasetQueryOptions::default()
    };
    let filtered_rows = query_dataset_rows(&app_root, primary_dataset, query_options)?;

    let mut runtime_dataset = primary_dataset.clone();
    runtime_dataset.rows = filtered_rows.rows.clone();
    if !filtered_rows.columns.is_empty() {
        runtime_dataset.columns = filtered_rows.columns.clone();
    }

    let mut datasets = bundle
        .compiled
        .resources
        .iter()
        .filter_map(|resource| resource.dataset.clone().map(|dataset| (resource.id.clone(), dataset)))
        .fold(BTreeMap::new(), |mut acc, (resource_id, dataset)| {
            acc.insert(resource_id, dataset.clone());
            acc.entry(dataset.id.clone()).or_insert(dataset);
            acc
        });
    datasets.insert(eval_plan.primary.id.clone(), runtime_dataset.clone());
    let workset = runtime_metric_workset(
        &eval_plan.owner.id,
        &eval_plan.request_metric_ids,
        owner_dataset,
    );
    let metric_filter = workset.eval_metric_ids.as_deref();
    let defs_for_hydrate = workset.defs_for_hydrate.clone();
    let dependency_revision_key = metric_request_revision_fingerprint(
        &app_root,
        &datasets,
        &eval_plan.owner.id,
        &defs_for_hydrate,
    );
    hydrate_file_backed_datasets_for_metric_defs(
        &app_root,
        &mut datasets,
        &defs_for_hydrate,
        &DatasetQueryOptions {
            page: 1,
            page_size: 0,
            search: effective_query_state.search.clone(),
            filters: effective_query_state.filters.clone(),
            group: effective_query_state.group.clone(),
            time_range: effective_query_state.time_range.clone(),
            collect_all: true,
            ..DatasetQueryOptions::default()
        },
    )?;
    let eval_scope = runtime_metric_eval_scope(
        Some(primary_dataset),
        &eval_plan.primary.id,
        &bundle.contract.scene.id,
        Some(bundle.active_target_file.as_str()),
        effective_query_state.search.as_deref(),
        &effective_query_state.filters,
        Some(&effective_query_state),
        &[],
        &dependency_revision_key,
    )?;
    let metrics_map = evaluate_runtime_metric_defs_with_scope(
        &owner_dataset.runtime_metric_defs,
        &runtime_dataset.rows,
        &datasets,
        metric_filter,
        &eval_scope,
    )?;
    let metrics = if metric_ids.is_empty() {
        metrics_map.into_values().collect::<Vec<_>>()
    } else {
        project_requested_metrics(
            &eval_plan.owner.id,
            &eval_plan.request_metric_ids,
            &owner_dataset.runtime_metric_defs,
            &metrics_map,
        )
    };
    let requested_metric_ids = eval_plan.request_metric_ids.clone();
    let analysis_contracts = build_metric_analysis_contract_attachments(
        &bundle.compiled,
        primary_dataset,
        &eval_plan.primary.id,
        &requested_metric_ids,
    );
    let contract_hint = contract_hint_when_empty(&analysis_contracts);

    Ok(json!({
        "app_id": app_id,
        "scene_id": bundle.contract.scene.id,
        "dataset_id": dataset_id,
        "total_rows": runtime_dataset.rows.len(),
        "metrics": metrics,
        "analysis_contracts": analysis_contracts,
        "contract_hint": contract_hint,
        "usage_hint": "指标问答优先使用 dataset_metric；analysis_contracts 与宿主 UI popup/route 同轨。无 contract 时勿编造 explain 字段。若要查看明细行，再改用 dataset_query。"
    }))
}

fn project_requested_metrics(
    resource_id: &str,
    request_metric_ids: &[String],
    runtime_metric_defs: &BTreeMap<String, Value>,
    metrics_map: &BTreeMap<String, MetricContract>,
) -> Vec<MetricContract> {
    request_metric_ids
        .iter()
        .filter_map(|metric_id| {
            let resolved =
                resolve_runtime_metric_def_key(resource_id, metric_id, runtime_metric_defs)?;
            let mut metric = metrics_map.get(&resolved)?.clone();
            metric.id = metric_id.clone();
            Some(metric)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::project_requested_metrics;
    use mei_lang_kernel::{MetricContract, MetricShape};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn project_requested_metrics_keeps_requested_id_after_canonical_resolution() {
        let runtime_metric_defs = BTreeMap::from([(
            "capsule/overview.mei::sales_total".to_string(),
            json!({"id": "capsule/overview.mei::sales_total"}),
        )]);
        let metrics_map = BTreeMap::from([(
            "capsule/overview.mei::sales_total".to_string(),
            MetricContract {
                id: "capsule/overview.mei::sales_total".to_string(),
                label: Some("Sales Total".to_string()),
                unit: None,
                purpose: None,
                shape: MetricShape::Scalar,
                schema: Vec::new(),
                dataset: None,
                transforms: Vec::new(),
                value: json!(42),
            },
        )]);
        let projected = project_requested_metrics(
            "__world_metrics__::capsule/overview.mei::metrics",
            &["sales_total".to_string()],
            &runtime_metric_defs,
            &metrics_map,
        );
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].id, "sales_total");
        assert_eq!(projected[0].value, json!(42));
    }
}
