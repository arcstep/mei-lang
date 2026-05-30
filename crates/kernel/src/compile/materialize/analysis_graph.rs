use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::{Map, Value};

use crate::model::{AnalysisEdge, AnalysisGraph, AnalysisNode};

pub(crate) fn expand_runtime_metric_defs(metric_defs: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    let mut expanded = BTreeMap::new();
    for (metric_id, raw) in metric_defs {
        expand_metric_def(metric_id, raw, &mut expanded);
    }
    expanded
}

pub(crate) fn build_analysis_artifacts(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> (BTreeMap<String, Value>, AnalysisGraph, BTreeMap<String, Value>) {
    let expanded = expand_runtime_metric_defs(metric_defs);
    let graph = build_analysis_graph_from_expanded(&expanded, root_dataset_id);
    let contracts = build_analysis_contracts_from_expanded(&expanded, &graph, root_dataset_id);
    (expanded, graph, contracts)
}

pub(crate) fn build_analysis_graph(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> AnalysisGraph {
    let expanded = expand_runtime_metric_defs(metric_defs);
    build_analysis_graph_from_expanded(&expanded, root_dataset_id)
}

pub(crate) fn build_analysis_contracts(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> BTreeMap<String, Value> {
    let expanded = expand_runtime_metric_defs(metric_defs);
    let graph = build_analysis_graph_from_expanded(&expanded, root_dataset_id);
    build_analysis_contracts_from_expanded(&expanded, &graph, root_dataset_id)
}

pub(crate) fn analysis_closure_metric_ids(
    graph: &AnalysisGraph,
    focus_ids: &[String],
) -> Vec<String> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    for focus_id in focus_ids {
        let focus_id = focus_id.trim();
        if focus_id.is_empty() {
            continue;
        }
        if visited.insert(focus_id.to_string()) {
            queue.push_back(focus_id.to_string());
        }
    }
    while let Some(node_id) = queue.pop_front() {
        for edge in &graph.edges {
            if edge.from != node_id {
                continue;
            }
            let Some(target) = graph.nodes.get(&edge.to) else {
                continue;
            };
            if target.node_kind != "metric" {
                continue;
            }
            if visited.insert(edge.to.clone()) {
                queue.push_back(edge.to.clone());
            }
        }
    }
    visited.into_iter().collect()
}

fn expand_metric_def(metric_id: &str, raw: &Value, out: &mut BTreeMap<String, Value>) {
    let Some(map) = raw.as_object() else {
        out.insert(metric_id.to_string(), raw.clone());
        return;
    };
    let mut normalized = map.clone();
    if !normalized.contains_key("key") {
        normalized.insert("key".to_string(), Value::String(metric_id.to_string()));
    }
    normalized.insert(
        "analysis_node_id".to_string(),
        Value::String(metric_id.to_string()),
    );
    let explain = normalized
        .get("explain")
        .map(|value| rewrite_explain_scope(metric_id, value));
    if let Some(explain_value) = explain.as_ref() {
        normalized.insert("explain".to_string(), explain_value.clone());
    }
    out.insert(metric_id.to_string(), Value::Object(normalized));
    let Some(items) = explain.as_ref().and_then(Value::as_array) else {
        return;
    };
    for item in items {
        let Some(item_map) = item.as_object() else {
            continue;
        };
        if item_map.get("__kind").and_then(Value::as_str) != Some("data_product") {
            continue;
        }
        let Some(local_id) = child_metric_local_id(item_map) else {
            continue;
        };
        let scoped_id = scoped_child_metric_id(metric_id, &local_id);
        let mut child_metric = item_map.clone();
        child_metric.insert("key".to_string(), Value::String(scoped_id.clone()));
        child_metric.insert("id".to_string(), Value::String(scoped_id.clone()));
        child_metric.insert("analysis_local_id".to_string(), Value::String(local_id));
        child_metric.insert(
            "analysis_parent_metric_id".to_string(),
            Value::String(metric_id.to_string()),
        );
        child_metric.insert(
            "analysis_node_id".to_string(),
            Value::String(scoped_id.clone()),
        );
        expand_metric_def(&scoped_id, &Value::Object(child_metric), out);
    }
}

fn rewrite_explain_scope(metric_id: &str, value: &Value) -> Value {
    let Some(items) = value.as_array() else {
        return value.clone();
    };
    let local_ids = scope_local_metric_ids(metric_id, items);
    Value::Array(
        items.iter()
            .map(|item| rewrite_scope_item(metric_id, item, &local_ids))
            .collect(),
    )
}

fn scope_local_metric_ids(metric_id: &str, items: &[Value]) -> BTreeMap<String, String> {
    let mut ids = BTreeMap::new();
    for item in items {
        let Some(map) = item.as_object() else {
            continue;
        };
        if map.get("__kind").and_then(Value::as_str) != Some("data_product") {
            continue;
        }
        let Some(local_id) = child_metric_local_id(map) else {
            continue;
        };
        ids.insert(local_id.clone(), scoped_child_metric_id(metric_id, &local_id));
    }
    ids
}

fn rewrite_scope_item(
    metric_id: &str,
    item: &Value,
    local_ids: &BTreeMap<String, String>,
) -> Value {
    let mut rewritten = rewrite_local_metric_refs(item, local_ids);
    let Some(map) = rewritten.as_object_mut() else {
        return rewritten;
    };
    map.insert(
        "analysis_parent_metric_id".to_string(),
        Value::String(metric_id.to_string()),
    );
    if map.get("__kind").and_then(Value::as_str) == Some("data_product") {
        if let Some(local_id) = child_metric_local_id(map) {
            map.insert(
                "analysis_local_id".to_string(),
                Value::String(local_id.clone()),
            );
            map.insert(
                "analysis_scoped_id".to_string(),
                Value::String(scoped_child_metric_id(metric_id, &local_id)),
            );
            map.insert("analysis_node_kind".to_string(), Value::String("metric".to_string()));
        }
        return rewritten;
    }
    let support_role = support_role_for_item(map);
    if !support_role.is_empty() {
        map.insert("support_role".to_string(), Value::String(support_role));
    }
    rewritten
}

fn rewrite_local_metric_refs(value: &Value, local_ids: &BTreeMap<String, String>) -> Value {
    match value {
        Value::Array(items) => Value::Array(
            items.iter()
                .map(|item| rewrite_local_metric_refs(item, local_ids))
                .collect(),
        ),
        Value::Object(map) => {
            let mut rewritten = serde_json::Map::new();
            for (key, child) in map {
                rewritten.insert(key.clone(), rewrite_local_metric_refs(child, local_ids));
            }
            if rewritten.get("__ref").and_then(Value::as_str) == Some("metric")
                && !rewritten.contains_key("from_dataset")
            {
                if let Some(local_id) = rewritten
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    if let Some(scoped_id) = local_ids.get(local_id) {
                        rewritten.insert("id".to_string(), Value::String(scoped_id.clone()));
                    }
                }
            }
            Value::Object(rewritten)
        }
        _ => value.clone(),
    }
}

fn support_role_for_item(map: &serde_json::Map<String, Value>) -> String {
    let raw = map
        .get("kind")
        .or_else(|| map.get("type"))
        .or_else(|| map.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    normalize_role_id(raw)
}

fn child_metric_local_id(map: &serde_json::Map<String, Value>) -> Option<String> {
    map.get("key")
        .or_else(|| map.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn scoped_child_metric_id(parent_metric_id: &str, local_id: &str) -> String {
    format!("{}::{}", parent_metric_id.trim(), local_id.trim())
}

fn normalize_role_id(value: &str) -> String {
    let raw = value.trim().to_lowercase();
    match raw.as_str() {
        "definition" | "metric_definition" | "metric-definition" => "definition".to_string(),
        "detail" | "details" => "detail".to_string(),
        "trend" | "trend_compare" | "timeseries" | "time_series" | "time-series" => {
            "trend".to_string()
        }
        "composition" | "breakdown" | "group" | "group_by" | "groupby" => {
            "composition".to_string()
        }
        "numerator_denominator" | "numerator-denominator" | "ratio" | "numerator" => {
            "numerator_denominator".to_string()
        }
        "note" | "text" | "md" | "markdown" => "note".to_string(),
        _ => raw.replace([' ', '-'], "_"),
    }
}

fn build_analysis_graph_from_expanded(
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
        let Some(items) = map.get("explain").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(item_map) = item.as_object() else {
                continue;
            };
            if item_map.get("__kind").and_then(Value::as_str) == Some("data_product") {
                if let Some(scoped_id) =
                    first_non_empty_string(item_map, &["analysis_scoped_id", "analysis_node_id", "id", "key"])
                {
                    push_edge(&mut edge_set, metric_id, &scoped_id, "scope_metric");
                }
                continue;
            }
            let role = support_role_for_item(item_map);
            let Some(target_metric_id) = metric_target_from_item(item_map) else {
                let block_id = narrative_block_id(metric_id, item_map);
                graph.nodes.entry(block_id.clone()).or_insert_with(|| {
                    narrative_node_from_item(&block_id, metric_id, item_map, root_dataset_id)
                });
                push_edge(&mut edge_set, metric_id, &block_id, &role);
                continue;
            };
            graph.nodes.entry(target_metric_id.clone()).or_insert_with(|| AnalysisNode {
                id: target_metric_id.clone(),
                canonical_metric_id: Some(target_metric_id.clone()),
                parent_id: Some(metric_id.clone()),
                node_kind: "metric".to_string(),
                support_role: Some(role.clone()),
                shape: None,
                label: first_non_empty_string(item_map, &["label"]),
                lineage_dataset_id: Some(root_dataset_id.to_string()),
                can_explain: false,
            });
            push_edge(&mut edge_set, metric_id, &target_metric_id, &role);
        }
    }
    graph.edges = edge_set
        .into_iter()
        .map(|(from, to, role)| AnalysisEdge { from, to, role })
        .collect();
    graph
}

fn build_analysis_contracts_from_expanded(
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

fn build_metric_contract(
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
    let mut tabs = Vec::<Value>::new();
    let mut tab_metrics = Map::new();
    let mut nodes = Vec::<Value>::new();
    let mut objects = Map::new();
    let mut blocks = Vec::<Value>::new();
    let mut seen_tabs = BTreeSet::new();
    let mut seen_nodes = BTreeSet::new();
    let default_tabular_metric_id = map
        .and_then(|value| value.get("explain"))
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                let item_map = item.as_object()?;
                if item_map.get("__kind").and_then(Value::as_str) != Some("data_product") {
                    return None;
                }
                first_non_empty_string(item_map, &["analysis_scoped_id", "analysis_node_id", "id", "key"])
            })
        });
    if let Some(items) = map.and_then(|value| value.get("explain")).and_then(Value::as_array) {
        for item in items {
            let Some(item_map) = item.as_object() else {
                continue;
            };
            if item_map.get("__kind").and_then(Value::as_str) == Some("data_product") {
                let Some(node_id) =
                    first_non_empty_string(item_map, &["analysis_scoped_id", "analysis_node_id", "id", "key"])
                else {
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
                continue;
            }
            let support_role = support_role_for_item(item_map);
            let block = explain_block_value(item_map, graph, root_dataset_id, default_tabular_metric_id.as_deref());
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

fn metric_node_from_raw(metric_id: &str, raw: &Value, root_dataset_id: &str) -> AnalysisNode {
    let map = raw.as_object();
    AnalysisNode {
        id: metric_id.to_string(),
        canonical_metric_id: Some(metric_id.to_string()),
        parent_id: map.and_then(|value| first_non_empty_string(value, &["analysis_parent_metric_id"])),
        node_kind: "metric".to_string(),
        support_role: None,
        shape: map.and_then(detect_metric_shape),
        label: map.and_then(|value| first_non_empty_string(value, &["label", "title"])),
        lineage_dataset_id: map
            .and_then(|value| first_non_empty_string(value, &["dataset", "dataset_id"]))
            .or_else(|| (!root_dataset_id.trim().is_empty()).then(|| root_dataset_id.to_string())),
        can_explain: map
            .and_then(|value| value.get("explain"))
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty()),
    }
}

fn narrative_node_from_item(
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
        support_role: Some(support_role_for_item(item_map)),
        shape: None,
        label: first_non_empty_string(item_map, &["label", "title"]),
        lineage_dataset_id: Some(root_dataset_id.to_string()),
        can_explain: false,
    }
}

fn analysis_node_value(
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
    obj.insert(
        "can_explain".to_string(),
        Value::Bool(node.is_some_and(|value| value.can_explain)),
    );
    Value::Object(obj)
}

fn explain_block_value(
    item_map: &Map<String, Value>,
    graph: &AnalysisGraph,
    root_dataset_id: &str,
    default_tabular_metric_id: Option<&str>,
) -> Map<String, Value> {
    let support_role = support_role_for_item(item_map);
    let mut block = Map::new();
    let block_id = first_non_empty_string(item_map, &["id"])
        .unwrap_or_else(|| support_role.clone());
    block.insert("id".to_string(), Value::String(block_id));
    block.insert("kind".to_string(), Value::String(support_role.clone()));
    block.insert(
        "support_role".to_string(),
        Value::String(support_role.clone()),
    );
    copy_field(item_map, &mut block, "label");
    copy_field(item_map, &mut block, "note");
    copy_field(item_map, &mut block, "content");
    copy_field(item_map, &mut block, "format");
    copy_field(item_map, &mut block, "basis_refs");
    copy_field(item_map, &mut block, "recommended_dimensions");
    copy_field(item_map, &mut block, "numerator");
    copy_field(item_map, &mut block, "denominator");
    copy_field(item_map, &mut block, "formula");
    copy_field(item_map, &mut block, "by");
    copy_field(item_map, &mut block, "date_field");
    copy_field(item_map, &mut block, "grain");
    copy_field(item_map, &mut block, "fields");
    copy_field(item_map, &mut block, "headers");
    copy_field(item_map, &mut block, "mapping");
    copy_field(item_map, &mut block, "chart_kind");
    let target_metric_id = metric_target_from_item(item_map)
        .or_else(|| default_tabular_metric_id.map(str::to_string).filter(|_| {
            matches!(
                support_role.as_str(),
                "detail" | "trend" | "composition" | "attribution"
            )
        }));
    if let Some(metric_id) = target_metric_id {
        block.insert("node_id".to_string(), Value::String(metric_id.clone()));
        block.insert("metric_id".to_string(), Value::String(metric_id.clone()));
        let runtime_ref = metric_runtime_ref(
            graph.nodes.get(&metric_id),
            &metric_id,
            root_dataset_id,
        );
        block.insert("runtime_ref".to_string(), Value::Object(runtime_ref));
    } else if let Some(dataset_id) = dataset_target_from_item(item_map) {
        let mut runtime_ref = Map::new();
        runtime_ref.insert("kind".to_string(), Value::String("data".to_string()));
        runtime_ref.insert("dataset_id".to_string(), Value::String(dataset_id.clone()));
        block.insert("dataset_id".to_string(), Value::String(dataset_id));
        block.insert("runtime_ref".to_string(), Value::Object(runtime_ref));
    }
    block
}

fn tab_metric_value(block: &Map<String, Value>) -> Option<Map<String, Value>> {
    let role = block
        .get("support_role")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let mut out = Map::new();
    out.insert("support_role".to_string(), Value::String(role.to_string()));
    for key in [
        "node_id",
        "metric_id",
        "runtime_ref",
        "fields",
        "headers",
        "mapping",
        "chart_kind",
        "by",
        "date_field",
        "grain",
        "label",
    ] {
        if let Some(value) = block.get(key).cloned() {
            out.insert(key.to_string(), value);
        }
    }
    Some(out)
}

fn metric_runtime_ref(
    node: Option<&AnalysisNode>,
    metric_id: &str,
    root_dataset_id: &str,
) -> Map<String, Value> {
    let mut runtime_ref = Map::new();
    runtime_ref.insert("kind".to_string(), Value::String("metric".to_string()));
    runtime_ref.insert("metric_id".to_string(), Value::String(metric_id.to_string()));
    if let Some(dataset_id) = node
        .and_then(|value| value.lineage_dataset_id.clone())
        .or_else(|| (!root_dataset_id.trim().is_empty()).then(|| root_dataset_id.to_string()))
    {
        runtime_ref.insert("dataset_id".to_string(), Value::String(dataset_id));
    }
    runtime_ref
}

fn metric_target_from_item(item_map: &Map<String, Value>) -> Option<String> {
    if let Some(source) = item_map.get("source").and_then(Value::as_object) {
        if source.get("__ref").and_then(Value::as_str) == Some("metric") {
            return first_non_empty_string(source, &["id"]);
        }
    }
    first_non_empty_string(item_map, &["metric_id", "metricId"])
}

fn dataset_target_from_item(item_map: &Map<String, Value>) -> Option<String> {
    if let Some(source) = item_map.get("source").and_then(Value::as_object) {
        if source.get("__ref").and_then(Value::as_str) == Some("data") {
            return first_non_empty_string(source, &["from_dataset", "id"]);
        }
    }
    None
}

fn narrative_block_id(parent_metric_id: &str, item_map: &Map<String, Value>) -> String {
    let local_id = first_non_empty_string(item_map, &["id"])
        .unwrap_or_else(|| support_role_for_item(item_map));
    format!("{parent_metric_id}#{}", local_id.trim())
}

fn detect_metric_shape(map: &Map<String, Value>) -> Option<String> {
    if let Some(shape) = first_non_empty_string(map, &["shape"]) {
        return Some(shape);
    }
    if map.get("values").and_then(Value::as_object).is_some() {
        return Some("scalar".to_string());
    }
    if map.get("value").and_then(Value::as_array).is_some() {
        return Some("dataframe".to_string());
    }
    map.get("value")
        .map(|value| if value.is_array() { "dataframe" } else { "scalar" }.to_string())
}

fn first_non_empty_string(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| {
            map.get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

fn copy_field(source: &Map<String, Value>, out: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).cloned() {
        out.insert(key.to_string(), value);
    }
}

fn push_edge(edges: &mut BTreeSet<(String, String, String)>, from: &str, to: &str, role: &str) {
    let from = from.trim();
    let to = to.trim();
    if from.is_empty() || to.is_empty() {
        return;
    }
    let role = role.trim();
    edges.insert((
        from.to_string(),
        to.to_string(),
        if role.is_empty() {
            "support".to_string()
        } else {
            role.to_string()
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::{analysis_closure_metric_ids, build_analysis_contracts, build_analysis_graph};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn analysis_closure_metric_ids_walks_scoped_metric_children() {
        let defs = BTreeMap::from([(
            "sales_total".to_string(),
            json!({
                "key": "sales_total",
                "label": "销售总额",
                "explain": [
                    {
                        "__kind": "data_product",
                        "id": "detail_table",
                        "shape": "dataframe",
                        "value": [{"id": "A"}],
                        "explain": [
                            {
                                "__kind": "data_product",
                                "id": "detail_leaf",
                                "shape": "dataframe",
                                "value": [{"id": "leaf"}]
                            }
                        ]
                    },
                    {
                        "__kind": "explain_item",
                        "id": "detail",
                        "kind": "detail",
                        "source": {"__ref": "metric", "id": "detail_table"}
                    }
                ]
            }),
        )]);
        let graph = build_analysis_graph(&defs, "sales_metrics");
        let closure = analysis_closure_metric_ids(&graph, &["sales_total".to_string()]);
        assert_eq!(
            closure,
            vec![
                "sales_total".to_string(),
                "sales_total::detail_table".to_string(),
                "sales_total::detail_table::detail_leaf".to_string(),
            ]
        );
    }

    #[test]
    fn build_analysis_contracts_ignores_legacy_explain_object() {
        let defs = BTreeMap::from([(
            "sales_total".to_string(),
            json!({
                "key": "sales_total",
                "label": "销售总额",
                "explain": {
                    "note": "legacy explain object",
                    "detail_table_metric_id": "detail_table"
                }
            }),
        )]);
        let contracts = build_analysis_contracts(&defs, "sales_metrics");
        assert!(
            contracts.get("sales_total").is_none(),
            "legacy explain object should not build analysis contracts"
        );
    }
}
