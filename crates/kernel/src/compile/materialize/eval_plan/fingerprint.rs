use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

use serde_json::Value;

use super::types::EvalPlanNodeKind;

pub(super) fn metric_plan_node_id(metric_id: &str) -> String {
    format!("metric:{metric_id}")
}

pub(super) fn hydrate_plan_node_id(dataset_id: &str) -> String {
    format!("hydrate:{}", dataset_id.trim())
}

pub(super) fn expr_plan_node_id(kind: EvalPlanNodeKind, expr: &Value) -> String {
    let prefix = match kind {
        EvalPlanNodeKind::Rowset => "rowset",
        EvalPlanNodeKind::ScalarExpr => "scalar",
        EvalPlanNodeKind::MetricEval => "metric",
        EvalPlanNodeKind::Hydrate => "hydrate",
        EvalPlanNodeKind::Unknown => "expr",
    };
    format!("{prefix}:{}", expr_fingerprint(expr))
}

pub(super) fn expr_fingerprint(expr: &Value) -> String {
    let serialized = serde_json::to_string(&canonicalize_expr_value(expr)).unwrap_or_default();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serialized.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn canonicalize_expr_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_expr_value).collect()),
        Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            for key in map.keys().cloned().collect::<BTreeSet<_>>() {
                if let Some(item) = map.get(&key) {
                    sorted.insert(key, canonicalize_expr_value(item));
                }
            }
            Value::Object(sorted)
        }
        _ => value.clone(),
    }
}

pub(super) fn analysis_expr_plan_kind(analysis_type: &str) -> EvalPlanNodeKind {
    match analysis_type.trim() {
        "count" | "sum" | "avg" | "min" | "max" | "median" | "unique_count" | "item_count"
        | "ratio" | "percent" | "sum_first_number" | "sum_rowset_counts" | "number" | "lit"
        | "mom" | "yoy" | "change_rate" => EvalPlanNodeKind::ScalarExpr,
        _ => EvalPlanNodeKind::Rowset,
    }
}
