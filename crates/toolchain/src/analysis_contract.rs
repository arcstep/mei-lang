use std::collections::BTreeMap;

use mei_lang_kernel::{resolve_runtime_metric_def_key, CompiledApp, DatasetView};
use serde_json::{json, Map, Value};

use mei_lang_datasets::{locate_runtime_metric_resource, metric_ids_visible_for_dataset};

const MAX_NOTE_CHARS: usize = 240;
const MAX_TABS: usize = 12;
const MAX_BLOCK_KINDS: usize = 16;
const MAX_CONTRACTS_IN_PREVIEW: usize = 48;

fn json_serialized_len(value: &Value) -> usize {
    serde_json::to_string(value).map(|s| s.len()).unwrap_or(0)
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = input.chars().take(max_chars).collect::<String>();
    out.push('…');
    out
}

pub fn lookup_runtime_analysis_contract(
    dataset: &DatasetView,
    resource_id: &str,
    metric_id: &str,
) -> Option<Value> {
    let metric_id = metric_id.trim();
    if metric_id.is_empty() {
        return None;
    }
    let canonical_id =
        resolve_runtime_metric_def_key(resource_id, metric_id, &dataset.runtime_metric_defs)
            .unwrap_or_else(|| metric_id.to_string());
    dataset
        .runtime_analysis_contracts
        .get(&canonical_id)
        .cloned()
}

pub fn summarize_analysis_contract_for_llm(contract: &Value) -> Value {
    let Some(map) = contract.as_object() else {
        return json!({ "present": false });
    };
    let title = map
        .get("title")
        .or_else(|| map.get("focus_node_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let note = map
        .get("note")
        .and_then(Value::as_str)
        .map(|text| truncate_chars(text, MAX_NOTE_CHARS));
    let tabs = map
        .get("tabs")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .take(MAX_TABS)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let block_kinds = map
        .get("blocks")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|block| {
                    block
                        .get("kind")
                        .or_else(|| block.get("support_role"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
                .take(MAX_BLOCK_KINDS)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let table_metric_id = map
        .get("table_metric_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            map.get("tab_metrics")
                .and_then(|value| value.get("detail"))
                .and_then(|detail| detail.get("metric_id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let explain_metric_count = map
        .get("explain_metrics")
        .and_then(Value::as_array)
        .map(|items| items.len())
        .unwrap_or(0);
    json!({
        "present": true,
        "focus_node_id": map.get("focus_node_id").and_then(Value::as_str),
        "root_metric_id": map.get("root_metric_id").and_then(Value::as_str),
        "root_dataset_id": map.get("root_dataset_id").and_then(Value::as_str),
        "title": title,
        "note": note,
        "tabs": tabs,
        "block_kinds": block_kinds,
        "table_metric_id": table_metric_id,
        "explain_metric_count": explain_metric_count,
        "approx_contract_chars": json_serialized_len(contract),
        "usage_hint": "This summary mirrors host UI analysis_contract; use dataset_metric for values and this summary for explain/popup semantics."
    })
}

pub fn build_dataset_analysis_contracts_preview_for_access(
    compiled: &CompiledApp,
    primary_dataset: &DatasetView,
    primary_resource_id: &str,
    metric_ids: &[String],
    world_metrics_decl: Option<&BTreeMap<String, serde_json::Value>>,
) -> Value {
    let mut preview = Map::new();
    let ids = if metric_ids.is_empty() {
        metric_ids_visible_for_dataset(compiled, primary_dataset, world_metrics_decl)
    } else {
        metric_ids.to_vec()
    };
    for metric_id in ids.iter().take(MAX_CONTRACTS_IN_PREVIEW) {
        if let Some(contract) =
            lookup_runtime_analysis_contract(primary_dataset, primary_resource_id, metric_id)
        {
            preview.insert(
                metric_id.clone(),
                summarize_analysis_contract_for_llm(&contract),
            );
            continue;
        }
        let Ok((owner, _resolved)) =
            locate_runtime_metric_resource(compiled, &primary_dataset.id, metric_id)
        else {
            continue;
        };
        let Some(owner_dataset) = owner.dataset.as_ref() else {
            continue;
        };
        let Some(contract) = lookup_runtime_analysis_contract(owner_dataset, &owner.id, metric_id)
        else {
            continue;
        };
        preview.insert(
            metric_id.clone(),
            summarize_analysis_contract_for_llm(&contract),
        );
    }
    Value::Object(preview)
}

pub fn build_metric_analysis_contract_attachments(
    compiled: &CompiledApp,
    primary_dataset: &DatasetView,
    primary_resource_id: &str,
    metric_ids: &[String],
) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for metric_id in metric_ids {
        if let Some(contract) =
            lookup_runtime_analysis_contract(primary_dataset, primary_resource_id, metric_id)
        {
            out.insert(
                metric_id.clone(),
                summarize_analysis_contract_for_llm(&contract),
            );
            continue;
        }
        let Ok((owner, _resolved)) =
            locate_runtime_metric_resource(compiled, &primary_dataset.id, metric_id)
        else {
            continue;
        };
        let Some(owner_dataset) = owner.dataset.as_ref() else {
            continue;
        };
        let Some(contract) = lookup_runtime_analysis_contract(owner_dataset, &owner.id, metric_id)
        else {
            continue;
        };
        out.insert(
            metric_id.clone(),
            summarize_analysis_contract_for_llm(&contract),
        );
    }
    out
}

pub fn contract_hint_when_empty(contracts: &BTreeMap<String, Value>) -> Option<String> {
    if contracts.is_empty() {
        Some("no_runtime_analysis_contracts_for_requested_metrics".to_string())
    } else {
        None
    }
}

pub fn contract_hint_when_preview_empty(preview: &Value) -> Option<String> {
    match preview.as_object() {
        Some(map) if map.is_empty() => {
            Some("no_runtime_analysis_contracts_for_dataset_metrics".to_string())
        }
        _ => None,
    }
}

pub fn contract_attachment_stats(
    contracts: &BTreeMap<String, Value>,
    requested_metric_count: usize,
) -> BTreeMap<String, u64> {
    let present_count = contracts
        .values()
        .filter(|entry| {
            entry
                .get("present")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    let mut stats = BTreeMap::new();
    stats.insert(
        "analysis_contract_requested_metric_count".to_string(),
        requested_metric_count as u64,
    );
    stats.insert(
        "analysis_contract_attachment_count".to_string(),
        contracts.len() as u64,
    );
    stats.insert(
        "analysis_contract_present_count".to_string(),
        present_count as u64,
    );
    stats.insert(
        "analysis_contract_missing_count".to_string(),
        requested_metric_count.saturating_sub(present_count) as u64,
    );
    stats
}

pub fn contract_preview_stats(
    preview: &Value,
    visible_metric_count: usize,
) -> BTreeMap<String, u64> {
    let preview_count = preview.as_object().map(|map| map.len()).unwrap_or(0);
    let present_count = preview
        .as_object()
        .map(|map| {
            map.values()
                .filter(|entry| {
                    entry
                        .get("present")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0);
    let mut stats = BTreeMap::new();
    stats.insert(
        "analysis_contract_visible_metric_count".to_string(),
        visible_metric_count as u64,
    );
    stats.insert(
        "analysis_contract_preview_count".to_string(),
        preview_count as u64,
    );
    stats.insert(
        "analysis_contract_preview_present_count".to_string(),
        present_count as u64,
    );
    stats.insert(
        "analysis_contract_preview_missing_count".to_string(),
        visible_metric_count.saturating_sub(present_count) as u64,
    );
    stats
}
