use std::path::Path as FsPath;

use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    opencode::{
        bridge::{
            abort_session as bridge_abort_session, create_session as bridge_create_session,
            global_event as bridge_global_event, health as bridge_health,
            list_sessions as bridge_list_sessions,
            project_current_worktree as bridge_project_current_worktree,
            respond_permission as bridge_respond_permission,
            revert_session_message as bridge_revert_session_message,
            send_prompt as bridge_send_prompt, session_diff as bridge_session_diff,
            session_messages as bridge_session_messages,
            unrevert_session as bridge_unrevert_session, vcs_summary as bridge_vcs_summary,
            BridgeCreateSessionRequest, BridgeHealthResponse, BridgePermissionResponseRequest,
            BridgePromptRequest, BridgeRevertRequest, BridgeSessionDiffQuery, BridgeSessionSummary,
        },
        events::{
            extract_sse_data, normalize_global_event_to_host_event,
            normalize_upstream_message_to_snapshot, HostOpencodeEvent, HostOpencodeMessageList,
        },
        runtime::{
            managed_opencode_config_summary, managed_opencode_runtime_status,
            managed_opencode_server_url, managed_opencode_skill_status, start_managed_opencode,
            stop_managed_opencode, sync_managed_opencode_skill,
        },
        StartManagedOpencodeRequest,
    },
    AppState,
};

use super::super::error_response;
use super::super::scene_api::{
    build_world_context_snapshot, default_resource_query_tools, WorldScope,
    RESOURCE_QUERY_SCHEMA_VERSION,
};
use super::permissions::{
    classify_blocked_permission, collect_and_reject_blocked_permissions,
    normalize_session_messages_limit, HostBlockedPermissionList, SessionMessagesQuery,
};
use super::prompt_context::{
    build_dynamic_session_context_preview, enrich_prompt_request, load_or_refresh_session_context,
};
use super::sse::{sse_session_status_notice, take_sse_frame};

pub async fn api_opencode_config(State(state): State<AppState>) -> Response {
    Json(managed_opencode_config_summary(&state)).into_response()
}

