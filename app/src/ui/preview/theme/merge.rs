use mei_lang_kernel::UiNodeDecl;
use serde_json::Value;

use super::ThemeResolved;

pub(crate) fn resolve_shared_refs(value: &Value, shared: &Value) -> Value {
    match value {
        Value::Object(map) => {
            if map.get("__ref").and_then(Value::as_str) == Some("shared") {
                return resolve_shared_ref(map, shared);
            }
            let mut out = serde_json::Map::new();
            for (key, entry) in map {
                out.insert(key.clone(), resolve_shared_refs(entry, shared));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| resolve_shared_refs(item, shared))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn resolve_shared_ref(map: &serde_json::Map<String, Value>, shared: &Value) -> Value {
    let key = map
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let resolved = key.and_then(|path| read_shared_path(shared, path));
    resolved
        .cloned()
        .or_else(|| map.get("default").cloned())
        .unwrap_or(Value::Null)
}

fn read_shared_path<'a>(shared: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = shared;
    for segment in path.split('.') {
        let key = segment.trim();
        if key.is_empty() {
            return None;
        }
        current = current.as_object()?.get(key)?;
    }
    Some(current)
}

/// 整卡 panel：theme.panel + `props`（剥离槽位键）。
pub(crate) fn resolve_panel_card_props(theme: &ThemeResolved, panel: &UiNodeDecl) -> Value {
    let merged = resolve_panel_props(theme, &panel.props);
    strip_slot_keys_from_card_props(&merged)
}

pub(crate) fn resolve_panel_props(theme: &ThemeResolved, props: &Value) -> Value {
    let use_bare = props
        .as_object()
        .and_then(|map| map.get("chrome"))
        .and_then(Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("bare"));
    if use_bare {
        deep_merge_value(&theme.panel_bare, props)
    } else {
        deep_merge_value(&theme.panel, props)
    }
}

pub(crate) fn resolve_panel_head_props(theme: &ThemeResolved, panel: &UiNodeDecl) -> Value {
    deep_merge_value(&theme.panel_head, &panel.head_props)
}

pub(crate) fn resolve_panel_body_props(theme: &ThemeResolved, panel: &UiNodeDecl) -> Value {
    deep_merge_value(&theme.panel_body, &panel.body_props)
}

fn strip_slot_keys_from_card_props(props: &Value) -> Value {
    let Some(map) = props.as_object() else {
        return props.clone();
    };
    let mut map = map.clone();
    map.remove("heading");
    Value::Object(map)
}

pub(crate) fn deep_merge_value(base: &Value, overlay: &Value) -> Value {
    let (Some(base_obj), Some(overlay_obj)) = (base.as_object(), overlay.as_object()) else {
        return overlay.clone();
    };
    let mut merged = base_obj.clone();
    for (key, value) in overlay_obj {
        let next = if let Some(existing) = merged.get(key) {
            deep_merge_value(existing, value)
        } else {
            value.clone()
        };
        merged.insert(key.clone(), next);
    }
    Value::Object(merged)
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_shared_refs_replaces_nested_shared_refs() {
        let shared = json!({
            "layout": {
                "rail_width": "520px",
                "table": {"preview_chars": 18}
            }
        });
        let value = json!({
            "width": {"__ref": "shared", "id": "layout.rail_width"},
            "components": {
                "dataset_table": {
                    "cell_preview_max_chars": {"__ref": "shared", "id": "layout.table.preview_chars"},
                }
            }
        });
        let resolved = resolve_shared_refs(&value, &shared);
        assert_eq!(resolved.get("width").and_then(Value::as_str), Some("520px"));
        assert_eq!(
            resolved
                .get("components")
                .and_then(Value::as_object)
                .and_then(|map| map.get("dataset_table"))
                .and_then(Value::as_object)
                .and_then(|map| map.get("cell_preview_max_chars"))
                .and_then(Value::as_i64),
            Some(18)
        );
    }
}
