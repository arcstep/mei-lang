use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::RuntimeState;

use super::bundle::load_world_runtime_bundle;
use crate::http::scene_api::types::{WorldRuntimePeekResponse, WorldScope};
use super::util::normalize_limit;

fn recent_trace_messages(state: &RuntimeState, trace_limit: usize) -> Vec<String> {
    state
        .trace_events
        .iter()
        .rev()
        .take(trace_limit)
        .map(|item| item.message.clone())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub(super) fn recent_trace_messages_for_snapshot(state: &RuntimeState, trace_limit: usize) -> Vec<String> {
    recent_trace_messages(state, trace_limit)
}

pub(crate) fn query_world_runtime(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    trace_limit: Option<usize>,
) -> Result<WorldRuntimePeekResponse> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let normalized_trace_limit = normalize_limit(trace_limit, 5, 50);
    Ok(WorldRuntimePeekResponse {
        app_id: app_id.to_string(),
        scene_id: bundle.contract.scene.id.clone(),
        phase: bundle.state.phase.clone(),
        result: bundle.state.result.clone(),
        countdown: bundle.state.countdown,
        available_actions: bundle
            .scene_view
            .available_actions
            .iter()
            .take(20)
            .cloned()
            .collect(),
        recent_trace_messages: recent_trace_messages(&bundle.state, normalized_trace_limit),
    })
}
