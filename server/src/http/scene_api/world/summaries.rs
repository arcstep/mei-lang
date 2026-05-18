use std::collections::BTreeMap;

use mei_lang_kernel::ResourceDecl;
use serde_json::{json, Value};

use super::json_shrink::json_serialized_len;
use super::util::normalize_path;

/// 从已物化的 `dataset` JSON 中提取列名/类型（有界），避免模型为「有哪些字段」再去 read_file `.mei`。
fn extract_dataset_schema_preview(dataset: &Value) -> Option<Value> {
    let cols = dataset.get("columns")?.as_array()?;
    const MAX_COLS: usize = 72;
    let mut preview = Vec::new();
    for c in cols.iter().take(MAX_COLS) {
        let Some(co) = c.as_object() else {
            continue;
        };
        let name = co.get("name").and_then(Value::as_str).unwrap_or("?");
        let typ = co
            .get("type")
            .or_else(|| co.get("type_name"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        let mut row = serde_json::Map::new();
        row.insert("name".to_string(), json!(name));
        row.insert("type".to_string(), json!(typ));
        if let Some(s) = co.get("source").and_then(Value::as_str) {
            row.insert("source".to_string(), json!(s));
        }
        if let Some(o) = co.get("optional").and_then(Value::as_bool) {
            if o {
                row.insert("optional".to_string(), json!(true));
            }
        }
        preview.push(Value::Object(row));
    }
    Some(json!({
        "column_count": cols.len(),
        "columns_preview": preview,
        "columns_preview_truncated": cols.len() > MAX_COLS,
    }))
}

fn summarize_dataset_decl(dataset: &Value) -> Value {
    let len = json_serialized_len(dataset);
    let schema = extract_dataset_schema_preview(dataset);
    match dataset {
        Value::Object(m) => {
            let keys: Vec<&str> = m.keys().map(String::as_str).take(32).collect();
            let kind = m.get("kind").and_then(Value::as_str);
            let key = m.get("key").and_then(Value::as_str);
            let normalize_n = m
                .get("normalize")
                .and_then(Value::as_object)
                .map(|o| o.len())
                .unwrap_or(0);
            json!({
                "present": true,
                "approx_decl_chars": len,
                "kind": kind,
                "key": key,
                "top_level_keys_sample": keys,
                "top_level_key_count": m.len(),
                "normalize_field_count": normalize_n,
                "schema": schema,
                "note": "full dataset body omitted; `schema.columns_preview` lists declared columns (bounded). Use read_file on the entry `.mei` only when the user needs exact DSL quotes or edits — not for routine field lists."
            })
        }
        _ => json!({
            "present": true,
            "approx_decl_chars": len,
            "note": "dataset value is non-object; omitted for size."
        }),
    }
}

pub(super) fn summarize_filters_decl(filters: &Value) -> Value {
    let len = json_serialized_len(filters);
    if len <= 1_200 {
        return filters.clone();
    }
    match filters {
        Value::Object(m) => {
            let keys: Vec<&str> = m.keys().map(String::as_str).collect();
            json!({
                "object_key_count": keys.len(),
                "keys": keys.iter().take(40).copied().collect::<Vec<_>>(),
                "approx_chars": len,
                "note": "filters object truncated to keys only."
            })
        }
        _ => json!({ "approx_chars": len, "note": "filters omitted (too large)." }),
    }
}

fn summarize_metrics_decl(metrics: &BTreeMap<String, Value>) -> Value {
    let count = metrics.len();
    let keys: Vec<&str> = metrics.keys().map(String::as_str).take(48).collect();
    json!({
        "metric_ids_sample": keys,
        "metric_id_count": count,
        "note": "metric bodies omitted; ids are enough to reason about bindings before read_file."
    })
}

/// 供 `resource_get` 与 HTTP API 使用：避免把 dataset / metrics 等大 JSON 原样塞进模型上下文。
pub(crate) fn summarize_resource_decl(item: &ResourceDecl) -> Value {
    let content_note = item.content.as_ref().map(|c| {
        if c.len() <= 800 {
            json!(c.as_str())
        } else {
            json!({
                "prefix": c.chars().take(400).collect::<String>(),
                "truncated_chars": c.len().saturating_sub(400),
            })
        }
    });
    json!({
        "_payload_shape": "resource_summary_v1",
        "id": item.id,
        "kind": item.kind,
        "title": item.title,
        "purpose": item.purpose,
        "source": item.source.as_ref().map(|s| json!({ "path": normalize_path(&s.path) })).unwrap_or(Value::Null),
        "dataset": item.dataset.as_ref().map_or(json!({ "present": false }), summarize_dataset_decl),
        "metrics": item.metrics.as_ref().map_or(Value::Null, summarize_metrics_decl),
        "filters": item.filters.as_ref().map_or(Value::Null, summarize_filters_decl),
        "content": content_note.unwrap_or(Value::Null),
    })
}
