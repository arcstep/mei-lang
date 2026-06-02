use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::{Map, Value};

use crate::model::{AnalysisEdge, AnalysisGraph, AnalysisNode, SemanticEdgeKind, SemanticNodeKind};

const INFERRED_SCALAR_ROWSET_LOCAL_ID: &str = "__scalar_rowset__";

/// Expand authored/runtime metric defs into the runtime-authoritative metric
/// definition map.
///
/// This step lowers explain-scope local objects into scoped metric ids so that
/// runtime evaluation, cache identity, and semantic graph construction all
/// share the same canonical metric space.
pub(crate) fn expand_runtime_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    let mut expanded = BTreeMap::new();
    for (metric_id, raw) in metric_defs {
        expand_metric_def(metric_id, raw, &mut expanded);
    }
    expanded
}

pub(crate) fn build_analysis_artifacts(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> (
    BTreeMap<String, Value>,
    AnalysisGraph,
    BTreeMap<String, Value>,
) {
    let expanded = expand_runtime_metric_defs(metric_defs);
    let graph = build_analysis_graph_from_expanded(&expanded, root_dataset_id);
    let contracts = build_analysis_contracts_from_expanded(&expanded, &graph, root_dataset_id);
    (expanded, graph, contracts)
}

/// Build the compile-derived semantic analysis graph.
///
/// This graph is the current semantic DAG artifact. It should not be confused
/// with request-time evaluation dependencies recorded by `RequestDag`.
pub(crate) fn build_analysis_graph(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> AnalysisGraph {
    let expanded = expand_runtime_metric_defs(metric_defs);
    build_analysis_graph_from_expanded(&expanded, root_dataset_id)
}

/// Build consumer projection contracts from expanded runtime metric defs plus
/// the semantic analysis graph.
///
/// These contracts are for consumers such as drilldown/popup and are not a
/// semantic or runtime-evaluation source of truth.
pub(crate) fn build_analysis_contracts(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> BTreeMap<String, Value> {
    let expanded = expand_runtime_metric_defs(metric_defs);
    let graph = build_analysis_graph_from_expanded(&expanded, root_dataset_id);
    build_analysis_contracts_from_expanded(&expanded, &graph, root_dataset_id)
}

/// Select the metric workset implied by semantic analysis closure.
///
/// This walks compile-derived semantic edges to discover reachable metric
/// nodes. It is intentionally narrower than the request eval DAG: it selects
/// metric defs to consider for evaluation but does not express execution order
/// or expression dependencies.
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
            if !edge.participates_in_default_closure() {
                continue;
            }
            if !target.participates_in_metric_closure() {
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
    maybe_hoist_inferred_scalar_rowset(metric_id, &normalized, out);
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

fn maybe_hoist_inferred_scalar_rowset(
    metric_id: &str,
    normalized: &Map<String, Value>,
    out: &mut BTreeMap<String, Value>,
) {
    let Some(items) = normalized.get("explain").and_then(Value::as_array) else {
        return;
    };
    if !explain_needs_tabular_source(items) {
        return;
    }
    let Some(rowset) = extract_primary_scalar_rowset(normalized) else {
        return;
    };
    let local_id = INFERRED_SCALAR_ROWSET_LOCAL_ID.to_string();
    let scoped_id = scoped_child_metric_id(metric_id, &local_id);
    if out.contains_key(&scoped_id) {
        return;
    }
    let mut child_metric = Map::new();
    child_metric.insert(
        "__kind".to_string(),
        Value::String("data_product".to_string()),
    );
    child_metric.insert("id".to_string(), Value::String(local_id.clone()));
    child_metric.insert("shape".to_string(), Value::String("dataframe".to_string()));
    child_metric.insert("value".to_string(), rowset);
    child_metric.insert(
        "analysis_inferred_scalar_rowset".to_string(),
        Value::Bool(true),
    );
    child_metric.insert("key".to_string(), Value::String(scoped_id.clone()));
    child_metric.insert(
        "analysis_local_id".to_string(),
        Value::String(local_id.clone()),
    );
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

fn explain_needs_tabular_source(items: &[Value]) -> bool {
    items.iter().any(|item| {
        item.as_object().is_some_and(|map| {
            matches!(
                support_role_for_item(map).as_str(),
                "detail" | "trend" | "composition" | "attribution"
            )
        })
    })
}

fn extract_primary_scalar_rowset(metric: &Map<String, Value>) -> Option<Value> {
    let expr = primary_scalar_value_expr(metric)?;
    extract_rowset_from_scalar_expr(&expr)
}

fn primary_scalar_value_expr(metric: &Map<String, Value>) -> Option<Value> {
    if let Some(values) = metric.get("values").and_then(Value::as_object) {
        if let Some(value) = values.get("value") {
            return Some(value.clone());
        }
        return values.values().next().cloned();
    }
    metric.get("value").cloned()
}

fn extract_rowset_from_scalar_expr(expr: &Value) -> Option<Value> {
    let map = expr.as_object()?;
    if map.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
        return is_rowset_expression(expr).then(|| expr.clone());
    }
    match map.get("type").and_then(Value::as_str)? {
        "count" | "percent" | "item_count" => map
            .get("rowset")
            .or_else(|| map.get("value"))
            .cloned()
            .filter(|value| is_rowset_expression(value)),
        "sum" | "avg" | "min" | "max" | "median" | "unique_count" => map
            .get("value")
            .and_then(extract_rowset_from_numeric_source),
        "sum_rowset_counts" => map
            .get("rowsets")
            .and_then(Value::as_array)
            .and_then(|rowsets| rowsets.first())
            .cloned()
            .filter(|value| is_rowset_expression(value)),
        _ => None,
    }
}

fn extract_rowset_from_numeric_source(expr: &Value) -> Option<Value> {
    let map = expr.as_object()?;
    if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
        match map.get("type").and_then(Value::as_str)? {
            "number" => map
                .get("rowset")
                .or_else(|| map.get("source"))
                .cloned()
                .filter(|value| is_rowset_expression(value)),
            _ => extract_rowset_from_scalar_expr(expr),
        }
    } else {
        is_rowset_expression(expr).then(|| expr.clone())
    }
}

fn is_rowset_expression(expr: &Value) -> bool {
    match expr {
        Value::Array(_) => true,
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                return true;
            }
            if map.get("__kind").and_then(Value::as_str) != Some("analysis_expr") {
                return false;
            }
            match map.get("type").and_then(Value::as_str).unwrap_or_default() {
                "rows" => true,
                "count" | "sum" | "avg" | "min" | "max" | "median" | "ratio" | "percent"
                | "unique_count" | "item_count" | "sum_first_number" | "sum_rowset_counts"
                | "mom" | "yoy" | "change_rate" | "lit" | "number" => false,
                _ => true,
            }
        }
        _ => false,
    }
}

fn inferred_scalar_rowset_metric_id(metric_id: &str) -> String {
    scoped_child_metric_id(metric_id, INFERRED_SCALAR_ROWSET_LOCAL_ID)
}

fn infer_inferred_scalar_rowset_metric_id(
    graph: &AnalysisGraph,
    metric_id: &str,
) -> Option<String> {
    let scoped = inferred_scalar_rowset_metric_id(metric_id);
    graph.nodes.contains_key(&scoped).then_some(scoped)
}

fn infer_inferred_scalar_rowset_metric_id_from_defs(
    metric_id: &str,
    expanded_defs: &BTreeMap<String, Value>,
) -> Option<String> {
    let scoped = inferred_scalar_rowset_metric_id(metric_id);
    expanded_defs.contains_key(&scoped).then_some(scoped)
}

fn rewrite_explain_scope(metric_id: &str, value: &Value) -> Value {
    let Some(items) = value.as_array() else {
        return value.clone();
    };
    let local_ids = scope_local_metric_ids(metric_id, items);
    Value::Array(
        items
            .iter()
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
        ids.insert(
            local_id.clone(),
            scoped_child_metric_id(metric_id, &local_id),
        );
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
            map.insert(
                "analysis_node_kind".to_string(),
                Value::String("metric".to_string()),
            );
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
            items
                .iter()
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
        "composition" | "breakdown" | "group" | "group_by" | "groupby" => "composition".to_string(),
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
                if let Some(dataset_id) = dataset_target_from_item(item_map).or_else(|| {
                    unique_lineage_dataset_id_from_metric_values(raw, role.as_str())
                }) {
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

fn metric_node_from_raw(metric_id: &str, raw: &Value, root_dataset_id: &str) -> AnalysisNode {
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
        semantic_kind: SemanticNodeKind::NarrativeSupport,
        support_role: Some(support_role_for_item(item_map)),
        shape: None,
        label: first_non_empty_string(item_map, &["label", "title"]),
        lineage_dataset_id: Some(root_dataset_id.to_string()),
        tabular_source_dataset_id: None,
        can_explain: false,
    }
}

fn tabular_node_from_dataset_id(node_id: &str, dataset_id: &str) -> AnalysisNode {
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

fn explain_block_value(
    metric_id: &str,
    raw: &Value,
    item_map: &Map<String, Value>,
    explain_items: &[Value],
    item_index: usize,
    graph: &AnalysisGraph,
    root_dataset_id: &str,
) -> Map<String, Value> {
    let support_role = support_role_for_item(item_map);
    let mut block = Map::new();
    let block_id =
        first_non_empty_string(item_map, &["id"]).unwrap_or_else(|| support_role.clone());
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
    let scoped_dataframe_metric_id =
        infer_explain_scoped_dataframe(explain_items, item_index, support_role.as_str());
    let lineage_dataset_id =
        infer_unique_lineage_dataset_id(raw, graph, metric_id, support_role.as_str());
    let target_metric_id = metric_target_from_item(item_map).or_else(|| {
        if support_role == "detail" && explain_has_support_role(explain_items, "composition") {
            infer_inferred_scalar_rowset_metric_id(graph, metric_id)
        } else {
            scoped_dataframe_metric_id.clone().or_else(|| {
                infer_inferred_scalar_rowset_metric_id(graph, metric_id)
            })
        }
    });
    if let Some(tabular_metric_id) = target_metric_id {
        block.insert(
            "node_id".to_string(),
            Value::String(tabular_metric_id.clone()),
        );
        block.insert(
            "metric_id".to_string(),
            Value::String(tabular_metric_id.clone()),
        );
        let runtime_ref = metric_runtime_ref(
            graph.nodes.get(&tabular_metric_id),
            &tabular_metric_id,
            root_dataset_id,
        );
        block.insert("runtime_ref".to_string(), Value::Object(runtime_ref));
    } else if let Some(dataset_id) = dataset_target_from_item(item_map).or(lineage_dataset_id) {
        let tabular_node_id = tabular_source_node_id(&dataset_id);
        let mut runtime_ref = Map::new();
        runtime_ref.insert("kind".to_string(), Value::String("data".to_string()));
        runtime_ref.insert("dataset_id".to_string(), Value::String(dataset_id.clone()));
        block.insert("node_id".to_string(), Value::String(tabular_node_id));
        block.insert("dataset_id".to_string(), Value::String(dataset_id));
        block.insert("runtime_ref".to_string(), Value::Object(runtime_ref));
    }
    block
}

fn collect_data_ref_dataset_ids(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                if let Some(dataset_id) = first_non_empty_string(map, &["from_dataset", "id"]) {
                    out.insert(dataset_id);
                }
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
                && map.get("type").and_then(Value::as_str) == Some("rows")
            {
                if let Some(dataset_id) = first_non_empty_string(map, &["dataset"]) {
                    out.insert(dataset_id);
                }
            }
            for child in map.values() {
                collect_data_ref_dataset_ids(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_data_ref_dataset_ids(child, out);
            }
        }
        _ => {}
    }
}

fn collect_metric_value_lineage_dataset_ids(raw: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(map) = raw.as_object() else {
        return out;
    };
    if let Some(values) = map.get("values") {
        collect_data_ref_dataset_ids(values, &mut out);
    }
    if let Some(value) = map.get("value") {
        collect_data_ref_dataset_ids(value, &mut out);
    }
    out
}

fn lineage_dataset_ids_from_graph(graph: &AnalysisGraph, metric_id: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for edge in &graph.edges {
        if edge.from != metric_id || edge.role != "lineage" {
            continue;
        }
        let Some(node) = graph.nodes.get(&edge.to) else {
            continue;
        };
        if node.semantic_kind() != SemanticNodeKind::TabularSource {
            continue;
        }
        if let Some(dataset_id) = node
            .tabular_source_dataset_id
            .as_deref()
            .or(node.lineage_dataset_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            out.insert(dataset_id.to_string());
        }
    }
    out
}

fn infer_unique_lineage_dataset_id(
    raw: &Value,
    graph: &AnalysisGraph,
    metric_id: &str,
    support_role: &str,
) -> Option<String> {
    if !matches!(
        support_role,
        "detail" | "trend" | "composition" | "attribution"
    ) {
        return None;
    }
    unique_lineage_dataset_id_from_metric_values(raw, support_role).or_else(|| {
        let from_graph = lineage_dataset_ids_from_graph(graph, metric_id);
        if from_graph.len() == 1 {
            from_graph.into_iter().next()
        } else {
            None
        }
    })
}

fn unique_lineage_dataset_id_from_metric_values(
    raw: &Value,
    support_role: &str,
) -> Option<String> {
    if !matches!(
        support_role,
        "detail" | "trend" | "composition" | "attribution"
    ) {
        return None;
    }
    if let Some(map) = raw.as_object() {
        if let Some(dataset_id) = first_non_empty_string(map, &["dataset", "dataset_id"]) {
            return Some(dataset_id);
        }
    }
    let selectors = collect_metric_value_lineage_dataset_ids(raw);
    if selectors.len() == 1 {
        return selectors.into_iter().next();
    }
    None
}

fn scoped_dataframe_metric_id(item_map: &Map<String, Value>) -> Option<String> {
    first_non_empty_string(
        item_map,
        &["analysis_scoped_id", "analysis_node_id", "id", "key"],
    )
}

fn infer_explain_scoped_dataframe(
    explain_items: &[Value],
    current_index: usize,
    support_role: &str,
) -> Option<String> {
    if !matches!(
        support_role,
        "detail" | "trend" | "composition" | "attribution"
    ) {
        return None;
    }
    if support_role == "detail" {
        return infer_explain_scoped_dataframe_for_detail(explain_items, current_index);
    }
    for item in explain_items[..current_index].iter().rev() {
        let Some(item_map) = item.as_object() else {
            continue;
        };
        if item_map.get("__kind").and_then(Value::as_str) != Some("data_product") {
            continue;
        }
        return scoped_dataframe_metric_id(item_map);
    }
    single_explain_scoped_dataframe(explain_items)
}

fn infer_explain_scoped_dataframe_for_detail(
    explain_items: &[Value],
    current_index: usize,
) -> Option<String> {
    for (index, item) in explain_items[..current_index].iter().enumerate().rev() {
        let Some(item_map) = item.as_object() else {
            continue;
        };
        if item_map.get("__kind").and_then(Value::as_str) != Some("data_product") {
            continue;
        };
        let has_tabular_support_between = explain_items[index + 1..current_index]
            .iter()
            .filter_map(Value::as_object)
            .map(support_role_for_item)
            .any(|role| role == "trend" || role == "composition");
        if has_tabular_support_between {
            continue;
        }
        return scoped_dataframe_metric_id(item_map);
    }
    single_explain_scoped_dataframe(explain_items)
}

fn single_explain_scoped_dataframe(explain_items: &[Value]) -> Option<String> {
    let scoped: Vec<String> = explain_items
        .iter()
        .filter_map(|item| {
            let item_map = item.as_object()?;
            if item_map.get("__kind").and_then(Value::as_str) != Some("data_product") {
                return None;
            }
            scoped_dataframe_metric_id(item_map)
        })
        .collect();
    if scoped.len() == 1 {
        return scoped.into_iter().next();
    }
    None
}

fn explain_has_support_role(explain_items: &[Value], role: &str) -> bool {
    explain_items.iter().any(|item| {
        item.as_object()
            .is_some_and(|map| support_role_for_item(map) == role)
    })
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
        "dataset_id",
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
    runtime_ref.insert(
        "metric_id".to_string(),
        Value::String(metric_id.to_string()),
    );
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

fn tabular_source_node_id(dataset_id: &str) -> String {
    format!("tabular::{}", dataset_id.trim())
}

fn semantic_node_kind_name(kind: SemanticNodeKind) -> &'static str {
    match kind {
        SemanticNodeKind::Metric => "metric",
        SemanticNodeKind::NarrativeSupport => "narrative_support",
        SemanticNodeKind::TabularSource => "tabular_source",
        SemanticNodeKind::Unknown => "unknown",
    }
}

fn semantic_edge_kind_from_role(role: &str) -> SemanticEdgeKind {
    match role.trim() {
        "scope_metric" => SemanticEdgeKind::ScopeMetric,
        "support"
        | "definition"
        | "detail"
        | "trend"
        | "composition"
        | "numerator_denominator"
        | "attribution"
        | "note" => SemanticEdgeKind::Support,
        "lineage" => SemanticEdgeKind::Lineage,
        "association" => SemanticEdgeKind::Association,
        "reuse" => SemanticEdgeKind::Reuse,
        _ => SemanticEdgeKind::Unknown,
    }
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
    map.get("value").map(|value| {
        if value.is_array() {
            "dataframe"
        } else {
            "scalar"
        }
        .to_string()
    })
}

fn first_non_empty_string(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        map.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn metric_note_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str().map(str::trim).filter(|text| !text.is_empty()) {
        return Some(text.to_string());
    }
    value
        .as_object()
        .and_then(|map| first_non_empty_string(map, &["content", "text", "note"]))
}

fn metric_note_format(value: &Value) -> Option<String> {
    if value.as_str().is_some() {
        return Some("text".to_string());
    }
    value
        .as_object()
        .and_then(|map| first_non_empty_string(map, &["format"]))
        .or_else(|| metric_note_text(value).map(|_| "text".to_string()))
}

fn apply_metric_narrative(map: &Map<String, Value>, contract: &mut Map<String, Value>) {
    if let Some(note_value) = map.get("note") {
        if let Some(text) = metric_note_text(note_value) {
            contract.insert("note".to_string(), Value::String(text));
            if let Some(format) = metric_note_format(note_value) {
                contract.insert("note_format".to_string(), Value::String(format));
            }
        }
    }
    for key in ["basis_refs", "recommended_dimensions"] {
        if contract.contains_key(key) {
            continue;
        }
        if let Some(value) = map.get(key).filter(|value| !value_is_empty(value)) {
            contract.insert(key.to_string(), value.clone());
        }
    }
}

fn merge_definition_narrative_fallback(item_map: &Map<String, Value>, contract: &mut Map<String, Value>) {
    if !contract.contains_key("note") {
        if let Some(note) = first_non_empty_string(
            item_map,
            &["note", "content", "text", "markdown", "md", "desc", "description"],
        ) {
            contract.insert("note".to_string(), Value::String(note));
            contract.insert("note_format".to_string(), Value::String("text".to_string()));
        }
    }
    if !contract.contains_key("basis_refs") {
        if let Some(value) = item_map.get("basis_refs").filter(|value| !value_is_empty(value)) {
            contract.insert("basis_refs".to_string(), value.clone());
        }
    }
    if !contract.contains_key("recommended_dimensions") {
        if let Some(value) = item_map
            .get("recommended_dimensions")
            .filter(|value| !value_is_empty(value))
        {
            contract.insert("recommended_dimensions".to_string(), value.clone());
        }
    }
}

fn is_empty_legacy_definition_item(item_map: &Map<String, Value>) -> bool {
    support_role_for_item(item_map) == "definition"
        && metric_note_text(&Value::Object(item_map.clone())).is_none()
        && item_map
            .get("basis_refs")
            .is_none_or(|value| value_is_empty(value))
        && item_map
            .get("recommended_dimensions")
            .is_none_or(|value| value_is_empty(value))
}

fn value_is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => false,
    }
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
    use crate::model::{
        AnalysisEdge, AnalysisGraph, AnalysisNode, SemanticEdgeKind, SemanticNodeKind,
    };
    use serde_json::{json, Value};
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
    fn analysis_closure_metric_ids_ignores_narrative_support_nodes() {
        let defs = BTreeMap::from([(
            "sales_total".to_string(),
            json!({
                "key": "sales_total",
                "label": "销售总额",
                "explain": [
                    {
                        "__kind": "explain_item",
                        "id": "definition",
                        "kind": "definition",
                        "note": "口径说明"
                    },
                    {
                        "__kind": "data_product",
                        "id": "detail_table",
                        "shape": "dataframe",
                        "value": [{"id": "A"}]
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
            ]
        );
        assert!(
            !closure.iter().any(|id| id.contains('#')),
            "semantic closure used for runtime metric selection should ignore narrative-only support nodes"
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

    #[test]
    fn build_analysis_graph_emits_tabular_sources_and_lineage_edges() {
        let defs = BTreeMap::from([(
            "sales_total".to_string(),
            json!({
                "key": "sales_total",
                "label": "销售总额",
                "dataset": "warning_list",
                "explain": [
                    {
                        "__kind": "explain_item",
                        "id": "detail",
                        "kind": "detail",
                        "source": {"__ref": "data", "from_dataset": "warning_detail"}
                    }
                ]
            }),
        )]);
        let graph = build_analysis_graph(&defs, "sales_metrics");
        let root_tabular = graph
            .nodes
            .get("tabular::warning_list")
            .expect("root dataset should materialize as tabular source");
        assert_eq!(
            root_tabular.semantic_kind(),
            SemanticNodeKind::TabularSource
        );
        assert_eq!(
            root_tabular.tabular_source_dataset_id.as_deref(),
            Some("warning_list")
        );
        let detail_tabular = graph
            .nodes
            .get("tabular::warning_detail")
            .expect("detail dataset should materialize as tabular source");
        assert_eq!(
            detail_tabular.semantic_kind(),
            SemanticNodeKind::TabularSource
        );
        assert!(graph.edges.iter().any(|edge| {
            edge.from == "sales_total"
                && edge.to == "tabular::warning_list"
                && edge.semantic_kind() == SemanticEdgeKind::Lineage
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.from == "sales_total"
                && edge.to == "tabular::warning_detail"
                && edge.semantic_kind() == SemanticEdgeKind::Lineage
        }));
        assert!(
            graph.validate_invariants().is_empty(),
            "tabular lineage graph should satisfy semantic invariants"
        );
    }

    #[test]
    fn build_analysis_contracts_infers_detail_from_explain_scoped_dataframe() {
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
                        "value": [{"id": "A"}]
                    },
                    {
                        "__kind": "explain_item",
                        "id": "detail",
                        "kind": "detail",
                        "fields": ["id"]
                    }
                ]
            }),
        )]);
        let contracts = build_analysis_contracts(&defs, "sales_metrics");
        let contract = contracts.get("sales_total").expect("contract");
        let tab_metrics = contract
            .get("tab_metrics")
            .and_then(Value::as_object)
            .expect("tab_metrics");
        let detail = tab_metrics
            .get("detail")
            .and_then(Value::as_object)
            .expect("detail tab");
        assert_eq!(
            detail.get("metric_id").and_then(Value::as_str),
            Some("sales_total::detail_table")
        );
    }

    #[test]
    fn build_analysis_contracts_infers_detail_from_metric_value_lineage() {
        let defs = BTreeMap::from([(
            "unit_count".to_string(),
            json!({
                "key": "unit_count",
                "label": "单位数",
                "values": {
                    "value": {
                        "__kind": "analysis_expr",
                        "type": "count",
                        "rowset": {"__ref": "data", "id": "enforcement_units"}
                    }
                },
                "explain": [
                    {
                        "__kind": "explain_item",
                        "id": "detail",
                        "kind": "detail",
                        "fields": ["序号", "类别"]
                    }
                ]
            }),
        )]);
        let contracts = build_analysis_contracts(&defs, "__world_metrics__");
        let contract = contracts.get("unit_count").expect("contract");
        let tab_metrics = contract
            .get("tab_metrics")
            .and_then(Value::as_object)
            .expect("tab_metrics");
        let detail = tab_metrics
            .get("detail")
            .and_then(Value::as_object)
            .expect("detail tab");
        assert_eq!(
            detail.get("metric_id").and_then(Value::as_str),
            Some("unit_count::__scalar_rowset__")
        );
        let runtime_ref = detail
            .get("runtime_ref")
            .and_then(Value::as_object)
            .expect("runtime_ref");
        assert_eq!(
            runtime_ref.get("kind").and_then(Value::as_str),
            Some("metric")
        );
    }

    #[test]
    fn build_analysis_graph_adds_lineage_from_metric_values() {
        let defs = BTreeMap::from([(
            "unit_count".to_string(),
            json!({
                "key": "unit_count",
                "values": {
                    "value": {
                        "__kind": "analysis_expr",
                        "type": "count",
                        "rowset": {"__ref": "data", "id": "enforcement_units"}
                    }
                },
                "explain": [
                    {
                        "__kind": "explain_item",
                        "id": "detail",
                        "kind": "detail",
                        "fields": ["序号"]
                    }
                ]
            }),
        )]);
        let graph = build_analysis_graph(&defs, "__world_metrics__");
        assert!(graph.edges.iter().any(|edge| {
            edge.from == "unit_count"
                && edge.to == "tabular::enforcement_units"
                && edge.semantic_kind() == SemanticEdgeKind::Lineage
        }));
    }

    #[test]
    fn build_analysis_contracts_detail_prefers_lineage_when_composition_present() {
        let defs = BTreeMap::from([(
            "verify_rate".to_string(),
            json!({
                "key": "verify_rate",
                "values": {
                    "value": {
                        "__kind": "analysis_expr",
                        "type": "percent",
                        "rowset": {"__ref": "data", "id": "warning_list"}
                    }
                },
                "explain": [
                    {
                        "__kind": "data_product",
                        "id": "breakdown_table",
                        "shape": "dataframe",
                        "value": [{"status": "yes", "value": 1}]
                    },
                    {
                        "__kind": "explain_item",
                        "id": "composition",
                        "kind": "composition",
                        "by": "status"
                    },
                    {
                        "__kind": "explain_item",
                        "id": "detail",
                        "kind": "detail",
                        "fields": ["预警ID"]
                    }
                ]
            }),
        )]);
        let contracts = build_analysis_contracts(&defs, "__world_metrics__");
        let contract = contracts.get("verify_rate").expect("contract");
        let tab_metrics = contract
            .get("tab_metrics")
            .and_then(Value::as_object)
            .expect("tab_metrics");
        let detail = tab_metrics
            .get("detail")
            .and_then(Value::as_object)
            .expect("detail tab");
        assert_eq!(
            detail.get("metric_id").and_then(Value::as_str),
            Some("verify_rate::__scalar_rowset__")
        );
        let composition = tab_metrics
            .get("composition")
            .and_then(Value::as_object)
            .expect("composition tab");
        assert_eq!(
            composition.get("metric_id").and_then(Value::as_str),
            Some("verify_rate::breakdown_table")
        );
    }

    #[test]
    fn build_analysis_contracts_infers_detail_from_scalar_rowset_without_explain_dataframe() {
        let defs = BTreeMap::from([(
            "transfer_clue_count".to_string(),
            json!({
                "key": "transfer_clue_count",
                "values": {
                    "value": {
                        "__kind": "analysis_expr",
                        "type": "count",
                        "rowset": {
                            "__kind": "analysis_expr",
                            "type": "first_by",
                            "rowset": {
                                "__kind": "analysis_expr",
                                "type": "where",
                                "rowset": {"__ref": "data", "id": "warning_list"},
                                "predicate": {"__kind": "analysis_expr", "type": "present", "field": "问题跟踪ID"}
                            },
                            "field": "问题跟踪ID"
                        }
                    }
                },
                "explain": [
                    {
                        "__kind": "explain_item",
                        "id": "detail",
                        "kind": "detail",
                        "fields": ["问题跟踪ID"]
                    }
                ]
            }),
        )]);
        let contracts = build_analysis_contracts(&defs, "__world_metrics__");
        let contract = contracts.get("transfer_clue_count").expect("contract");
        let tab_metrics = contract
            .get("tab_metrics")
            .and_then(Value::as_object)
            .expect("tab_metrics");
        let detail = tab_metrics
            .get("detail")
            .and_then(Value::as_object)
            .expect("detail tab");
        assert_eq!(
            detail.get("metric_id").and_then(Value::as_str),
            Some("transfer_clue_count::__scalar_rowset__")
        );
    }

    #[test]
    fn build_analysis_contracts_infers_recovered_funds_detail_from_sum_number_rowset() {
        let defs = BTreeMap::from([(
            "recovered_funds".to_string(),
            json!({
                "key": "recovered_funds",
                "values": {
                    "value": {
                        "__kind": "analysis_expr",
                        "type": "sum",
                        "value": {
                            "__kind": "analysis_expr",
                            "type": "number",
                            "rowset": {
                                "__kind": "analysis_expr",
                                "type": "first_by",
                                "rowset": {"__ref": "data", "id": "issue_result_list"},
                                "field": "处理结果ID"
                            },
                            "field": "挽回资金"
                        }
                    }
                },
                "explain": [
                    {
                        "__kind": "explain_item",
                        "id": "detail",
                        "kind": "detail",
                        "fields": ["处理结果ID", "挽回资金"]
                    }
                ]
            }),
        )]);
        let contracts = build_analysis_contracts(&defs, "__world_metrics__");
        let contract = contracts.get("recovered_funds").expect("contract");
        let detail = contract
            .get("tab_metrics")
            .and_then(Value::as_object)
            .and_then(|tabs| tabs.get("detail"))
            .and_then(Value::as_object)
            .expect("detail tab");
        assert_eq!(
            detail.get("metric_id").and_then(Value::as_str),
            Some("recovered_funds::__scalar_rowset__")
        );
    }

    #[test]
    fn build_analysis_contracts_reads_metric_level_note_and_basis_refs() {
        let defs = BTreeMap::from([(
            "handled_person_times".to_string(),
            json!({
                "key": "handled_person_times",
                "label": "处理人数",
                "note": "按处理结果ID去重计处理人数。",
                "basis_refs": ["12.问题处理结果清单.xlsx", "处理结果ID"],
                "values": {"value": 1},
                "explain": [
                    {
                        "__kind": "explain_item",
                        "id": "detail",
                        "kind": "detail",
                        "fields": ["处理结果ID"]
                    }
                ]
            }),
        )]);
        let contracts = build_analysis_contracts(&defs, "__world_metrics__");
        let contract = contracts
            .get("handled_person_times")
            .and_then(Value::as_object)
            .expect("contract");
        assert_eq!(
            contract.get("note").and_then(Value::as_str),
            Some("按处理结果ID去重计处理人数。")
        );
        assert_eq!(
            contract
                .get("basis_refs")
                .and_then(Value::as_array)
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            contract.get("tabs").and_then(Value::as_array).map(|tabs| {
                tabs.iter()
                    .filter_map(Value::as_str)
                    .any(|tab| tab == "definition")
            }),
            Some(false)
        );
    }

    #[test]
    fn analysis_closure_metric_ids_skips_auxiliary_association_edges() {
        let graph = AnalysisGraph {
            nodes: BTreeMap::from([
                (
                    "root".to_string(),
                    AnalysisNode {
                        id: "root".to_string(),
                        node_kind: "metric".to_string(),
                        ..Default::default()
                    },
                ),
                (
                    "related".to_string(),
                    AnalysisNode {
                        id: "related".to_string(),
                        node_kind: "metric".to_string(),
                        ..Default::default()
                    },
                ),
            ]),
            edges: vec![AnalysisEdge {
                from: "root".to_string(),
                to: "related".to_string(),
                role: "association".to_string(),
                semantic_kind: SemanticEdgeKind::Association,
            }],
        };

        let closure = analysis_closure_metric_ids(&graph, &["root".to_string()]);
        assert_eq!(
            closure,
            vec!["root".to_string()],
            "auxiliary association edges should not silently expand the default semantic closure"
        );
    }
}
