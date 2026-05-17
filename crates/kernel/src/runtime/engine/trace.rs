use serde_json::Value;

use crate::runtime::types::{RuntimeState, RuntimeTraceItem};

pub(in crate::runtime::engine) fn push_timeline(
    state: &mut RuntimeState,
    message: impl Into<String>,
) {
    state.timeline.push(message.into());
    if state.timeline.len() > 12 {
        let overflow = state.timeline.len() - 12;
        state.timeline.drain(0..overflow);
    }
}

pub(in crate::runtime::engine) fn push_trace(
    state: &mut RuntimeState,
    kind: impl Into<String>,
    message: impl Into<String>,
    details: Value,
) {
    state.trace_events.push(RuntimeTraceItem {
        kind: kind.into(),
        message: message.into(),
        details,
    });
    if state.trace_events.len() > 12 {
        let overflow = state.trace_events.len() - 12;
        state.trace_events.drain(0..overflow);
    }
}
