//! Bounded analysis_contract summaries for access-side AI consumers.
//!
//! Host UI consumers read full `analysis_contract` from preview runtime refs; access
//! tools should see the same derived contracts in a size-limited form instead of
//! inventing parallel drilldown semantics.

use std::collections::BTreeMap;

use mei_lang_kernel::{resolve_runtime_metric_def_key, CompiledApp, DatasetView};
use serde_json::{json, Map, Value};

use super::json_shrink::json_serialized_len;
use crate::http::datasets::locate_runtime_metric_resource;
use crate::http::scene_api::types::WorldRuntimeBundle;

const MAX_NOTE_CHARS: usize = 240;
const MAX_TABS: usize = 12;
const MAX_BLOCK_KINDS: usize = 16;
const MAX_CONTRACTS_IN_PREVIEW: usize = 48;

pub(crate) fn lookup_runtime_analysis_contract(
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

pub(crate) fn summarize_analysis_contract_for_llm(contract: &Value) -> Value {
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

pub(crate) fn build_dataset_analysis_contracts_preview_for_access(
    compiled: &CompiledApp,
    primary_dataset: &DatasetView,
    primary_resource_id: &str,
    metric_ids: &[String],
    world_metrics_decl: Option<&BTreeMap<String, serde_json::Value>>,
) -> Value {
    use crate::http::datasets::metric_ids_visible_for_dataset;

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

#[allow(dead_code)] // exercised by unit tests in this module
pub(crate) fn build_dataset_analysis_contracts_preview(
    dataset: &DatasetView,
    resource_id: &str,
    metric_ids: &[String],
) -> Value {
    let mut preview = serde_json::Map::new();
    let ids = if metric_ids.is_empty() {
        dataset
            .runtime_analysis_contracts
            .keys()
            .take(MAX_CONTRACTS_IN_PREVIEW)
            .cloned()
            .collect::<Vec<_>>()
    } else {
        metric_ids.to_vec()
    };
    for metric_id in ids.iter().take(MAX_CONTRACTS_IN_PREVIEW) {
        let Some(contract) = lookup_runtime_analysis_contract(dataset, resource_id, metric_id)
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

pub(crate) fn contract_hint_when_empty(contracts: &BTreeMap<String, Value>) -> Option<String> {
    if contracts.is_empty() {
        Some("no_runtime_analysis_contracts_for_requested_metrics".to_string())
    } else {
        None
    }
}

pub(crate) fn contract_hint_when_preview_empty(preview: &Value) -> Option<String> {
    match preview.as_object() {
        Some(map) if map.is_empty() => {
            Some("no_runtime_analysis_contracts_for_dataset_metrics".to_string())
        }
        _ => None,
    }
}

pub(crate) fn contract_attachment_stats(
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

pub(crate) fn contract_preview_stats(
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

pub(crate) fn build_metric_analysis_contract_attachments(
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

pub(crate) fn build_analysis_contract_catalog_lines(bundle: &WorldRuntimeBundle) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(String::new());
    lines.push("[World — analysis_contract summaries (bounded)]".to_string());
    lines.push(
        "Derived explain/popup contracts for runtime metrics. Prefer dataset_metric for values; use contract summaries for explain tabs and drilldown semantics (same source as host UI)."
            .to_string(),
    );

    let mut emitted = 0usize;
    for resource in &bundle.compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if dataset.runtime_analysis_contracts.is_empty() {
            continue;
        }
        lines.push(format!(
            "  dataset resource id={} runtime_contract_count={}",
            resource.id,
            dataset.runtime_analysis_contracts.len()
        ));
        for (metric_id, contract) in dataset.runtime_analysis_contracts.iter().take(24) {
            if emitted >= MAX_CONTRACTS_IN_PREVIEW {
                break;
            }
            let summary = summarize_analysis_contract_for_llm(contract);
            let title = summary
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(metric_id.as_str());
            let tabs = summary
                .get("tabs")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default();
            let note = summary.get("note").and_then(Value::as_str).unwrap_or("-");
            lines.push(format!(
                "    - metric={} title={} tabs=[{}] note={}",
                metric_id, title, tabs, note
            ));
            emitted += 1;
        }
        if emitted >= MAX_CONTRACTS_IN_PREVIEW {
            break;
        }
    }
    if emitted == 0 {
        lines.push("  (no runtime analysis_contract materialized in current scope)".to_string());
    }
    lines
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = input.chars().take(max_chars).collect::<String>();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::{
        build_dataset_analysis_contracts_preview, contract_attachment_stats,
        contract_hint_when_empty, contract_hint_when_preview_empty, contract_preview_stats,
        summarize_analysis_contract_for_llm,
    };
    use mei_lang_kernel::{DatasetView, SourceDecl};
    use serde_json::{json, Value};
    use std::collections::BTreeMap;

    #[test]
    fn contract_hint_when_empty_or_preview_empty() {
        assert_eq!(
            contract_hint_when_empty(&BTreeMap::new()).as_deref(),
            Some("no_runtime_analysis_contracts_for_requested_metrics")
        );
        assert!(contract_hint_when_preview_empty(&json!({})).is_some());
        assert!(contract_hint_when_preview_empty(&json!({"m": {"present": true}})).is_none());
    }

    #[test]
    fn summarize_analysis_contract_for_llm_keeps_tabs_and_note() {
        let contract = json!({
            "focus_node_id": "sales_total",
            "title": "销售总额",
            "note": "按销售单去重统计。",
            "tabs": ["definition", "detail", "trend"],
            "blocks": [
                {"kind": "definition"},
                {"kind": "detail"},
            ],
            "tab_metrics": {
                "detail": {"metric_id": "sales_total_table"}
            }
        });
        let summary = summarize_analysis_contract_for_llm(&contract);
        assert_eq!(summary.get("present").and_then(Value::as_bool), Some(true));
        assert_eq!(
            summary
                .get("tabs")
                .and_then(Value::as_array)
                .map(|v| v.len()),
            Some(3)
        );
        assert_eq!(
            summary.get("table_metric_id").and_then(Value::as_str),
            Some("sales_total_table")
        );
        assert_eq!(
            summary.get("note").and_then(Value::as_str),
            Some("按销售单去重统计。")
        );
    }

    #[test]
    fn build_dataset_analysis_contracts_preview_resolves_canonical_metric_key() {
        let dataset = DatasetView {
            id: "orders".to_string(),
            title: None,
            purpose: None,
            schema: Vec::new(),
            stage_schema: Vec::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            source: SourceDecl {
                kind: "csv".to_string(),
                path: "data/orders.csv".to_string(),
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
            runtime_metric_defs: BTreeMap::from([(
                "orders_detail_table".to_string(),
                json!({"id": "orders_detail_table"}),
            )]),
            runtime_analysis_graph: Default::default(),
            runtime_analysis_contracts: BTreeMap::from([(
                "orders_detail_table".to_string(),
                json!({
                    "focus_node_id": "orders_detail_table",
                    "title": "订单明细",
                    "tabs": ["detail"],
                }),
            )]),
        };
        let preview = build_dataset_analysis_contracts_preview(
            &dataset,
            "orders",
            &["orders_detail_table".to_string()],
        );
        let entry = preview
            .get("orders_detail_table")
            .and_then(Value::as_object)
            .expect("preview entry");
        assert_eq!(entry.get("present").and_then(Value::as_bool), Some(true));
        assert_eq!(entry.get("title").and_then(Value::as_str), Some("订单明细"));
    }

    #[test]
    fn contract_stats_reflect_present_and_missing() {
        let attachments = BTreeMap::from([
            ("m1".to_string(), json!({"present": true})),
            ("m2".to_string(), json!({"present": false})),
        ]);
        let attachment_stats = contract_attachment_stats(&attachments, 3);
        assert_eq!(
            attachment_stats.get("analysis_contract_attachment_count"),
            Some(&2)
        );
        assert_eq!(
            attachment_stats.get("analysis_contract_present_count"),
            Some(&1)
        );
        assert_eq!(
            attachment_stats.get("analysis_contract_missing_count"),
            Some(&2)
        );

        let preview_stats = contract_preview_stats(
            &json!({
                "m1": {"present": true},
                "m2": {"present": true},
                "m3": {"present": false}
            }),
            4,
        );
        assert_eq!(
            preview_stats.get("analysis_contract_preview_count"),
            Some(&3)
        );
        assert_eq!(
            preview_stats.get("analysis_contract_preview_present_count"),
            Some(&2)
        );
        assert_eq!(
            preview_stats.get("analysis_contract_preview_missing_count"),
            Some(&2)
        );
    }
}
