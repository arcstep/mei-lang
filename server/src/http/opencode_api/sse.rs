use axum::{
    body::Body,
    http::{header::CONTENT_TYPE, HeaderValue},
    response::Response,
};
use serde_json;

use crate::opencode::events::HostOpencodeEvent;

/// OpenCode 未启动或上游不可用时，仍返回 **200 + event-stream**，避免浏览器 EventSource 对非 2xx 无限重连，
/// 并由前端收到 `session_status` 后主动 `close()` 停止重连。
pub(crate) fn sse_session_status_notice(
    session_id: String,
    status: &str,
    message: impl Into<String>,
) -> Response {
    let event = HostOpencodeEvent::SessionStatus {
        session_id,
        status: status.to_string(),
        message: message.into(),
    };
    let stream = async_stream::stream! {
        if let Ok(encoded) = serde_json::to_string(&event) {
            yield Ok::<String, std::io::Error>(format!("data: {encoded}\n\n"));
        }
    };
    let mut response = Response::new(Body::from_stream(stream));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    response
}
