use serde_json::Value;

use super::drilldown::first_non_empty_string;

pub(super) fn normalize_explain_source(value: &Value) -> Option<Value> {
    let map = value.as_object()?;
    match map.get("__ref").and_then(Value::as_str) {
        Some("metric") => {
            let metric_id = first_non_empty_string(map, &["id"])?;
            let mut source = serde_json::Map::new();
            source.insert("kind".to_string(), Value::String("metric_ref".to_string()));
            source.insert("metric_id".to_string(), Value::String(metric_id));
            if let Some(dataset_id) = first_non_empty_string(map, &["from_dataset"]) {
                source.insert("dataset_id".to_string(), Value::String(dataset_id));
            }
            if let Some(scene_id) = first_non_empty_string(map, &["scene_id"]) {
                source.insert("scene_id".to_string(), Value::String(scene_id));
            }
            if let Some(scene_file) = first_non_empty_string(map, &["scene_file"]) {
                source.insert("scene_file".to_string(), Value::String(scene_file));
            }
            Some(Value::Object(source))
        }
        Some("dataset") | Some("data") => {
            let dataset_id = first_non_empty_string(map, &["id"])?;
            let mut source = serde_json::Map::new();
            source.insert("kind".to_string(), Value::String("dataset_ref".to_string()));
            source.insert("dataset_id".to_string(), Value::String(dataset_id));
            if let Some(scene_id) = first_non_empty_string(map, &["scene_id"]) {
                source.insert("scene_id".to_string(), Value::String(scene_id));
            }
            if let Some(scene_file) = first_non_empty_string(map, &["scene_file"]) {
                source.insert("scene_file".to_string(), Value::String(scene_file));
            }
            Some(Value::Object(source))
        }
        _ => Some(Value::Object(map.clone())),
    }
}

pub(super) fn table_metric_id_from_source(value: &Value) -> Option<String> {
    let map = value.as_object()?;
    first_non_empty_string(map, &["table_metric_id", "metric_id"])
}

pub(super) fn dataset_id_from_source(value: &Value) -> Option<String> {
    let map = value.as_object()?;
    first_non_empty_string(map, &["dataset_id"])
}

pub(super) fn normalize_explain_entry_object(obj: &serde_json::Map<String, Value>) -> Option<Value> {
    let raw_kind = first_non_empty_string(obj, &["kind", "type", "id"])?;
    let kind = normalize_analysis_tab_id(&raw_kind)?;
    let id = first_non_empty_string(obj, &["id", "key", "name"])
        .and_then(|raw| normalize_analysis_tab_id(raw.as_str()))
        .unwrap_or_else(|| kind.clone());
    let mut entry = obj.clone();
    entry.insert("id".to_string(), Value::String(id));
    entry.insert("kind".to_string(), Value::String(kind.clone()));
    if let Some(source) = obj.get("source").and_then(normalize_explain_source) {
        entry.insert("source".to_string(), source.clone());
        if !entry.contains_key("table_metric_id") {
            if let Some(metric_id) = table_metric_id_from_source(&source) {
                entry.insert("table_metric_id".to_string(), Value::String(metric_id));
            }
        }
        if !entry.contains_key("dataset_id") {
            if let Some(dataset_id) = dataset_id_from_source(&source) {
                entry.insert("dataset_id".to_string(), Value::String(dataset_id));
            }
        }
    }
    entry.insert("support_role".to_string(), Value::String(kind));
    Some(Value::Object(entry))
}

pub(super) fn normalize_analysis_node_object(obj: &serde_json::Map<String, Value>) -> Option<Value> {
    let local_id = first_non_empty_string(obj, &["analysis_local_id", "key", "id"])?;
    let scoped_metric_id = first_non_empty_string(
        obj,
        &["analysis_scoped_id", "analysis_node_id", "key", "id"],
    )?;
    let mut node = serde_json::Map::new();
    node.insert("id".to_string(), Value::String(local_id));
    node.insert("metric_id".to_string(), Value::String(scoped_metric_id));
    node.insert("node_kind".to_string(), Value::String("metric".to_string()));
    if let Some(shape) = first_non_empty_string(obj, &["shape"]) {
        node.insert("shape".to_string(), Value::String(shape));
    }
    if let Some(label) = first_non_empty_string(obj, &["label"]) {
        node.insert("label".to_string(), Value::String(label));
    }
    if let Some(parent_metric_id) = first_non_empty_string(obj, &["analysis_parent_metric_id"]) {
        node.insert(
            "parent_metric_id".to_string(),
            Value::String(parent_metric_id),
        );
    }
    node.insert(
        "can_explain".to_string(),
        Value::Bool(
            obj.get("explain")
                .and_then(Value::as_array)
                .is_some_and(|items| !items.is_empty()),
        ),
    );
    Some(Value::Object(node))
}

pub(super) fn explain_metric_entries_from_value(value: &Value) -> Vec<Value> {
    let mut entries: Vec<Value> = Vec::new();
    if let Some(items) = value.as_array() {
        for item in items {
            let Some(map) = item.as_object() else {
                continue;
            };
            let id = first_non_empty_string(map, &["id", "key", "name"])
                .and_then(|raw| normalize_analysis_tab_id(raw.as_str()));
            let kind = first_non_empty_string(map, &["kind", "type"])
                .and_then(|raw| normalize_analysis_tab_id(raw.as_str()));
            let Some(metric_id) = id.or(kind) else {
                continue;
            };
            let mut entry = map.clone();
            entry.insert("id".to_string(), Value::String(metric_id));
            entries.push(Value::Object(entry));
        }
        return entries;
    }
    if let Some(map) = value.as_object() {
        for (key, item) in map {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let mut entry = obj.clone();
            let id = first_non_empty_string(obj, &["id"])
                .and_then(|raw| normalize_analysis_tab_id(raw.as_str()))
                .or_else(|| normalize_analysis_tab_id(key))
                .unwrap_or_else(|| key.trim().to_string());
            entry.insert("id".to_string(), Value::String(id));
            entries.push(Value::Object(entry));
        }
    }
    entries
}

pub(super) fn normalize_analysis_tab_id(value: &str) -> Option<String> {
    let raw = value.trim().to_lowercase();
    if raw.is_empty() {
        return None;
    }
    let tab = match raw.as_str() {
        "definition" | "def" | "metric_definition" | "metric-definition" => "definition",
        "composition" | "breakdown" | "group" | "group_by" | "groupby" => "composition",
        "trend" | "timeseries" | "time_series" | "time-series" | "trend_compare" => "trend",
        "numerator_denominator" | "numerator-denominator" | "numerator" | "ratio" => {
            "numerator_denominator"
        }
        "attribution" | "reason" => "attribution",
        "detail" | "details" => "detail",
        _ => raw.as_str(),
    };
    Some(tab.to_string())
}