pub async fn api_opencode_runtime(State(state): State<AppState>) -> Response {
    match managed_opencode_runtime_status(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_skill(State(state): State<AppState>) -> Response {
    match managed_opencode_skill_status(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_sync_skill(State(state): State<AppState>) -> Response {
    match sync_managed_opencode_skill(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_health(State(state): State<AppState>) -> Response {
    fn normalize_path(value: &str) -> String {
        value.trim().trim_end_matches('/').to_string()
    }

    fn worktree_matches_expected(project_worktree: &str, expected_worktree: &str) -> bool {
        let project = normalize_path(project_worktree);
        let expected = normalize_path(expected_worktree);
        if project == expected {
            return true;
        }

        let project_path = FsPath::new(&project);
        let expected_path = FsPath::new(&expected);
        // OpenCode 可能返回更外层的 project/worktree，Mei `source_root` 更具体
        if expected_path.starts_with(project_path) {
            return true;
        }
        // OpenCode cwd 在子目录，Mei 绑定整个 `workspaces`
        if project_path.starts_with(expected_path) {
            return true;
        }
        false
    }

    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(_) => {
            return Json(BridgeHealthResponse {
                server_url: String::new(),
                healthy: false,
                version: String::new(),
                expected_worktree: Some(state.source_root.display().to_string()),
                project_worktree: None,
                vcs_detected: false,
                vcs_branch: None,
                history_available: false,
                history_reason: Some(
                    "OpenCode 服务当前不可用；Undo/Redo 与自动刷新依赖正确的 worktree 和 Git/VCS 视角。"
                        .to_string(),
                ),
            })
            .into_response()
        }
    };
    match bridge_health(&state.opencode_http, &server_url).await {
        Ok(mut status) => {
            let expected_worktree = state.source_root.display().to_string();
            status.expected_worktree = Some(expected_worktree.clone());
            match bridge_project_current_worktree(&state.opencode_http, &server_url).await {
                Ok(project_worktree) => status.project_worktree = project_worktree,
                Err(error) => {
                    status.history_available = false;
                    status.history_reason =
                        Some(format!("无法读取 OpenCode 当前 worktree：{error}"));
                    return Json(status).into_response();
                }
            }
            match bridge_vcs_summary(&state.opencode_http, &server_url).await {
                Ok((vcs_detected, vcs_branch)) => {
                    status.vcs_detected = vcs_detected;
                    status.vcs_branch = vcs_branch;
                }
                Err(error) => {
                    status.history_available = false;
                    status.history_reason = Some(format!("无法读取 OpenCode VCS 状态：{error}"));
                    return Json(status).into_response();
                }
            }
            let project_matches = status
                .project_worktree
                .as_deref()
                .is_some_and(|value| worktree_matches_expected(value, &expected_worktree));
            if !status.healthy {
                status.history_available = false;
                status.history_reason =
                    Some("OpenCode 服务未连接；Undo/Redo 与自动刷新当前不可用。".to_string());
            } else if !project_matches {
                status.history_available = false;
                status.history_reason = Some(format!(
                    "OpenCode 当前 worktree 为 {}，而 MeiLang 预期工作区为 {}；Undo/Redo 与自动刷新不可用。",
                    status.project_worktree.as_deref().unwrap_or("(unknown)"),
                    expected_worktree
                ));
            } else if !status.vcs_detected {
                status.history_available = false;
                status.history_reason = Some(
                    "OpenCode 当前 worktree 未检测到 Git/VCS；Undo/Redo 与自动刷新不可用。"
                        .to_string(),
                );
            } else {
                status.history_available = true;
                status.history_reason = None;
            }
            Json(status).into_response()
        }
        Err(error) => error_response(error),
    }
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

pub async fn api_opencode_pending_permissions(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    match collect_and_reject_blocked_permissions(&state, &server_url, &session_id).await {
        Ok(pending) => Json(HostBlockedPermissionList {
            session_id,
            pending,
        })
        .into_response(),
        Err(error) => error_response(error),
    }
}

#[derive(Debug, Deserialize)]
pub struct OpencodeContextPreviewQuery {
    pub app_id: String,
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub entry_id: Option<String>,
    #[serde(default)]
    pub target_file: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OpencodeContextPreviewResponse {
    pub app_id: String,
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub entry_id: Option<String>,
    #[serde(default)]
    pub target_file: Option<String>,
    pub session_context: String,
    pub system_prompt: String,
    pub query_schema_version: String,
    #[serde(default)]
    pub query_tools: Vec<Value>,
    pub resource_inventory: Value,
    #[serde(default)]
    pub preview_error: Option<String>,
    #[serde(default)]
    pub skill_status: Option<Value>,
}

pub async fn api_opencode_context_preview(
    State(state): State<AppState>,
    Query(query): Query<OpencodeContextPreviewQuery>,
) -> Response {
    let app_id = query.app_id.trim();
    if app_id.is_empty() {
        return error_response("query parameter `app_id` is required");
    }
    let mut request = BridgePromptRequest {
        text: String::new(),
        app_id: Some(app_id.to_string()),
        scene_id: query.scene_id.clone(),
        entry_id: query.entry_id.clone(),
        target_file: query.target_file.clone(),
        system: None,
        agent: None,
        model: None,
    };
    let session_context =
        build_dynamic_session_context_preview(&state, &request).unwrap_or_else(|| String::new());
    request = enrich_prompt_request(&state, Some(&session_context), request);
    let scope = WorldScope {
        scene_id: query.scene_id.clone(),
        entry_id: query.entry_id.clone(),
        target_file: query.target_file.clone(),
    };
    let (tools, resource_inventory, preview_error) =
        match build_world_context_snapshot(&state.source_root, app_id, Some(&scope)) {
            Ok(snapshot) => {
                let tools = if snapshot.query_tools.is_empty() {
                    default_resource_query_tools()
                } else {
                    snapshot.query_tools.clone()
                };
                (
                    tools,
                    serde_json::to_value(snapshot.resource_inventory).unwrap_or(Value::Null),
                    None,
                )
            }
            Err(error) => {
                // 上下文预览属于辅助信息，不应因为 scope 不匹配/编译中间态持续返回 500。
                tracing::debug!(
                    app_id = %app_id,
                    scene_id = ?query.scene_id,
                    entry_id = ?query.entry_id,
                    target_file = ?query.target_file,
                    %error,
                    "degraded context preview snapshot"
                );
                (default_resource_query_tools(), Value::Null, Some(error.to_string()))
            }
        };
    let query_tools = tools
        .into_iter()
        .map(|item| serde_json::to_value(item).unwrap_or(Value::Null))
        .collect::<Vec<_>>();
    let skill_status = crate::opencode::runtime::managed_opencode_skill_status(&state)
        .ok()
        .and_then(|item| serde_json::to_value(item).ok());
    Json(OpencodeContextPreviewResponse {
        app_id: app_id.to_string(),
        scene_id: query.scene_id,
        entry_id: query.entry_id,
        target_file: query.target_file,
        session_context,
        system_prompt: request.system.unwrap_or_default(),
        query_schema_version: RESOURCE_QUERY_SCHEMA_VERSION.to_string(),
        query_tools,
        resource_inventory,
        preview_error,
        skill_status,
    })
    .into_response()
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
    let session_context = load_or_refresh_session_context(&state, &session_id, &request);
    let request = enrich_prompt_request(&state, session_context.as_deref(), request);
    match bridge_send_prompt(&state.opencode_http, &server_url, &session_id, request).await {
        Ok(summary) => Json(summary).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_session_events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    use axum::body::Body;
    use axum::http::{header::CONTENT_TYPE, HeaderValue};

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
            let opencode_http = state.opencode_http.clone();
            let server_url = server_url.clone();
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
                            let normalized = match normalized {
                                Some(HostOpencodeEvent::PermissionRequested {
                                    session_id,
                                    permission_id,
                                    permission,
                                    patterns,
                                    ..
                                }) => {
                                    let (path, requires_admin, message) =
                                        classify_blocked_permission(&permission, &patterns);
                                    if !permission_id.trim().is_empty() {
                                        let _ = bridge_respond_permission(
                                            &opencode_http,
                                            &server_url,
                                            &session_id,
                                            &permission_id,
                                            BridgePermissionResponseRequest {
                                                response: "reject".to_string(),
                                            },
                                        )
                                        .await;
                                    }
                                    Some(HostOpencodeEvent::PermissionBlocked {
                                        session_id,
                                        permission_id,
                                        permission,
                                        path,
                                        patterns,
                                        requires_admin,
                                        message,
                                    })
                                }
                                other => other,
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

pub async fn api_opencode_session_diff(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<BridgeSessionDiffQuery>,
) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    match bridge_session_diff(
        &state.opencode_http,
        &server_url,
        &session_id,
        query.message_id.as_deref(),
    )
    .await
    {
        Ok(summary) => Json(summary).into_response(),
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

pub async fn api_opencode_revert_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<BridgeRevertRequest>,
) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    match bridge_revert_session_message(&state.opencode_http, &server_url, &session_id, request)
        .await
    {
        Ok(summary) => Json(summary).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_opencode_unrevert_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let server_url = match managed_opencode_server_url(&state) {
        Ok(url) => url,
        Err(error) => return error_response(error),
    };
    match bridge_unrevert_session(&state.opencode_http, &server_url, &session_id).await {
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
