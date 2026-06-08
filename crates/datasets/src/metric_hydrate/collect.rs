use std::collections::BTreeSet;

use serde_json::Value;

pub(crate) fn collect_dataset_ids_from_values(values: &[Value]) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for value in values {
        collect_dataset_ids_from_value(value, &mut ids);
    }
    ids
}

pub(crate) fn collect_dataset_ids_from_value(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_dataset_ids_from_value(item, out);
            }
        }
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("data") {
                if let Some(id) = map
                    .get("from_dataset")
                    .or_else(|| map.get("id"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    out.insert(id.to_string());
                }
            }
            if map.get("__kind").and_then(Value::as_str) == Some("analysis_expr") {
                if map.get("type").and_then(Value::as_str) == Some("rows") {
                    if let Some(id) = map.get("dataset").and_then(Value::as_str) {
                        let text = id.trim();
                        if !text.is_empty() {
                            out.insert(text.strip_prefix("dataset.").unwrap_or(text).to_string());
                        }
                    }
                }
            }
            for nested in map.values() {
                collect_dataset_ids_from_value(nested, out);
            }
        }
        _ => {}
    }
}
