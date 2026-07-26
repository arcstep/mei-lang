use super::{
    child_metric_local_id, explain_needs_tabular_source, extract_primary_scalar_rowset,
    rewrite_explain_scope, scoped_child_metric_id, support_role_for_item,
    INFERRED_SCALAR_ROWSET_LOCAL_ID,
};

use std::collections::BTreeMap;

use serde_json::{Map, Value};

pub(crate) fn expand_metric_def(metric_id: &str, raw: &Value, out: &mut BTreeMap<String, Value>) {
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
    maybe_hoist_composition_dataframes(metric_id, &normalized, out);
    maybe_hoist_trend_dataframes(metric_id, &normalized, out);
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

pub(super) fn maybe_hoist_composition_dataframes(
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
    let scalar_rowset_id = scoped_child_metric_id(metric_id, INFERRED_SCALAR_ROWSET_LOCAL_ID);
    let rowset_ref = serde_json::json!({"__ref": "metric", "id": scalar_rowset_id});
    for item in items {
        let Some(item_map) = item.as_object() else {
            continue;
        };
        if item_map.get("__kind").and_then(Value::as_str) == Some("data_product") {
            continue;
        }
        if support_role_for_item(item_map) != "composition" {
            continue;
        }
        let Some(local_id) = item_map
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let scoped_id = scoped_child_metric_id(metric_id, local_id);
        if out.contains_key(&scoped_id) {
            continue;
        }
        let by_field = item_map
            .get("by")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                item_map
                    .get("fields")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            });
        let Some(by_field) = by_field else {
            continue;
        };
        // Optional multi-value membership: explode `by` by delimiter before group_by
        // (e.g. 风险等级 "蓝/黄/红" → count toward 蓝, 黄, and 红 separately).
        let delimiter = item_map
            .get("delimiter")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let group_rowset = if let Some(delimiter) = delimiter {
            let mut split_expr = Map::new();
            split_expr.insert(
                "__kind".to_string(),
                Value::String("analysis_expr".to_string()),
            );
            split_expr.insert("type".to_string(), Value::String("split_text".to_string()));
            split_expr.insert("rowset".to_string(), rowset_ref.clone());
            split_expr.insert("field".to_string(), Value::String(by_field.to_string()));
            split_expr.insert("delimiter".to_string(), Value::String(delimiter.to_string()));
            // Composition membership: drop rows with no non-empty parts (e.g. raw "/").
            split_expr.insert("on_empty".to_string(), Value::String("drop".to_string()));
            Value::Object(split_expr)
        } else {
            rowset_ref.clone()
        };
        let mut group_expr = Map::new();
        group_expr.insert(
            "__kind".to_string(),
            Value::String("analysis_expr".to_string()),
        );
        group_expr.insert("type".to_string(), Value::String("group_by".to_string()));
        group_expr.insert("rowset".to_string(), group_rowset);
        group_expr.insert("by".to_string(), Value::String(by_field.to_string()));
        group_expr.insert(
            "agg".to_string(),
            Value::String(
                item_map
                    .get("agg")
                    .and_then(Value::as_str)
                    .unwrap_or("count")
                    .to_string(),
            ),
        );
        if let Some(value_field) = item_map
            .get("value_field")
            .or_else(|| item_map.get("value"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            group_expr.insert("value".to_string(), Value::String(value_field.to_string()));
        }
        if let Some(limit) = item_map
            .get("top_n")
            .or_else(|| item_map.get("limit"))
            .and_then(Value::as_u64)
        {
            group_expr.insert("limit".to_string(), Value::from(limit));
        }
        let mut child_metric = Map::new();
        child_metric.insert(
            "__kind".to_string(),
            Value::String("data_product".to_string()),
        );
        child_metric.insert("id".to_string(), Value::String(local_id.to_string()));
        child_metric.insert("shape".to_string(), Value::String("dataframe".to_string()));
        child_metric.insert("value".to_string(), Value::Object(group_expr));
        child_metric.insert(
            "schema".to_string(),
            Value::Array(vec![
                serde_json::json!({"name": by_field, "type": "string"}),
                serde_json::json!({"name": "value", "type": "number"}),
            ]),
        );
        child_metric.insert(
            "analysis_inferred_composition".to_string(),
            Value::Bool(true),
        );
        child_metric.insert("key".to_string(), Value::String(scoped_id.clone()));
        child_metric.insert(
            "analysis_local_id".to_string(),
            Value::String(local_id.to_string()),
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
}

pub(super) fn maybe_hoist_trend_dataframes(
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
    let scalar_rowset_id = scoped_child_metric_id(metric_id, INFERRED_SCALAR_ROWSET_LOCAL_ID);
    let rowset_ref = serde_json::json!({"__ref": "metric", "id": scalar_rowset_id});
    for item in items {
        let Some(item_map) = item.as_object() else {
            continue;
        };
        if item_map.get("__kind").and_then(Value::as_str) == Some("data_product") {
            continue;
        }
        if support_role_for_item(item_map) != "trend" {
            continue;
        }
        let Some(local_id) = item_map
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let scoped_id = scoped_child_metric_id(metric_id, local_id);
        if out.contains_key(&scoped_id) {
            continue;
        }
        let Some(date_field) = item_map
            .get("date_field")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let by = item_map
            .get("grain")
            .or_else(|| item_map.get("by"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("month");
        let mut trend_expr = Map::new();
        trend_expr.insert(
            "__kind".to_string(),
            Value::String("analysis_expr".to_string()),
        );
        trend_expr.insert("type".to_string(), Value::String("trend".to_string()));
        trend_expr.insert("rowset".to_string(), rowset_ref.clone());
        trend_expr.insert(
            "date_field".to_string(),
            Value::String(date_field.to_string()),
        );
        trend_expr.insert("by".to_string(), Value::String(by.to_string()));
        trend_expr.insert(
            "agg".to_string(),
            Value::String(
                item_map
                    .get("agg")
                    .and_then(Value::as_str)
                    .unwrap_or("count")
                    .to_string(),
            ),
        );
        if let Some(value_field) = item_map
            .get("value_field")
            .or_else(|| item_map.get("value"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            trend_expr.insert("value".to_string(), Value::String(value_field.to_string()));
        }
        if let Some(limit) = item_map
            .get("top_n")
            .or_else(|| item_map.get("limit"))
            .and_then(Value::as_u64)
        {
            trend_expr.insert("limit".to_string(), Value::from(limit));
        }
        let label_field = if by == "month" { "month" } else { by };
        let mut child_metric = Map::new();
        child_metric.insert(
            "__kind".to_string(),
            Value::String("data_product".to_string()),
        );
        child_metric.insert("id".to_string(), Value::String(local_id.to_string()));
        child_metric.insert("shape".to_string(), Value::String("dataframe".to_string()));
        child_metric.insert("value".to_string(), Value::Object(trend_expr));
        child_metric.insert(
            "schema".to_string(),
            Value::Array(vec![
                serde_json::json!({"name": label_field, "type": "string"}),
                serde_json::json!({"name": "value", "type": "number"}),
            ]),
        );
        child_metric.insert("analysis_inferred_trend".to_string(), Value::Bool(true));
        child_metric.insert("key".to_string(), Value::String(scoped_id.clone()));
        child_metric.insert(
            "analysis_local_id".to_string(),
            Value::String(local_id.to_string()),
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
}

pub(super) fn maybe_hoist_inferred_scalar_rowset(
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
