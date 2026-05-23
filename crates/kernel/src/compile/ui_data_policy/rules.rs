use serde_json::Value;

pub(super) fn collect_forbidden_paths(value: &Value, path: &str) -> Vec<String> {
    let mut out = Vec::new();
    match value {
        Value::Object(map) => {
            if forbidden_binding(map) {
                out.push(path.to_string());
            }
            for (key, child) in map {
                let next = format!("{path}.{key}");
                out.extend(collect_forbidden_paths(child, &next));
            }
        }
        Value::Array(items) => {
            for (idx, child) in items.iter().enumerate() {
                let next = format!("{path}[{idx}]");
                out.extend(collect_forbidden_paths(child, &next));
            }
        }
        _ => {}
    }
    out
}

pub(super) fn forbidden_binding(map: &serde_json::Map<String, Value>) -> bool {
    if map.get("__ref").and_then(Value::as_str) == Some("data") {
        return true;
    }
    map.get("__kind").and_then(Value::as_str) == Some("analysis_expr")
        && map.get("type").and_then(Value::as_str) == Some("rows")
}

pub(super) fn has_external_locator(map: &serde_json::Map<String, Value>) -> bool {
    map.get("scene_file")
        .or_else(|| map.get("path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some()
        || map
            .get("scene_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some()
}
