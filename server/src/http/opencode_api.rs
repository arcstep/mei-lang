use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{
        header::CONTENT_TYPE,
        HeaderValue,
    },
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    opencode::{
        bridge::{
            abort_session as bridge_abort_session, create_session as bridge_create_session,
            global_event as bridge_global_event, health as bridge_health,
            list_sessions as bridge_list_sessions, respond_permission as bridge_respond_permission,
            send_prompt as bridge_send_prompt, session_messages as bridge_session_messages,
            BridgeCreateSessionRequest, BridgeHealthResponse, BridgePermissionResponseRequest,
            BridgePromptRequest, BridgeSessionSummary,
        },
        events::{
            extract_sse_data, normalize_global_event_to_host_event,
            normalize_upstream_message_to_snapshot, HostOpencodeEvent, HostOpencodeMessageList,
        },
        runtime::{
            managed_opencode_config_summary, managed_opencode_runtime_status,
            managed_opencode_server_url, start_managed_opencode, stop_managed_opencode,
        },
        StartManagedOpencodeRequest,
    },
    AppState,
};

use super::error_response;

#[derive(Debug, Deserialize)]
pub struct SessionMessagesQuery {
    limit: Option<usize>,
}

const DEFAULT_SESSION_MESSAGES_LIMIT: usize = 80;
const MAX_SESSION_MESSAGES_LIMIT: usize = 300;

fn normalize_session_messages_limit(limit: Option<usize>) -> usize {
    let resolved = limit.unwrap_or(DEFAULT_SESSION_MESSAGES_LIMIT);
    resolved.clamp(1, MAX_SESSION_MESSAGES_LIMIT)
}

pub async fn api_opencode_config(State(state): State<AppState>) -> Response {
    Json(managed_opencode_config_summary(&state)).into_response()
}

pub async fn api_opencode_runtime(State(state): State<AppState>) -> Response {
    match managed_opencode_runtime_status(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_health(State(state): State<AppState>) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(_) => {
            return Json(BridgeHealthResponse {
                server_url: String::new(),
                healthy: false,
                version: String::new(),
            })
            .into_response()
        }
    };
    match bridge_health(&state.opencode_http, &server_url).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

fn take_sse_frame(buffer: &mut String) -> Option<String> {
    let idx = buffer.find("\n\n")?;
    let frame = buffer[..idx].to_string();
    let rest = buffer[idx + 2..].to_string();
    *buffer = rest;
    Some(frame)
}

/// OpenCode 未启动或上游不可用时，仍返回 **200 + event-stream**，避免浏览器 EventSource 对非 2xx 无限重连，
/// 并由前端收到 `session_status` 后主动 `close()` 停止重连。
fn sse_session_status_notice(session_id: String, status: &str, message: impl Into<String>) -> Response {
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

pub async fn api_opencode_start(
    State(state): State<AppState>,
    Json(request): Json<StartManagedOpencodeRequest>,
) -> Response {
    match start_managed_opencode(&state, request).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_stop(State(state): State<AppState>) -> Response {
    match stop_managed_opencode(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_create_session(
    State(state): State<AppState>,
    Json(request): Json<BridgeCreateSessionRequest>,
) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    match bridge_create_session(&state.opencode_http, &server_url, request).await {
        Ok(session) => Json(session).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_list_sessions(State(state): State<AppState>) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(_) => return Json(Vec::<BridgeSessionSummary>::new()).into_response(),
    };
    match bridge_list_sessions(&state.opencode_http, &server_url).await {
        Ok(sessions) => Json(sessions).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_send_message(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<BridgePromptRequest>,
) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    match bridge_send_prompt(&state.opencode_http, &server_url, &session_id, request).await {
        Ok(summary) => Json(summary).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_session_events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(_) => {
            return sse_session_status_notice(
                session_id,
                "opencode_unavailable",
                "OpenCode 服务当前不可用；请先检查外部 opencode-server 是否已启动并可访问。",
            );
        }
    };
    match bridge_global_event(&state.opencode_http, &server_url).await {
        Ok(upstream) => {
            let stream = async_stream::stream! {
                let mut upstream = upstream;
                let mut buffer = String::new();
                loop {
                    let chunk = match upstream.chunk().await {
                        Ok(Some(chunk)) => chunk,
                        Ok(None) => break,
                        Err(error) => {
                            let event = HostOpencodeEvent::debug(
                                "upstream_chunk_error",
                                json!({ "error": error.to_string() }),
                            );
                            if let Ok(encoded) = serde_json::to_string(&event) {
                                yield Ok::<String, std::io::Error>(format!("data: {encoded}\n\n"));
                            }
                            break;
                        }
                    };
                    buffer.push_str(String::from_utf8_lossy(&chunk).as_ref());
                    if buffer.contains("\r\n") {
                        buffer = buffer.replace("\r\n", "\n");
                    }
                    while let Some(frame) = take_sse_frame(&mut buffer) {
                        if let Some(data) = extract_sse_data(&frame) {
                            let normalized = match serde_json::from_str::<Value>(&data) {
                                Ok(event) => normalize_global_event_to_host_event(event, &session_id),
                                Err(_) => {
                                    Some(HostOpencodeEvent::debug(
                                        "decode_error",
                                        json!({ "raw": data }),
                                    ))
                                }
                            };
                            if let Some(event) = normalized {
                                if let Ok(encoded) = serde_json::to_string(&event) {
                                    yield Ok::<String, std::io::Error>(format!("data: {encoded}\n\n"));
                                }
                            }
                        }
                    }
                }
            };
            let mut response = Response::new(Body::from_stream(stream));
            response
                .headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
            response
        }
        Err(error) => sse_session_status_notice(
            session_id,
            "upstream_unavailable",
            format!("无法连接 OpenCode 事件流：{error}"),
        ),
    }
}

pub async fn api_opencode_session_messages(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<SessionMessagesQuery>,
) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    match bridge_session_messages(&state.opencode_http, &server_url, &session_id).await {
        Ok(rows) => {
            let mut messages = rows
                .iter()
                .filter_map(normalize_upstream_message_to_snapshot)
                .filter(|item| item.session_id == session_id)
                .collect::<Vec<_>>();
            let limit = normalize_session_messages_limit(query.limit);
            if messages.len() > limit {
                messages = messages.split_off(messages.len() - limit);
            }
            Json(HostOpencodeMessageList {
                session_id,
                messages,
            })
            .into_response()
        }
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_abort_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    match bridge_abort_session(&state.opencode_http, &server_url, &session_id).await {
        Ok(summary) => Json(summary).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_respond_permission(
    State(state): State<AppState>,
    Path((session_id, permission_id)): Path<(String, String)>,
    Json(request): Json<BridgePermissionResponseRequest>,
) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    match bridge_respond_permission(
        &state.opencode_http,
        &server_url,
        &session_id,
        &permission_id,
        request,
    )
    .await
    {
        Ok(summary) => Json(summary).into_response(),
        Err(error) => error_response(error),
    }
}
