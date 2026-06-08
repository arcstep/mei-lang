use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::expand::support_role_for_item;

pub(super) fn first_non_empty_string(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        map.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(super) fn metric_note_text(value: &Value) -> Option<String> {
    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return Some(text.to_string());
    }
    value
        .as_object()
        .and_then(|map| first_non_empty_string(map, &["content", "text", "note"]))
}

pub(super) fn metric_note_format(value: &Value) -> Option<String> {
    if value.as_str().is_some() {
        return Some("text".to_string());
    }
    value
        .as_object()
        .and_then(|map| first_non_empty_string(map, &["format"]))
        .or_else(|| metric_note_text(value).map(|_| "text".to_string()))
}

pub(super) fn apply_metric_narrative(map: &Map<String, Value>, contract: &mut Map<String, Value>) {
    if let Some(note_value) = map.get("note") {
        if let Some(text) = metric_note_text(note_value) {
            contract.insert("note".to_string(), Value::String(text));
            if let Some(format) = metric_note_format(note_value) {
                contract.insert("note_format".to_string(), Value::String(format));
            }
        }
    }
    for key in ["basis_refs", "recommended_dimensions"] {
        if contract.contains_key(key) {
            continue;
        }
        if let Some(value) = map.get(key).filter(|value| !value_is_empty(value)) {
            contract.insert(key.to_string(), value.clone());
        }
    }
}

pub(super) fn merge_definition_narrative_fallback(
    item_map: &Map<String, Value>,
    contract: &mut Map<String, Value>,
) {
    if !contract.contains_key("note") {
        if let Some(note) = first_non_empty_string(
            item_map,
            &[
                "note",
                "content",
                "text",
                "markdown",
                "md",
                "desc",
                "description",
            ],
        ) {
            contract.insert("note".to_string(), Value::String(note));
            contract.insert("note_format".to_string(), Value::String("text".to_string()));
        }
    }
    if !contract.contains_key("basis_refs") {
        if let Some(value) = item_map
            .get("basis_refs")
            .filter(|value| !value_is_empty(value))
        {
            contract.insert("basis_refs".to_string(), value.clone());
        }
    }
    if !contract.contains_key("recommended_dimensions") {
        if let Some(value) = item_map
            .get("recommended_dimensions")
            .filter(|value| !value_is_empty(value))
        {
            contract.insert("recommended_dimensions".to_string(), value.clone());
        }
    }
}

pub(super) fn is_empty_legacy_definition_item(item_map: &Map<String, Value>) -> bool {
    support_role_for_item(item_map) == "definition"
        && metric_note_text(&Value::Object(item_map.clone())).is_none()
        && item_map
            .get("basis_refs")
            .is_none_or(|value| value_is_empty(value))
        && item_map
            .get("recommended_dimensions")
            .is_none_or(|value| value_is_empty(value))
}

pub(super) fn value_is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => false,
    }
}

pub(super) fn copy_field(source: &Map<String, Value>, out: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).cloned() {
        out.insert(key.to_string(), value);
    }
}

pub(super) fn push_edge(
    edges: &mut BTreeSet<(String, String, String)>,
    from: &str,
    to: &str,
    role: &str,
) {
    let from = from.trim();
    let to = to.trim();
    if from.is_empty() || to.is_empty() {
        return;
    }
    let role = role.trim();
    edges.insert((
        from.to_string(),
        to.to_string(),
        if role.is_empty() {
            "support".to_string()
        } else {
            role.to_string()
        },
    ));
}
