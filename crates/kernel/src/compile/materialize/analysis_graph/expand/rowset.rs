use super::{scoped_child_metric_id, support_role_for_item, INFERRED_SCALAR_ROWSET_LOCAL_ID};

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::model::AnalysisGraph;

pub(crate) fn explain_needs_tabular_source(items: &[Value]) -> bool {
    items.iter().any(|item| {
        item.as_object().is_some_and(|map| {
            matches!(
                support_role_for_item(map).as_str(),
                "detail" | "trend" | "composition" | "attribution"
            )
        })
    })
}

pub(crate) fn extract_primary_scalar_rowset(metric: &Map<String, Value>) -> Option<Value> {
    let expr = primary_scalar_value_expr(metric)?;
    extract_rowset_from_scalar_expr(&expr)
}

pub(super) fn primary_scalar_value_expr(metric: &Map<String, Value>) -> Option<Value> {
    if let Some(values) = metric.get("values").and_then(Value::as_object) {
        if let Some(value) = values.get("value") {
            return Some(value.clone());
        }
        return values.values().next().cloned();
    }
    metric.get("value").cloned()
}

pub(super) fn extract_rowset_from_scalar_expr(expr: &Value) -> Option<Value> {
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
        "ratio" => extract_rowset_from_ratio_operands(map),
        _ => None,
    }
}

fn extract_rowset_from_ratio_operands(map: &Map<String, Value>) -> Option<Value> {
    let numerator = map
        .get("numerator")
        .and_then(extract_rowset_from_scalar_expr);
    let denominator = map
        .get("denominator")
        .and_then(extract_rowset_from_scalar_expr);
    match (numerator, denominator) {
        (Some(left), Some(right)) if rowset_exprs_equivalent(&left, &right) => Some(left),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        _ => None,
    }
}

fn rowset_exprs_equivalent(left: &Value, right: &Value) -> bool {
    left == right
}

pub(super) fn extract_rowset_from_numeric_source(expr: &Value) -> Option<Value> {
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

pub(super) fn is_rowset_expression(expr: &Value) -> bool {
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

pub(crate) fn inferred_scalar_rowset_metric_id(metric_id: &str) -> String {
    scoped_child_metric_id(metric_id, INFERRED_SCALAR_ROWSET_LOCAL_ID)
}

pub(crate) fn infer_inferred_scalar_rowset_metric_id(
    graph: &AnalysisGraph,
    metric_id: &str,
) -> Option<String> {
    let scoped = inferred_scalar_rowset_metric_id(metric_id);
    graph.nodes.contains_key(&scoped).then_some(scoped)
}

pub(crate) fn infer_inferred_scalar_rowset_metric_id_from_defs(
    metric_id: &str,
    expanded_defs: &BTreeMap<String, Value>,
) -> Option<String> {
    let scoped = inferred_scalar_rowset_metric_id(metric_id);
    expanded_defs.contains_key(&scoped).then_some(scoped)
}
