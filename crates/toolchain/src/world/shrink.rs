use std::collections::BTreeMap;

use mei_lang_kernel::ResourceDecl;
use serde_json::{json, Value};

use super::bundle::normalize_path;

pub(crate) fn json_serialized_len(value: &Value) -> usize {
    serde_json::to_string(value).map(|s| s.len()).unwrap_or(0)
}

pub(crate) fn shrink_json_for_llm(value: &Value, max_total: usize) -> Value {
    let len = json_serialized_len(value);
    if len <= max_total {
        return value.clone();
    }
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, item) in map.iter().take(48) {
                let entry_len = json_serialized_len(item);
                if entry_len > 2_000 {
                    out.insert(
                        key.clone(),
                        json!({
                            "_omitted": true,
                            "approx_chars": entry_len,
                        }),
                    );
                } else {
                    out.insert(key.clone(), item.clone());
                }
            }
            out.insert(
                "_truncated".to_string(),
                json!({
                    "reason": "payload too large for tool output",
                    "approx_original_chars": len,
                }),
            );
            Value::Object(out)
        }
        Value::Array(items) => json!({
            "type": "array",
            "len": items.len(),
            "head": items.iter().take(5).cloned().collect::<Vec<_>>(),
        }),
        Value::String(text) => {
            let cap = 1_000usize;
            if text.len() <= cap {
                Value::String(text.clone())
            } else {
                Value::String(format!("{}…", text.chars().take(cap).collect::<String>()))
            }
        }
        other => other.clone(),
    }
}

fn extract_dataset_schema_preview(dataset: &Value) -> Option<Value> {
    let columns = dataset.get("columns")?.as_array()?;
    const MAX_COLS: usize = 72;
    let mut preview = Vec::new();
    for column in columns.iter().take(MAX_COLS) {
        let Some(map) = column.as_object() else {
            continue;
        };
        let name = map.get("name").and_then(Value::as_str).unwrap_or("?");
        let ty = map
            .get("type")
            .or_else(|| map.get("type_name"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        let mut row = serde_json::Map::new();
        row.insert("name".to_string(), json!(name));
        row.insert("type".to_string(), json!(ty));
        if let Some(source) = map.get("source").and_then(Value::as_str) {
            row.insert("source".to_string(), json!(source));
        }
        if map
            .get("optional")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            row.insert("optional".to_string(), json!(true));
        }
        preview.push(Value::Object(row));
    }
    Some(json!({
        "column_count": columns.len(),
        "columns_preview": preview,
        "columns_preview_truncated": columns.len() > MAX_COLS,
    }))
}

fn summarize_dataset_decl(dataset: &Value) -> Value {
    let len = json_serialized_len(dataset);
    let schema = extract_dataset_schema_preview(dataset);
    match dataset {
        Value::Object(map) => {
            let keys: Vec<&str> = map.keys().map(String::as_str).take(32).collect();
            let kind = map.get("kind").and_then(Value::as_str);
            let key = map.get("key").and_then(Value::as_str);
            let normalize_n = map
                .get("normalize")
                .and_then(Value::as_object)
                .map(|object| object.len())
                .unwrap_or(0);
            json!({
                "present": true,
                "approx_decl_chars": len,
                "kind": kind,
                "key": key,
                "top_level_keys_sample": keys,
                "top_level_key_count": map.len(),
                "normalize_field_count": normalize_n,
                "schema": schema,
                "note": "full dataset body omitted; `schema.columns_preview` lists declared columns (bounded)."
            })
        }
        _ => json!({
            "present": true,
            "approx_decl_chars": len,
            "note": "dataset value is non-object; omitted for size."
        }),
    }
}

fn summarize_filters_decl(filters: &Value) -> Value {
    let len = json_serialized_len(filters);
    if len <= 1_200 {
        return filters.clone();
    }
    match filters {
        Value::Object(map) => json!({
            "object_key_count": map.len(),
            "keys": map.keys().take(40).cloned().collect::<Vec<_>>(),
            "approx_chars": len,
            "note": "filters object truncated to keys only.",
        }),
        _ => json!({
            "approx_chars": len,
            "note": "filters omitted (too large).",
        }),
    }
}

fn summarize_metrics_decl(metrics: &BTreeMap<String, Value>) -> Value {
    let keys: Vec<&str> = metrics.keys().map(String::as_str).take(48).collect();
    json!({
        "metric_ids_sample": keys,
        "metric_id_count": metrics.len(),
        "note": "metric bodies omitted; ids are enough to reason about bindings before read_file.",
    })
}

pub(crate) fn summarize_resource_decl(item: &ResourceDecl) -> Value {
    let content_note = item.content.as_ref().map(|content| {
        if content.len() <= 800 {
            json!(content.as_str())
        } else {
            json!({
                "prefix": content.chars().take(400).collect::<String>(),
                "truncated_chars": content.len().saturating_sub(400),
            })
        }
    });
    json!({
        "_payload_shape": "resource_summary_v1",
        "id": item.id,
        "kind": item.kind,
        "title": item.title,
        "purpose": item.purpose,
        "source": item.source.as_ref().map(|source| json!({ "path": normalize_path(&source.path) })).unwrap_or(Value::Null),
        "dataset": item.dataset.as_ref().map_or(json!({ "present": false }), summarize_dataset_decl),
        "metrics": item.metrics.as_ref().map_or(Value::Null, summarize_metrics_decl),
        "filters": item.filters.as_ref().map_or(Value::Null, summarize_filters_decl),
        "content": content_note.unwrap_or(Value::Null),
    })
}
