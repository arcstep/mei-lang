use serde_json::Value;

pub(crate) fn deep_merge_json(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            let mut out = base_map.clone();
            for (key, value) in overlay_map {
                if let Some(existing) = out.get(key) {
                    if existing.is_object() && value.is_object() {
                        out.insert(key.clone(), deep_merge_json(existing, value));
                    } else {
                        out.insert(key.clone(), value.clone());
                    }
                } else {
                    out.insert(key.clone(), value.clone());
                }
            }
            Value::Object(out)
        }
        _ => overlay.clone(),
    }
}

pub(super) fn value_has_key(value: &Value, key: &str) -> bool {
    value.as_object().is_some_and(|map| map.contains_key(key))
}
