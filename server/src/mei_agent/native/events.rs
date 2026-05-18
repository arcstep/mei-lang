use crate::agent_runtime::events::HostOpencodeEvent;

fn event_matches_session(ev: &HostOpencodeEvent, session_id: &str) -> bool {
    match ev {
        HostOpencodeEvent::SessionStatus { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::MessageInfo { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::MessagePartUpsert { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::MessagePartDelta { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::MessagePartRemoved { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::PermissionRequested { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::PermissionBlocked { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::PermissionResolved { session_id: s, .. } => s == session_id,
        HostOpencodeEvent::DebugRawEvent { session_id: s, .. } => s.as_deref() == Some(session_id),
    }
}

pub fn encode_host_event_line(ev: &HostOpencodeEvent) -> Option<String> {
    serde_json::to_string(ev)
        .ok()
        .map(|s| format!("data: {s}\n\n"))
}

pub fn filter_session_event(ev: HostOpencodeEvent, session_id: &str) -> Option<HostOpencodeEvent> {
    event_matches_session(&ev, session_id).then_some(ev)
}
