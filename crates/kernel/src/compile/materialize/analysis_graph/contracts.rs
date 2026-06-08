use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::model::AnalysisGraph;

use super::{expand::*, explain::*, graph_build::*, util::*};

pub(super) fn build_analysis_contracts_from_expanded(
    expanded_defs: &BTreeMap<String, Value>,
    graph: &AnalysisGraph,
    root_dataset_id: &str,
) -> BTreeMap<String, Value> {
    let mut contracts = BTreeMap::new();
    for (metric_id, raw) in expanded_defs {
        let has_explain_scope = raw
            .as_object()
            .and_then(|value| value.get("explain"))
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty());
        if !has_explain_scope {
            continue;
        }
        let contract = build_metric_contract(metric_id, raw, graph, root_dataset_id);
        contracts.insert(metric_id.clone(), Value::Object(contract));
    }
    contracts
}

pub(super) fn build_metric_contract(
    metric_id: &str,
    raw: &Value,
    graph: &AnalysisGraph,
    root_dataset_id: &str,
) -> Map<String, Value> {
    let map = raw.as_object();
    let mut contract = Map::new();
    contract.insert(
        "focus_node_id".to_string(),
        Value::String(metric_id.to_string()),
    );
    contract.insert(
        "root_metric_id".to_string(),
        Value::String(metric_id.to_string()),
    );
    if !root_dataset_id.trim().is_empty() {
        contract.insert(
            "root_dataset_id".to_string(),
            Value::String(root_dataset_id.to_string()),
        );
    }
    if let Some(label) = map.and_then(|value| first_non_empty_string(value, &["label", "title"])) {
        contract.insert("title".to_string(), Value::String(label));
    }
    if let Some(map) = map {
        apply_metric_narrative(map, &mut contract);
    }
    let metric_has_narrative = contract.contains_key("note")
        || contract.contains_key("basis_refs")
        || contract.contains_key("recommended_dimensions");
    let mut tabs = Vec::<Value>::new();
    let mut tab_metrics = Map::new();
    let mut nodes = Vec::<Value>::new();
    let mut objects = Map::new();
    let mut blocks = Vec::<Value>::new();
    let mut seen_tabs = BTreeSet::new();
    let mut seen_nodes = BTreeSet::new();
    if let Some(items) = map
        .and_then(|value| value.get("explain"))
        .and_then(Value::as_array)
    {
        for (item_index, item) in items.iter().enumerate() {
            let Some(item_map) = item.as_object() else {
                continue;
            };
            if item_map.get("__kind").and_then(Value::as_str) == Some("data_product") {
                let Some(node_id) = first_non_empty_string(
                    item_map,
                    &["analysis_scoped_id", "analysis_node_id", "id", "key"],
                ) else {
                    continue;
                };
                if seen_nodes.insert(node_id.clone()) {
                    let node_value = analysis_node_value(
                        graph.nodes.get(&node_id),
                        &node_id,
                        item_map.get("support_role").and_then(Value::as_str),
                    );
                    objects.insert(node_id.clone(), node_value.clone());
                    nodes.push(node_value);
                }
                if let Some(block) =
                    explain_block_for_data_product_dataframe(item_map, graph, root_dataset_id)
                {
                    let support_role = block
                        .get("support_role")
                        .and_then(Value::as_str)
                        .unwrap_or("detail")
                        .to_string();
                    if seen_tabs.insert(support_role.clone()) {
                        tabs.push(Value::String(support_role.clone()));
                    }
                    if let Some(node_id) = block.get("node_id").and_then(Value::as_str) {
                        if seen_nodes.insert(node_id.to_string()) {
                            let node_value = analysis_node_value(
                                graph.nodes.get(node_id),
                                node_id,
                                Some(support_role.as_str()),
                            );
                            objects.insert(node_id.to_string(), node_value.clone());
                            nodes.push(node_value);
                        }
                    }
                    if let Some(tab_metric) = tab_metric_value(&block) {
                        tab_metrics.insert(support_role.clone(), Value::Object(tab_metric));
                    }
                    blocks.push(Value::Object(block));
                }
                continue;
            }
            let support_role = support_role_for_item(item_map);
            if support_role == "note" && contract.contains_key("note") {
                continue;
            }
            if support_role == "definition" {
                merge_definition_narrative_fallback(item_map, &mut contract);
                if is_empty_legacy_definition_item(item_map) || metric_has_narrative {
                    continue;
                }
            }
            let block = explain_block_value(
                metric_id,
                raw,
                item_map,
                items,
                item_index,
                graph,
                root_dataset_id,
            );
            if support_role == "note" {
                if contract.get("note").is_none() {
                    if let Some(note) = block
                        .get("note")
                        .or_else(|| block.get("content"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                    {
                        contract.insert("note".to_string(), Value::String(note));
                    }
                }
                blocks.push(Value::Object(block));
                continue;
            }
            if seen_tabs.insert(support_role.clone()) {
                tabs.push(Value::String(support_role.clone()));
            }
            if let Some(node_id) = block
                .get("node_id")
                .and_then(Value::as_str)
                .map(str::to_string)
            {
                if seen_nodes.insert(node_id.clone()) {
                    let node_value = analysis_node_value(
                        graph.nodes.get(&node_id),
                        &node_id,
                        Some(support_role.as_str()),
                    );
                    objects.insert(node_id.clone(), node_value.clone());
                    nodes.push(node_value);
                }
            }
            if let Some(tab_metric) = tab_metric_value(&block) {
                tab_metrics.insert(support_role.clone(), Value::Object(tab_metric));
            }
            blocks.push(Value::Object(block));
        }
    }
    if !tabs.is_empty() {
        contract.insert("tabs".to_string(), Value::Array(tabs));
    }
    if !nodes.is_empty() {
        contract.insert("nodes".to_string(), Value::Array(nodes));
    }
    if !blocks.is_empty() {
        contract.insert("blocks".to_string(), Value::Array(blocks));
    }
    if !objects.is_empty() {
        contract.insert("objects".to_string(), Value::Object(objects));
    }
    if !tab_metrics.is_empty() {
        contract.insert("tab_metrics".to_string(), Value::Object(tab_metrics));
    }
    contract
}
