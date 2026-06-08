use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::model::{AnalysisGraph, AnalysisNode, SemanticEdgeKind, SemanticNodeKind};

use super::{expand::*, util::*};

pub(super) fn is_explain_dataframe_product(item_map: &Map<String, Value>) -> bool {
    item_map.get("__kind").and_then(Value::as_str) == Some("data_product")
        && item_map
            .get("shape")
            .and_then(Value::as_str)
            .is_some_and(|shape| shape.eq_ignore_ascii_case("dataframe"))
}

pub(super) fn fields_from_data_product_schema(item_map: &Map<String, Value>) -> Option<Value> {
    let columns = item_map.get("schema")?.as_array()?;
    let fields: Vec<Value> = columns
        .iter()
        .filter_map(|column| {
            let column_map = column.as_object()?;
            first_non_empty_string(column_map, &["name", "id", "key"]).map(Value::String)
        })
        .collect();
    (!fields.is_empty()).then_some(Value::Array(fields))
}

pub(super) fn explain_block_for_data_product_dataframe(
    item_map: &Map<String, Value>,
    graph: &AnalysisGraph,
    root_dataset_id: &str,
) -> Option<Map<String, Value>> {
    if !is_explain_dataframe_product(item_map) {
        return None;
    }
    let tabular_metric_id = scoped_dataframe_metric_id(item_map)?;
    let block_id = first_non_empty_string(item_map, &["id", "key"])
        .unwrap_or_else(|| tabular_metric_id.clone());
    let mut block = Map::new();
    block.insert("id".to_string(), Value::String(block_id));
    block.insert("kind".to_string(), Value::String("detail".to_string()));
    block.insert(
        "support_role".to_string(),
        Value::String("detail".to_string()),
    );
    copy_field(item_map, &mut block, "label");
    if !block.contains_key("fields") {
        if let Some(fields) = fields_from_data_product_schema(item_map) {
            block.insert("fields".to_string(), fields);
        }
    }
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
    Some(block)
}

pub(super) fn explain_block_value(
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
            scoped_dataframe_metric_id
                .clone()
                .or_else(|| infer_inferred_scalar_rowset_metric_id(graph, metric_id))
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

pub(super) fn collect_data_ref_dataset_ids(value: &Value, out: &mut BTreeSet<String>) {
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

pub(super) fn collect_metric_value_lineage_dataset_ids(raw: &Value) -> BTreeSet<String> {
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

pub(super) fn lineage_dataset_ids_from_graph(graph: &AnalysisGraph, metric_id: &str) -> BTreeSet<String> {
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

pub(super) fn infer_unique_lineage_dataset_id(
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

pub(super) fn unique_lineage_dataset_id_from_metric_values(raw: &Value, support_role: &str) -> Option<String> {
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

pub(super) fn scoped_dataframe_metric_id(item_map: &Map<String, Value>) -> Option<String> {
    first_non_empty_string(
        item_map,
        &["analysis_scoped_id", "analysis_node_id", "id", "key"],
    )
}

pub(super) fn infer_explain_scoped_dataframe(
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

pub(super) fn infer_explain_scoped_dataframe_for_detail(
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

pub(super) fn single_explain_scoped_dataframe(explain_items: &[Value]) -> Option<String> {
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

pub(super) fn explain_has_support_role(explain_items: &[Value], role: &str) -> bool {
    explain_items.iter().any(|item| {
        item.as_object()
            .is_some_and(|map| support_role_for_item(map) == role)
    })
}

pub(super) fn tab_metric_value(block: &Map<String, Value>) -> Option<Map<String, Value>> {
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

pub(super) fn metric_runtime_ref(
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

pub(super) fn metric_target_from_item(item_map: &Map<String, Value>) -> Option<String> {
    if let Some(source) = item_map.get("source").and_then(Value::as_object) {
        if source.get("__ref").and_then(Value::as_str) == Some("metric") {
            return first_non_empty_string(source, &["id"]);
        }
    }
    first_non_empty_string(item_map, &["metric_id", "metricId"])
}

pub(super) fn dataset_target_from_item(item_map: &Map<String, Value>) -> Option<String> {
    if let Some(source) = item_map.get("source").and_then(Value::as_object) {
        if source.get("__ref").and_then(Value::as_str) == Some("data") {
            return first_non_empty_string(source, &["from_dataset", "id"]);
        }
    }
    None
}

pub(super) fn narrative_block_id(parent_metric_id: &str, item_map: &Map<String, Value>) -> String {
    let local_id = first_non_empty_string(item_map, &["id"])
        .unwrap_or_else(|| support_role_for_item(item_map));
    format!("{parent_metric_id}#{}", local_id.trim())
}

pub(super) fn tabular_source_node_id(dataset_id: &str) -> String {
    format!("tabular::{}", dataset_id.trim())
}

pub(super) fn semantic_node_kind_name(kind: SemanticNodeKind) -> &'static str {
    match kind {
        SemanticNodeKind::Metric => "metric",
        SemanticNodeKind::NarrativeSupport => "narrative_support",
        SemanticNodeKind::TabularSource => "tabular_source",
        SemanticNodeKind::Unknown => "unknown",
    }
}

pub(super) fn semantic_edge_kind_from_role(role: &str) -> SemanticEdgeKind {
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

pub(super) fn detect_metric_shape(map: &Map<String, Value>) -> Option<String> {
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

