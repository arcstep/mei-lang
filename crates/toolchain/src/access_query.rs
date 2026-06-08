use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{locate_dataset_resource, DatasetView};
use serde_json::{json, Value};

use crate::analysis_contract::{
    build_dataset_analysis_contracts_preview_for_access,
    build_metric_analysis_contract_attachments, contract_attachment_stats,
    contract_hint_when_empty, contract_hint_when_preview_empty, contract_preview_stats,
};
use mei_lang_datasets::{
    evaluate_runtime_metrics_from_plan, metric_ids_visible_for_dataset, normalize_query_filters,
    normalize_query_search, plan_access_metric_eval_for_ids, query_dataset_rows,
    query_state_from_request, RuntimeMetricEvalMode, DatasetQueryOptions,
};
use crate::observation::{CompileObservation, EvalObservation, ExposureManifest};
use crate::types::WorldScope;
use crate::world::{load_world_runtime_bundle, normalize_path};

pub const RESOURCE_QUERY_SCHEMA_VERSION: &str = "resource-query-v5";

pub const DATASET_QUERY_DEFAULT_LIMIT: usize = 10;
pub const DATASET_QUERY_MAX_LIMIT: usize = 50;
pub const DATASET_QUERY_DEFAULT_COLUMNS: usize = 10;
pub const DATASET_QUERY_MAX_COLUMNS: usize = 10;
pub const DATASET_QUERY_MAX_CELL_CHARS: usize = 50;
const DATASET_QUERY_TOTAL_CHAR_BUDGET: usize = 12_000;

fn normalize_limit(limit: Option<usize>, default: usize, max: usize) -> usize {
    limit.unwrap_or(default).clamp(1, max)
}

fn normalize_dataset_limit(limit: Option<usize>) -> usize {
    normalize_limit(limit, DATASET_QUERY_DEFAULT_LIMIT, DATASET_QUERY_MAX_LIMIT)
}

fn dataset_available_columns(dataset: &DatasetView) -> Vec<String> {
    if !dataset.columns.is_empty() {
        return dataset.columns.clone();
    }
    dataset.schema.iter().map(|c| c.name.clone()).collect()
}

pub fn normalize_dataset_columns(
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

fn bounded_cell_value(value: &Value, truncated_cells: &mut usize) -> Value {
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

fn project_dataset_row(
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

fn json_serialized_len(value: &Value) -> usize {
    serde_json::to_string(value).map(|s| s.len()).unwrap_or(0)
}

fn shrink_json_for_llm(value: &Value, max_total: usize) -> Value {
    let len = json_serialized_len(value);
    if len <= max_total {
        return value.clone();
    }
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, item) in map.iter().take(48) {
                let entry_len = json_serialized_len(item);
                if entry_len > 2_000 {
                    out.insert(
                        key.clone(),
                        json!({
                            "_omitted": true,
                            "approx_chars": entry_len,
                        }),
                    );
                } else {
                    out.insert(key.clone(), item.clone());
                }
            }
            out.insert(
                "_truncated".to_string(),
                json!({
                    "reason": "payload too large for tool output",
                    "approx_original_chars": len,
                }),
            );
            Value::Object(out)
        }
        Value::Array(items) => json!({
            "type": "array",
            "len": items.len(),
            "head": items.iter().take(5).cloned().collect::<Vec<_>>(),
        }),
        Value::String(text) => {
            let cap = 1_000usize;
            if text.len() <= cap {
                Value::String(text.clone())
            } else {
                Value::String(format!("{}…", text.chars().take(cap).collect::<String>()))
            }
        }
        other => other.clone(),
    }
}

fn summarize_filters_decl(filters: &Value) -> Value {
    let len = json_serialized_len(filters);
    if len <= 1_200 {
        return filters.clone();
    }
    match filters {
        Value::Object(map) => json!({
            "object_key_count": map.len(),
            "keys": map.keys().take(40).cloned().collect::<Vec<_>>(),
            "approx_chars": len,
            "note": "filters object truncated to keys only.",
        }),
        _ => json!({
            "approx_chars": len,
            "note": "filters omitted (too large).",
        }),
    }
}

