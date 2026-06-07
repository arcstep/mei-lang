use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    agent_runtime::{
        bridge::{
            BridgeAbortSummary, BridgeCreateSessionRequest, BridgePermissionResponseRequest,
            BridgePermissionResponseSummary, BridgeRevertRequest, BridgeSessionDiffQuery,
            BridgeSessionMessageRaw, BridgeSessionSummary,
        },
        events::{
            normalize_upstream_message_to_snapshot, HostOpencodeEvent, HostOpencodeMessageList,
        },
    },
    mei_agent::{
        agent_abort_session, agent_create_session, agent_list_sessions, agent_respond_permission,
        agent_session_messages,
        native::{encode_host_event_line, filter_session_event},
        resolve_agent_conn,
    },
    AppState,
};

/// 与编辑侧外置化一致：宿主不再通过内置 Agent 改写 `.mei`（见 `topics/50`、`implementation/host/60`）。
pub(crate) const AUTHORING_WRITEBACK_RETIRED_ERROR: &str = "authoring_writeback_retired";

pub(crate) const AUTHORING_WRITEBACK_RETIRED_MESSAGE: &str =
    "宿主内置编辑侧 AI 代码写回已下线；请使用外部开发工具编辑 `.mei`，或通过 /api/ops/journal 查看运维变更。";

pub(crate) const AUTHORING_WRITEBACK_RETIRED_HISTORY_HINT: &str =
    "宿主内置编辑侧 AI 写回已下线（diff / Undo / Redo 不可用）；源码修改请交给 Cursor / Codex 等外部工具。";

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
    _state: State<AppState>,
    _session_id: Path<String>,
    _query: Query<BridgeSessionDiffQuery>,
) -> Response {
    authoring_writeback_retired_response("agent session diff 已随编辑侧写回能力一并下线。")
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
    _state: State<AppState>,
    _session_id: Path<String>,
    _request: Json<BridgeRevertRequest>,
) -> Response {
    authoring_writeback_retired_response("agent revert 已随编辑侧写回能力一并下线。")
}

pub async fn api_agent_unrevert_session(
    _state: State<AppState>,
    _session_id: Path<String>,
) -> Response {
    authoring_writeback_retired_response("agent unrevert 已随编辑侧写回能力一并下线。")
}

fn authoring_writeback_retired_response(detail: &str) -> Response {
    (
        StatusCode::GONE,
        Json(serde_json::json!({
            "error": AUTHORING_WRITEBACK_RETIRED_ERROR,
            "message": format!("{detail} {}", AUTHORING_WRITEBACK_RETIRED_MESSAGE),
        })),
    )
        .into_response()
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
