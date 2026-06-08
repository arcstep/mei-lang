use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::model::{AnalysisEdge, AnalysisGraph, AnalysisNode, SemanticNodeKind};

use super::{explain::*, expand::*, util::*};

pub(super) fn build_analysis_graph_from_expanded(
    expanded_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> AnalysisGraph {
    let mut graph = AnalysisGraph::default();
    let mut edge_set = BTreeSet::<(String, String, String)>::new();
    for (metric_id, raw) in expanded_defs {
        graph.nodes.insert(
            metric_id.clone(),
            metric_node_from_raw(metric_id, raw, root_dataset_id),
        );
    }
    for (metric_id, raw) in expanded_defs {
        let Some(map) = raw.as_object() else {
            continue;
        };
        if let Some(dataset_id) = first_non_empty_string(map, &["dataset", "dataset_id"]) {
            let tabular_node_id = tabular_source_node_id(&dataset_id);
            graph
                .nodes
                .entry(tabular_node_id.clone())
                .or_insert_with(|| tabular_node_from_dataset_id(&tabular_node_id, &dataset_id));
            push_edge(&mut edge_set, metric_id, &tabular_node_id, "lineage");
        }
        for dataset_id in collect_metric_value_lineage_dataset_ids(raw) {
            let tabular_node_id = tabular_source_node_id(&dataset_id);
            graph
                .nodes
                .entry(tabular_node_id.clone())
                .or_insert_with(|| tabular_node_from_dataset_id(&tabular_node_id, &dataset_id));
            push_edge(&mut edge_set, metric_id, &tabular_node_id, "lineage");
        }
        let Some(items) = map.get("explain").and_then(Value::as_array) else {
            continue;
        };
        for (item_index, item) in items.iter().enumerate() {
            let Some(item_map) = item.as_object() else {
                continue;
            };
            if item_map.get("__kind").and_then(Value::as_str) == Some("data_product") {
                if let Some(scoped_id) = first_non_empty_string(
                    item_map,
                    &["analysis_scoped_id", "analysis_node_id", "id", "key"],
                ) {
                    push_edge(&mut edge_set, metric_id, &scoped_id, "scope_metric");
                }
                continue;
            }
            let role = support_role_for_item(item_map);
            let target_metric_id = metric_target_from_item(item_map).or_else(|| {
                if role == "detail" && explain_has_support_role(items, "composition") {
                    infer_inferred_scalar_rowset_metric_id_from_defs(metric_id, expanded_defs)
                } else {
                    infer_explain_scoped_dataframe(items, item_index, role.as_str()).or_else(|| {
                        infer_inferred_scalar_rowset_metric_id_from_defs(metric_id, expanded_defs)
                    })
                }
            });
            let Some(target_metric_id) = target_metric_id else {
                if let Some(dataset_id) = dataset_target_from_item(item_map)
                    .or_else(|| unique_lineage_dataset_id_from_metric_values(raw, role.as_str()))
                {
                    let tabular_node_id = tabular_source_node_id(&dataset_id);
                    graph
                        .nodes
                        .entry(tabular_node_id.clone())
                        .or_insert_with(|| {
                            tabular_node_from_dataset_id(&tabular_node_id, &dataset_id)
                        });
                    push_edge(&mut edge_set, metric_id, &tabular_node_id, "lineage");
                    continue;
                }
                let block_id = narrative_block_id(metric_id, item_map);
                graph.nodes.entry(block_id.clone()).or_insert_with(|| {
                    narrative_node_from_item(&block_id, metric_id, item_map, root_dataset_id)
                });
                push_edge(&mut edge_set, metric_id, &block_id, &role);
                continue;
            };
            graph
                .nodes
                .entry(target_metric_id.clone())
                .or_insert_with(|| AnalysisNode {
                    id: target_metric_id.clone(),
                    canonical_metric_id: Some(target_metric_id.clone()),
                    parent_id: Some(metric_id.clone()),
                    node_kind: "metric".to_string(),
                    semantic_kind: SemanticNodeKind::Metric,
                    support_role: Some(role.clone()),
                    shape: None,
                    label: first_non_empty_string(item_map, &["label"]),
                    lineage_dataset_id: Some(root_dataset_id.to_string()),
                    tabular_source_dataset_id: None,
                    can_explain: false,
                });
            push_edge(&mut edge_set, metric_id, &target_metric_id, &role);
        }
        if let Some(inferred_id) =
            infer_inferred_scalar_rowset_metric_id_from_defs(metric_id, expanded_defs)
        {
            push_edge(&mut edge_set, metric_id, &inferred_id, "scope_metric");
        }
    }
    graph.edges = edge_set
        .into_iter()
        .map(|(from, to, role)| AnalysisEdge {
            semantic_kind: semantic_edge_kind_from_role(&role),
            from,
            to,
            role,
        })
        .collect();
    let validation_errors = graph.validate_invariants();
    debug_assert!(
        validation_errors.is_empty(),
        "analysis graph invariants should hold after build: {:?}",
        validation_errors
    );
    graph
}

pub(super) fn metric_node_from_raw(metric_id: &str, raw: &Value, root_dataset_id: &str) -> AnalysisNode {
    let map = raw.as_object();
    AnalysisNode {
        id: metric_id.to_string(),
        canonical_metric_id: Some(metric_id.to_string()),
        parent_id: map
            .and_then(|value| first_non_empty_string(value, &["analysis_parent_metric_id"])),
        node_kind: "metric".to_string(),
        semantic_kind: SemanticNodeKind::Metric,
        support_role: None,
        shape: map.and_then(detect_metric_shape),
        label: map.and_then(|value| first_non_empty_string(value, &["label", "title"])),
        lineage_dataset_id: map
            .and_then(|value| first_non_empty_string(value, &["dataset", "dataset_id"]))
            .or_else(|| (!root_dataset_id.trim().is_empty()).then(|| root_dataset_id.to_string())),
        tabular_source_dataset_id: None,
        can_explain: map
            .and_then(|value| value.get("explain"))
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty()),
    }
}

