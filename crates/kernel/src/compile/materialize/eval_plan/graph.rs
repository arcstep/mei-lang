use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::compile::analysis::eval_context::RuntimeMetricEvalScope;
use crate::model::DatasetView;

use super::fingerprint::{
    analysis_expr_plan_kind, expr_fingerprint, expr_plan_node_id, hydrate_plan_node_id,
    metric_plan_node_id,
};
use super::types::{
    EvalPlan, EvalPlanEdge, EvalPlanEdgeKind, EvalPlanNode, EvalPlanNodeKind, EvalPlanScope,
};

pub(crate) fn build_eval_plan(
    metric_defs: &BTreeMap<String, Value>,
    metric_ids: Option<&[String]>,
    datasets: &BTreeMap<String, DatasetView>,
    scope: &RuntimeMetricEvalScope,
) -> EvalPlan {
    let targets = selected_metric_ids(metric_defs, metric_ids);
    let mut nodes = BTreeMap::<String, EvalPlanNode>::new();
    let mut edges = BTreeSet::<(String, String, EvalPlanEdgeKind)>::new();
    for metric_id in &targets {
        let metric_node_id = metric_plan_node_id(metric_id);
        nodes.entry(metric_node_id.clone()).or_insert(EvalPlanNode {
            id: metric_node_id.clone(),
            kind: EvalPlanNodeKind::MetricEval,
            metric_id: Some(metric_id.clone()),
            dataset_id: None,
            expr_fingerprint: None,
            label: Some(metric_id.clone()),
        });
        let Some(raw) = metric_defs.get(metric_id).and_then(Value::as_object) else {
            continue;
        };
        if let Some(dataset_id) = first_non_empty_string(raw, &["dataset", "dataset_id"]) {
            add_hydrate_dependency(
                &metric_node_id,
                &dataset_id,
                scope,
                datasets,
                &mut nodes,
                &mut edges,
            );
        }
        if let Some(values) = raw.get("values").and_then(Value::as_object) {
            for (field, expr) in values {
                visit_expr(
                    expr,
                    &metric_node_id,
                    Some(metric_id.as_str()),
                    Some(field.as_str()),
                    scope,
                    datasets,
                    &mut nodes,
                    &mut edges,
                );
            }
        } else if let Some(expr) = raw
            .get("series")
            .or_else(|| raw.get("list"))
            .or_else(|| raw.get("value"))
        {
            visit_expr(
                expr,
                &metric_node_id,
                Some(metric_id.as_str()),
                None,
                scope,
                datasets,
                &mut nodes,
                &mut edges,
            );
        }
    }
    EvalPlan {
        scope: EvalPlanScope::from(scope),
        targets,
        nodes,
        edges: edges
            .into_iter()
            .map(|(from, to, kind)| EvalPlanEdge { from, to, kind })
            .collect(),
    }
}
fn selected_metric_ids(
    metric_defs: &BTreeMap<String, Value>,
    metric_ids: Option<&[String]>,
) -> Vec<String> {
    if let Some(ids) = metric_ids {
        let selected = ids
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .filter(|metric_id| metric_defs.contains_key(*metric_id))
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if !selected.is_empty() {
            return selected;
        }
    }
    metric_defs.keys().cloned().collect()
}