pub fn query_world_dataset(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
    columns: Option<&[String]>,
    limit: Option<usize>,
) -> Result<Value> {
    let request_started = Instant::now();
    let dataset_id = id.trim();
    if dataset_id.is_empty() {
        return Err(anyhow!("query parameter `id` is required"));
    }
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let load_bundle_ms = request_started.elapsed().as_millis() as u64;
    let locate_started = Instant::now();
    let loaded = locate_dataset_resource(&bundle.compiled, dataset_id)
        .map_err(|error| anyhow!("{error}"))?;
    let locate_dataset_ms = locate_started.elapsed().as_millis() as u64;
    let dataset = loaded
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_id}` is not a dataset"))?;

    let row_limit = normalize_dataset_limit(limit);
    let selected_columns = normalize_dataset_columns(dataset, columns);
    let app_root = mei_lang_kernel::resolve_app_root(source_root, app_id);
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
    let query_rows_started = Instant::now();
    let query_result = query_dataset_rows(&app_root, dataset, query_options)?;
    let query_rows_ms = query_rows_started.elapsed().as_millis() as u64;

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
    let build_contract_preview_started = Instant::now();
    let analysis_contracts_preview = build_dataset_analysis_contracts_preview_for_access(
        &bundle.compiled,
        dataset,
        &loaded.id,
        &metric_ids,
        world_resource.and_then(|item| item.metrics.as_ref()),
    );
    let build_contract_preview_ms = build_contract_preview_started.elapsed().as_millis() as u64;
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
    let compile_observation = CompileObservation::for_world_bundle(
        app_id,
        &bundle.contract.scene.id,
        Some(bundle.active_target_file.as_str()),
        load_bundle_ms,
    );
    let mut eval_observation = EvalObservation::new(false);
    eval_observation.insert_counter("rows_returned", query_result.rows.len() as u64);
    eval_observation.insert_counter("columns_returned", selected_columns.len() as u64);
    eval_observation.insert_counter("cells_truncated", truncated_cells as u64);
    let exposure_manifest = ExposureManifest::for_scene_scope(
        app_id,
        &bundle.contract.scene.id,
        Some(bundle.active_target_file.as_str()),
        Some(RESOURCE_QUERY_SCHEMA_VERSION),
    );
    let mut perf = BTreeMap::new();
    compile_observation.write_perf(&mut perf);
    perf.insert("load_world_bundle_ms".to_string(), load_bundle_ms);
    perf.insert("locate_dataset_ms".to_string(), locate_dataset_ms);
    perf.insert("dataset_query_rows_ms".to_string(), query_rows_ms);
    perf.insert(
        "build_analysis_contract_preview_ms".to_string(),
        build_contract_preview_ms,
    );
    eval_observation.write_perf(&mut perf);
    for (key, value) in contract_preview_stats(&analysis_contracts_preview, metric_ids.len()) {
        perf.insert(key, value);
    }
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
        "observation": {
            "compile": &compile_observation,
            "eval": &eval_observation,
            "exposure": &exposure_manifest,
        },
        "perf": perf,
        "usage_hint": "若需更多数据，请在 dataset_query 中追加 filters/search/columns/limit；默认仅返回前10行与前10列的有界样例。带 explain 的指标请同时查看 dataset.analysis_contracts_preview，与宿主 UI 的 analysis_contract 同轨。",
    });
    let shrink_started = Instant::now();
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
    let shrink_ms = shrink_started.elapsed().as_millis() as u64;
    if let Some(map) = payload.get_mut("perf").and_then(Value::as_object_mut) {
        map.insert(
            "payload_chars_before_budget".to_string(),
            json!(before as u64),
        );
        map.insert(
            "payload_chars_after_budget".to_string(),
            json!(after as u64),
        );
        map.insert("payload_shrink_ms".to_string(), json!(shrink_ms));
        map.insert(
            "payload_budget_truncated".to_string(),
            json!(u64::from(before > DATASET_QUERY_TOTAL_CHAR_BUDGET)),
        );
        map.insert(
            "total_ms".to_string(),
            json!(request_started.elapsed().as_millis() as u64),
        );
    }
    Ok(payload)
}

pub fn query_world_dataset_metrics(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
    metric_ids: &[String],
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
) -> Result<Value> {
    let request_started = Instant::now();
    let dataset_id = id.trim();
    if dataset_id.is_empty() {
        return Err(anyhow!("query parameter `id` is required"));
    }
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let load_bundle_ms = request_started.elapsed().as_millis() as u64;
    let plan_eval_started = Instant::now();
    let request_all_metrics = metric_ids.is_empty();
    let eval_plan = plan_access_metric_eval_for_ids(&bundle.compiled, dataset_id, metric_ids)?;
    let plan_eval_ms = plan_eval_started.elapsed().as_millis() as u64;
    let primary_dataset = eval_plan.primary_dataset;

    let app_root = mei_lang_kernel::resolve_app_root(source_root, app_id);
    let normalized_search = normalize_query_search(search);
    let normalized_filters = normalize_query_filters(filters);
    let effective_query_state =
        query_state_from_request(&normalized_filters, normalized_search.as_deref(), None);
    let eval_outcome = evaluate_runtime_metrics_from_plan(
        &bundle.compiled,
        &app_root,
        &eval_plan,
        &bundle.contract.scene.id,
        Some(bundle.active_target_file.as_str()),
        &effective_query_state,
        &[],
        RuntimeMetricEvalMode::WithoutDag,
        request_all_metrics,
    )?;
    let metrics = eval_outcome.metrics;
    let requested_metric_ids = eval_outcome.request_metric_ids.clone();
    let contract_attachment_started = Instant::now();
    let analysis_contracts = build_metric_analysis_contract_attachments(
        &bundle.compiled,
        primary_dataset,
        &eval_plan.primary.id,
        &requested_metric_ids,
    );
    let contract_attachment_ms = contract_attachment_started.elapsed().as_millis() as u64;
    let contract_hint = contract_hint_when_empty(&analysis_contracts);
    let compile_observation = CompileObservation::for_world_bundle(
        app_id,
        &bundle.contract.scene.id,
        Some(bundle.active_target_file.as_str()),
        load_bundle_ms,
    );
    let mut eval_observation = EvalObservation::new(false);
    eval_observation.insert_counter("metrics_returned", metrics.len() as u64);
    eval_observation.insert_counter("total_rows", eval_outcome.total_rows as u64);
    let exposure_manifest = ExposureManifest::for_scene_scope(
        app_id,
        &bundle.contract.scene.id,
        Some(bundle.active_target_file.as_str()),
        Some(RESOURCE_QUERY_SCHEMA_VERSION),
    );
    let mut perf = BTreeMap::new();
    compile_observation.write_perf(&mut perf);
    perf.insert("load_world_bundle_ms".to_string(), load_bundle_ms);
    perf.insert("plan_access_metric_eval_ms".to_string(), plan_eval_ms);
    perf.insert("dataset_query_rows_ms".to_string(), eval_outcome.query_ms);
    perf.insert("hydrate_datasets_ms".to_string(), eval_outcome.hydrate_ms);
    perf.insert("build_eval_scope_ms".to_string(), eval_outcome.eval_scope_ms);
    perf.insert("metric_eval_ms".to_string(), eval_outcome.metric_eval_ms);
    perf.insert(
        "build_analysis_contract_attachments_ms".to_string(),
        contract_attachment_ms,
    );
    eval_observation.write_perf(&mut perf);
    for (key, value) in contract_attachment_stats(&analysis_contracts, requested_metric_ids.len()) {
        perf.insert(key, value);
    }
    perf.insert(
        "total_ms".to_string(),
        request_started.elapsed().as_millis() as u64,
    );

    Ok(json!({
        "app_id": app_id,
        "scene_id": bundle.contract.scene.id,
        "dataset_id": dataset_id,
        "total_rows": eval_outcome.total_rows,
        "metrics": metrics,
        "analysis_contracts": analysis_contracts,
        "contract_hint": contract_hint,
        "observation": {
            "compile": &compile_observation,
            "eval": &eval_observation,
            "exposure": &exposure_manifest,
        },
        "perf": perf,
        "usage_hint": "指标问答优先使用 dataset_metric；analysis_contracts 与宿主 UI popup/route 同轨。无 contract 时勿编造 explain 字段。若要查看明细行，再改用 dataset_query。"
    }))
}

#[cfg(test)]
mod tests {
    use super::normalize_dataset_columns;
    use mei_lang_datasets::project_requested_metrics;
    use mei_lang_kernel::{DatasetView, MetricContract, MetricShape, SourceDecl};
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
                value_format: None,
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

    #[test]
    fn normalize_dataset_columns_caps_default_selection() {
        let dataset = DatasetView {
            id: "ds".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: (0..20).map(|i| format!("c{i}")).collect(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:ds".to_string(),
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
        let cols = normalize_dataset_columns(&dataset, None);
        assert_eq!(cols.len(), 10);
    }
}
