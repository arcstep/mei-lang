//! Session id for theme.layout / theme.tokens draft overlay (per browser tab / client).

use http::HeaderMap;
use std::sync::atomic::{AtomicU64, Ordering};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub const DRAFT_SESSION_HEADER: &str = "x-mei-draft-session";
pub const DRAFT_SESSION_COOKIE: &str = "mei-draft-session";

pub fn resolve_draft_session_id(headers: &HeaderMap) -> String {
    if let Some(value) = headers
        .get(DRAFT_SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return value.to_string();
    }
    if let Some(cookie) = headers.get(http::header::COOKIE).and_then(|v| v.to_str().ok()) {
        for part in cookie.split(';') {
            let piece = part.trim();
            if let Some(value) = piece.strip_prefix(&format!("{DRAFT_SESSION_COOKIE}=")) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    format!(
        "host-{}",
        SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}
