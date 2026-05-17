use std::time::Instant;

use serde_json::Value;

pub(crate) fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

pub(crate) fn value_to_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        other => other.to_string(),
    }
}
