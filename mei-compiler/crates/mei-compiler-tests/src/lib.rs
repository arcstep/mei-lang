use serde_json::Value as JsonValue;

/// 规范化 Decl IR JSON 以便 golden 比较（递归排序对象键）。
pub fn normalize_decl_ir(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for key in keys {
                if let Some(entry) = map.get(&key) {
                    out.insert(key, normalize_decl_ir(entry));
                }
            }
            JsonValue::Object(out)
        }
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(normalize_decl_ir).collect()),
        other => other.clone(),
    }
}

pub fn assert_decl_ir_eq(left: &JsonValue, right: &JsonValue) {
    let left = normalize_decl_ir(left);
    let right = normalize_decl_ir(right);
    assert_eq!(
        left,
        right,
        "decl IR mismatch:\nleft={}\nright={}",
        serde_json::to_string_pretty(&left).unwrap_or_default(),
        serde_json::to_string_pretty(&right).unwrap_or_default()
    );
}

pub fn mei_projects_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("mei-projects root")
        .to_path_buf()
}

pub fn ws_hello_hello_app_dir() -> std::path::PathBuf {
    mei_projects_root().join("workspaces/ws-hello/apps/hello")
}
