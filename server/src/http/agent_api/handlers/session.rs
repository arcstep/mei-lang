use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header::CONTENT_TYPE, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    agent_runtime::{
        bridge::{
            BridgeAbortSummary, BridgeCreateSessionRequest, BridgeDiffSummary,
            BridgePermissionResponseRequest, BridgePermissionResponseSummary, BridgeRevertRequest,
            BridgeRevertSummary, BridgeSessionDiffQuery, BridgeSessionMessageRaw,
            BridgeSessionSummary, BridgeUnrevertSummary,
        },
        events::{normalize_upstream_message_to_snapshot, HostOpencodeEvent, HostOpencodeMessageList},
    },
    mei_agent::{
        agent_abort_session, agent_create_session, agent_list_sessions, agent_respond_permission,
        agent_revert_session, agent_session_diff, agent_session_messages, agent_unrevert_session,
        native::{encode_host_event_line, filter_session_event},
        resolve_agent_conn, sanitize_relative_path,
    },
    AppState,
};

use crate::http::agent_api::{
    permissions::{
        collect_and_reject_blocked_permissions, normalize_session_messages_limit,
        HostBlockedPermissionList, SessionMessagesQuery,
    },
    sse::sse_session_status_notice,
};
use crate::http::error_response;

pub async fn api_agent_create_session(
    State(state): State<AppState>,
    Json(request): Json<BridgeCreateSessionRequest>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(error) => return error_response(error),
    };
    match agent_create_session(&state, &conn, request).await {
        Ok(session) => Json::<BridgeSessionSummary>(session).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_list_sessions(State(state): State<AppState>) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(_) => return Json(Vec::<BridgeSessionSummary>::new()).into_response(),
    };
    match agent_list_sessions(&state, &conn).await {
        Ok(sessions) => Json::<Vec<BridgeSessionSummary>>(sessions).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_pending_permissions(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(error) => return error_response(error),
    };
    match collect_and_reject_blocked_permissions(&state, &conn, &session_id).await {
        Ok(pending) => Json(HostBlockedPermissionList {
            session_id,
            pending,
        })
        .into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_session_events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(_) => {
            return sse_session_status_notice(
                session_id,
                "agent_unavailable",
                "内置助手未初始化；请确认服务启动成功且工作区 `.mei` 可写。",
            );
        }
    };

    let agent = conn.clone();
    let mut rx = agent.subscribe_events();
    let sid = session_id.clone();
    let stream = async_stream::stream! {
        let hello = HostOpencodeEvent::SessionStatus {
            session_id: sid.clone(),
            status: "connected".to_string(),
            message: "事件流已连接".to_string(),
        };
        if let Some(line) = encode_host_event_line(&hello) {
            yield Ok::<String, std::io::Error>(line);
        }
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if let Some(f) = filter_session_event(ev, &sid) {
                        if let Some(line) = encode_host_event_line(&f) {
                            yield Ok::<String, std::io::Error>(line);
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    let mut response = Response::new(Body::from_stream(stream));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("text/event-stream"));
    response
}

pub async fn api_agent_session_messages(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<SessionMessagesQuery>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(error) => return error_response(error),
    };
    match agent_session_messages(&state, &conn, &session_id).await {
        Ok(rows) => {
            let rows: Vec<BridgeSessionMessageRaw> = rows;
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

pub async fn api_agent_session_diff(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<BridgeSessionDiffQuery>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(error) => return error_response(error),
    };
    let diff_path = query.path.as_deref().and_then(sanitize_relative_path);
    match agent_session_diff(
        &state,
        &conn,
        &session_id,
        query.message_id.as_deref(),
        diff_path,
    )
    .await
    {
        Ok(summary) => Json::<BridgeDiffSummary>(summary).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_abort_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(error) => return error_response(error),
    };
    match agent_abort_session(&state, &conn, &session_id).await {
        Ok(summary) => Json::<BridgeAbortSummary>(summary).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_revert_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<BridgeRevertRequest>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(error) => return error_response(error),
    };
    match agent_revert_session(&state, &conn, &session_id, request).await {
        Ok(summary) => Json::<BridgeRevertSummary>(summary).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_unrevert_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(error) => return error_response(error),
    };
    match agent_unrevert_session(&state, &conn, &session_id).await {
        Ok(summary) => Json::<BridgeUnrevertSummary>(summary).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_respond_permission(
    State(state): State<AppState>,
    Path((session_id, permission_id)): Path<(String, String)>,
    Json(request): Json<BridgePermissionResponseRequest>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(error) => return error_response(error),
    };
    match agent_respond_permission(&state, &conn, &session_id, &permission_id, request).await {
        Ok(summary) => Json::<BridgePermissionResponseSummary>(summary).into_response(),
        Err(error) => error_response(error),
    }
}