pub(super) fn narrative_node_from_item(
    block_id: &str,
    parent_metric_id: &str,
    item_map: &Map<String, Value>,
    root_dataset_id: &str,
) -> AnalysisNode {
    AnalysisNode {
        id: block_id.to_string(),
        canonical_metric_id: None,
        parent_id: Some(parent_metric_id.to_string()),
        node_kind: "narrative".to_string(),
        semantic_kind: SemanticNodeKind::NarrativeSupport,
        support_role: Some(support_role_for_item(item_map)),
        shape: None,
        label: first_non_empty_string(item_map, &["label", "title"]),
        lineage_dataset_id: Some(root_dataset_id.to_string()),
        tabular_source_dataset_id: None,
        can_explain: false,
    }
}

pub(super) fn tabular_node_from_dataset_id(node_id: &str, dataset_id: &str) -> AnalysisNode {
    AnalysisNode {
        id: node_id.to_string(),
        canonical_metric_id: None,
        parent_id: None,
        node_kind: "tabular_source".to_string(),
        semantic_kind: SemanticNodeKind::TabularSource,
        support_role: None,
        shape: Some("dataframe".to_string()),
        label: Some(dataset_id.to_string()),
        lineage_dataset_id: Some(dataset_id.to_string()),
        tabular_source_dataset_id: Some(dataset_id.to_string()),
        can_explain: false,
    }
}

pub(super) fn analysis_node_value(
    node: Option<&AnalysisNode>,
    node_id: &str,
    support_role: Option<&str>,
) -> Value {
    let mut obj = Map::new();
    obj.insert("id".to_string(), Value::String(node_id.to_string()));
    obj.insert(
        "metric_id".to_string(),
        Value::String(
            node.and_then(|value| value.canonical_metric_id.clone())
                .unwrap_or_else(|| node_id.to_string()),
        ),
    );
    obj.insert(
        "node_kind".to_string(),
        Value::String(
            node.map(|value| value.node_kind.clone())
                .unwrap_or_else(|| "metric".to_string()),
        ),
    );
    obj.insert(
        "semantic_kind".to_string(),
        Value::String(
            node.map(|value| semantic_node_kind_name(value.semantic_kind()))
                .unwrap_or("metric")
                .to_string(),
        ),
    );
    if let Some(role) = support_role
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| node.and_then(|value| value.support_role.clone()))
    {
        obj.insert("support_role".to_string(), Value::String(role));
    }
    if let Some(label) = node.and_then(|value| value.label.clone()) {
        obj.insert("label".to_string(), Value::String(label));
    }
    if let Some(parent_id) = node.and_then(|value| value.parent_id.clone()) {
        obj.insert("parent_id".to_string(), Value::String(parent_id));
    }
    if let Some(dataset_id) = node
        .and_then(|value| value.tabular_source_dataset_id.clone())
        .or_else(|| node.and_then(|value| value.lineage_dataset_id.clone()))
    {
        obj.insert("dataset_id".to_string(), Value::String(dataset_id));
    }
    obj.insert(
        "can_explain".to_string(),
        Value::Bool(node.is_some_and(|value| value.can_explain)),
    );
    Value::Object(obj)
}