fn visit_expr(
    expr: &Value,
    parent_node_id: &str,
    metric_id: Option<&str>,
    field_label: Option<&str>,
    scope: &RuntimeMetricEvalScope,
    datasets: &BTreeMap<String, DatasetView>,
    nodes: &mut BTreeMap<String, EvalPlanNode>,
    edges: &mut BTreeSet<(String, String, EvalPlanEdgeKind)>,
) {
    match expr {
        Value::Array(items) => {
            for item in items {
                visit_expr(
                    item,
                    parent_node_id,
                    metric_id,
                    field_label,
                    scope,
                    datasets,
                    nodes,
                    edges,
                );
            }
        }
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                let dataset_id = first_non_empty_string(map, &["from_dataset", "id"]);
                let node_id = expr_plan_node_id(EvalPlanNodeKind::Rowset, expr);
                nodes.entry(node_id.clone()).or_insert(EvalPlanNode {
                    id: node_id.clone(),
                    kind: EvalPlanNodeKind::Rowset,
                    metric_id: metric_id.map(str::to_string),
                    dataset_id: dataset_id.clone(),
                    expr_fingerprint: Some(expr_fingerprint(expr)),
                    label: dataset_id
                        .clone()
                        .or_else(|| field_label.map(str::to_string)),
                });
                edges.insert((
                    parent_node_id.to_string(),
                    node_id.clone(),
                    EvalPlanEdgeKind::DependsOn,
                ));
                if let Some(dataset_id) = dataset_id {
                    add_hydrate_dependency(&node_id, &dataset_id, scope, datasets, nodes, edges);
                }
                return;
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
                let plan_kind = analysis_expr_plan_kind(
                    map.get("type").and_then(Value::as_str).unwrap_or_default(),
                );
                let node_id = expr_plan_node_id(plan_kind, expr);
                nodes.entry(node_id.clone()).or_insert(EvalPlanNode {
                    id: node_id.clone(),
                    kind: plan_kind,
                    metric_id: metric_id.map(str::to_string),
                    dataset_id: None,
                    expr_fingerprint: Some(expr_fingerprint(expr)),
                    label: map
                        .get("type")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .or_else(|| field_label.map(str::to_string)),
                });
                edges.insert((
                    parent_node_id.to_string(),
                    node_id.clone(),
                    EvalPlanEdgeKind::DependsOn,
                ));
                if let Some(dataset_id) = dataset_dependency_from_analysis_expr(map) {
                    if let Some(node) = nodes.get_mut(&node_id) {
                        if node.dataset_id.is_none() {
                            node.dataset_id = Some(dataset_id.clone());
                        }
                    }
                    add_hydrate_dependency(&node_id, &dataset_id, scope, datasets, nodes, edges);
                }
                for child in map.values() {
                    visit_expr(
                        child, &node_id, metric_id, None, scope, datasets, nodes, edges,
                    );
                }
                return;
            }
            for child in map.values() {
                visit_expr(
                    child,
                    parent_node_id,
                    metric_id,
                    field_label,
                    scope,
                    datasets,
                    nodes,
                    edges,
                );
            }
        }
        _ => {}
    }
}

fn add_hydrate_dependency(
    parent_node_id: &str,
    dataset_id: &str,
    scope: &RuntimeMetricEvalScope,
    datasets: &BTreeMap<String, DatasetView>,
    nodes: &mut BTreeMap<String, EvalPlanNode>,
    edges: &mut BTreeSet<(String, String, EvalPlanEdgeKind)>,
) {
    if !dataset_requires_hydration(dataset_id, scope, datasets) {
        return;
    }
    let hydrate_node_id = hydrate_plan_node_id(dataset_id);
    nodes
        .entry(hydrate_node_id.clone())
        .or_insert(EvalPlanNode {
            id: hydrate_node_id.clone(),
            kind: EvalPlanNodeKind::Hydrate,
            metric_id: None,
            dataset_id: Some(dataset_id.to_string()),
            expr_fingerprint: None,
            label: Some(dataset_id.to_string()),
        });
    edges.insert((
        parent_node_id.to_string(),
        hydrate_node_id,
        EvalPlanEdgeKind::Hydrates,
    ));
}

fn dataset_dependency_from_analysis_expr(map: &serde_json::Map<String, Value>) -> Option<String> {
    if map.get("type").and_then(Value::as_str) == Some("rows") {
        return first_non_empty_string(map, &["dataset"]);
    }
    None
}

fn dataset_requires_hydration(
    dataset_id: &str,
    scope: &RuntimeMetricEvalScope,
    datasets: &BTreeMap<String, DatasetView>,
) -> bool {
    let normalized = dataset_id.trim();
    if normalized.is_empty() || normalized == scope.base_dataset_id.trim() {
        return false;
    }
    let Some(dataset) = lookup_dataset_view(datasets, normalized) else {
        return false;
    };
    let path = dataset.source.path.trim();
    if path.is_empty() || path.starts_with("dataset_view:") {
        return false;
    }
    let kind = dataset.source.kind.trim();
    if kind.eq_ignore_ascii_case("derived") || kind.eq_ignore_ascii_case("world_metrics") {
        return false;
    }
    true
}

fn lookup_dataset_view<'a>(
    datasets: &'a BTreeMap<String, DatasetView>,
    dataset_id: &str,
) -> Option<&'a DatasetView> {
    let normalized = dataset_id.strip_prefix("dataset.").unwrap_or(dataset_id);
    datasets
        .get(normalized)
        .or_else(|| datasets.get(dataset_id))
        .or_else(|| {
            datasets.iter().find_map(|(key, dataset)| {
                (dataset.id == normalized
                    || key.ends_with(&format!("::{normalized}"))
                    || key.ends_with(&format!("/{normalized}")))
                .then_some(dataset)
            })
        })
        .or_else(|| {
            super::super::world_metrics::local_dataset_id_from_namespaced_token(normalized)
                .and_then(|local| lookup_dataset_view(datasets, local))
        })
}

fn first_non_empty_string(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        map.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}
