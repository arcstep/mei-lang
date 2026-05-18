use serde_json::Value;

pub(super) const LLM_RESOURCE_GET_BUDGET_CHARS: usize = 12_000;

pub(super) fn json_serialized_len(v: &Value) -> usize {
    serde_json::to_string(v).map(|s| s.len()).unwrap_or(0)
}

pub(super) fn shrink_json_for_llm(v: &Value, max_total: usize) -> Value {
    let len = json_serialized_len(v);
    if len <= max_total {
        return v.clone();
    }
    match v {
        Value::Object(m) => {
            let mut out = serde_json::Map::new();
            for (k, val) in m.iter().take(48) {
                let elen = json_serialized_len(val);
                if elen > 2_000 {
                    out.insert(k.clone(), serde_json::json!({ "_omitted": true, "approx_chars": elen }));
                } else {
                    out.insert(k.clone(), val.clone());
                }
            }
            out.insert(
                "_truncated".to_string(),
                serde_json::json!({
                    "reason": "payload too large for tool output",
                    "approx_original_chars": len,
                }),
            );
            Value::Object(out)
        }
        Value::Array(a) => serde_json::json!({
            "type": "array",
            "len": a.len(),
            "head": a.iter().take(5).cloned().collect::<Vec<_>>(),
        }),
        Value::String(s) => {
            let cap = 1_000usize;
            if s.len() <= cap {
                Value::String(s.clone())
            } else {
                Value::String(format!("{}…", s.chars().take(cap).collect::<String>()))
            }
        }
        other => other.clone(),
    }
}
