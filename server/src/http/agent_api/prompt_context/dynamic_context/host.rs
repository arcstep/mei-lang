use std::hash::{Hash, Hasher};

use crate::agent_runtime::bridge::BridgePromptRequest;

pub(super) fn append_host_protocol_lines(
    lines: &mut Vec<String>,
    host_protocol: Option<&serde_json::Value>,
) {
    let Some(host) = host_protocol.and_then(|value| value.as_object()) else {
        return;
    };
    let schema = host
        .get("schema")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let surface = host
        .get("surface")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let route_mode = host
        .get("route_mode")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let mode = host
        .get("mode")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    lines.push(String::new());
    lines.push("[Host — protocol]".to_string());
    lines.push(format!(
        "schema={} surface={} route_mode={} mode={}",
        schema, surface, route_mode, mode
    ));
}

pub(super) fn append_host_contract_schema_line(lines: &mut Vec<String>, schema: Option<&str>) {
    let schema = schema
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    lines.push(format!("host_contract_schema={schema}"));
}

pub(super) fn host_protocol_digest(request: &BridgePromptRequest) -> String {
    let Some(value) = request.host_protocol.as_ref() else {
        return "na".to_string();
    };
    let Ok(raw) = serde_json::to_vec(value) else {
        return "invalid".to_string();
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
