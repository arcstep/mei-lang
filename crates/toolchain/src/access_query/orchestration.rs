use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};
use mei_lang_kernel::locate_dataset_resource;
use mei_lang_datasets::{
    evaluate_runtime_metrics_from_plan, metric_ids_visible_for_dataset, normalize_query_filters,
    normalize_query_search, plan_access_metric_eval_for_ids, query_dataset_rows,
    query_state_from_request, RuntimeMetricEvalMode, DatasetQueryOptions,
};
use serde_json::{json, Value};

use crate::analysis_contract::{
    build_dataset_analysis_contracts_preview_for_access,
    build_metric_analysis_contract_attachments, contract_attachment_stats,
    contract_hint_when_empty, contract_hint_when_preview_empty, contract_preview_stats,
};
use crate::observation::{CompileObservation, EvalObservation, ExposureManifest};
use crate::types::WorldScope;
use crate::world::{load_world_runtime_bundle, normalize_path};

use super::dataset_binding::{normalize_dataset_limit, DATASET_QUERY_MAX_COLUMNS};
use super::normalize_dataset_columns;
use super::serialization::{
    build_schema_preview, json_serialized_len, project_dataset_row, shrink_json_for_llm,
    summarize_filters_decl, DATASET_QUERY_MAX_CELL_CHARS, DATASET_QUERY_TOTAL_CHAR_BUDGET,
};
use super::RESOURCE_QUERY_SCHEMA_VERSION;

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
