use std::hash::{Hash, Hasher};

use crate::agent_runtime::bridge::BridgePromptRequest;

pub(super) fn append_browser_context_lines(lines: &mut Vec<String>, browser_context: Option<&serde_json::Value>) {
    let Some(ctx) = browser_context.and_then(|value| value.as_object()) else {
        return;
    };
    let view_tab = ctx
        .get("view_tab")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("-");
    let overlay_open = ctx
        .get("overlay_open")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let active_ids = ctx
        .get("active_query_state_ids")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    lines.push(String::new());
    lines.push("[Browser — context]".to_string());
    lines.push(format!(
        "view_tab={} overlay_open={} query_states={}",
        view_tab,
        if overlay_open { "true" } else { "false" },
        active_ids.len()
    ));
    let ids = active_ids
        .iter()
        .filter_map(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .take(8)
        .collect::<Vec<_>>();
    if !ids.is_empty() {
        lines.push(format!("active_query_state_ids: {}", ids.join(", ")));
    }
}


pub(super) fn browser_context_digest(request: &BridgePromptRequest) -> String {
    let Some(value) = request.browser_context.as_ref() else {
        return "na".to_string();
    };
    let Ok(raw) = serde_json::to_vec(value) else {
        return "invalid".to_string();
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
