use super::{child_metric_local_id, scoped_child_metric_id, support_role_for_item};

use std::collections::BTreeMap;

use serde_json::Value;


pub(crate) fn rewrite_explain_scope(metric_id: &str, value: &Value) -> Value {
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

pub(super) fn scope_local_metric_ids(metric_id: &str, items: &[Value]) -> BTreeMap<String, String> {
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

pub(super) fn rewrite_scope_item(
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

pub(super) fn rewrite_local_metric_refs(
    value: &Value,
    local_ids: &BTreeMap<String, String>,
) -> Value {
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
