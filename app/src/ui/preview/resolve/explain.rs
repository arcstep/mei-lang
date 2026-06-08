use serde_json::Value;

pub(crate) use super::explain_apply::{
    apply_analyses_value, apply_explain_items, apply_explain_object,
};

pub(crate) fn string_array_from_value(value: &Value) -> Vec<String> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.as_str().map(str::trim))
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

pub(crate) fn object_map_from_value(value: &Value) -> serde_json::Map<String, Value> {
    let Some(map) = value.as_object() else {
        return serde_json::Map::new();
    };
    map.clone()
}

