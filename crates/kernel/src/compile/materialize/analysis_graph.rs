use std::collections::BTreeMap;

use serde_json::Value;

pub(crate) fn expand_runtime_metric_defs(metric_defs: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    let mut expanded = BTreeMap::new();
    for (metric_id, raw) in metric_defs {
        expand_metric_def(metric_id, raw, &mut expanded);
    }
    expanded
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
