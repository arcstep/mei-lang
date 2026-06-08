use mei_lang_kernel::DatasetView;
use serde_json::{json, Value};

pub(crate) const DATASET_QUERY_MAX_CELL_CHARS: usize = 50;
pub(crate) const DATASET_QUERY_TOTAL_CHAR_BUDGET: usize = 12_000;

fn truncate_text_chars(input: &str, max_chars: usize) -> (String, bool) {
    if input.chars().count() <= max_chars {
        return (input.to_string(), false);
    }
    let mut out = input.chars().take(max_chars).collect::<String>();
    out.push('…');
    (out, true)
}

pub(crate) fn bounded_cell_value(value: &Value, truncated_cells: &mut usize) -> Value {
    match value {
        Value::String(s) => {
            let (text, changed) = truncate_text_chars(s, DATASET_QUERY_MAX_CELL_CHARS);
            if changed {
                *truncated_cells += 1;
            }
            Value::String(text)
        }
        Value::Array(_) | Value::Object(_) => {
            let raw = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
            let (text, changed) = truncate_text_chars(&raw, DATASET_QUERY_MAX_CELL_CHARS);
            if changed {
                *truncated_cells += 1;
            }
            Value::String(text)
        }
        other => other.clone(),
    }
}

pub(crate) fn project_dataset_row(
    row: &Value,
    selected_columns: &[String],
    truncated_cells: &mut usize,
) -> Value {
    let mut out = serde_json::Map::new();
    if let Some(obj) = row.as_object() {
        for col in selected_columns {
            let value = obj
                .get(col)
                .map(|v| bounded_cell_value(v, truncated_cells))
                .unwrap_or(Value::Null);
            out.insert(col.clone(), value);
        }
        return Value::Object(out);
    }
    out.insert("_raw".to_string(), bounded_cell_value(row, truncated_cells));
    Value::Object(out)
}

pub(crate) fn build_schema_preview(dataset: &DatasetView, selected_columns: &[String]) -> Vec<Value> {
    use std::collections::BTreeMap;

    let schema_map = dataset
        .schema
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect::<BTreeMap<_, _>>();
    selected_columns
        .iter()
        .map(|name| {
            if let Some(col) = schema_map.get(name.as_str()) {
                json!({
                    "name": col.name,
                    "type": col.type_name,
                    "source": col.source,
                    "optional": col.optional,
                })
            } else {
                json!({
                    "name": name,
                    "type": "unknown",
                })
            }
        })
        .collect()
}

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

pub(crate) fn summarize_filters_decl(filters: &Value) -> Value {
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
