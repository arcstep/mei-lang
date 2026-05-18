use std::path::Path as FsPath;
use std::time::Instant;

use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    agent_runtime::{
        bridge::{
            BridgeAbortSummary, BridgeCreateSessionRequest, BridgeDiffSummary,
            BridgeHealthResponse, BridgeModelRef, BridgePermissionResponseRequest,
            BridgePermissionResponseSummary, BridgePromptRequest, BridgePromptSummary,
            BridgeRevertRequest, BridgeRevertSummary, BridgeSessionDiffQuery,
            BridgeSessionMessageRaw, BridgeSessionSummary, BridgeUnrevertSummary,
        },
        events::{
            normalize_upstream_message_to_snapshot, HostOpencodeEvent, HostOpencodeMessageList,
        },
        runtime::{
            managed_agent_config_summary, managed_agent_runtime_status, managed_agent_skill_status,
            start_managed_agent, stop_managed_agent, sync_managed_agent_skill,
        },
        StartManagedOpencodeRequest,
    },
    mei_agent::{
        agent_abort_session, agent_create_session, agent_health, agent_list_sessions,
        agent_project_worktree, agent_respond_permission, agent_revert_session, agent_send_prompt,
        agent_session_diff, agent_session_messages, agent_unrevert_session, agent_vcs_summary,
        llm_config,
        mode_policy::AgentModePolicy,
        native::{encode_host_event_line, filter_session_event},
        resolve_agent_conn, sanitize_relative_path,
    },
    AppState,
};

use super::super::error_response;
use super::super::scene_api::{
    build_world_context_snapshot_cached, default_resource_query_tools, WorldScope,
    RESOURCE_QUERY_SCHEMA_VERSION,
};
use super::permissions::{
    collect_and_reject_blocked_permissions, normalize_session_messages_limit,
    HostBlockedPermissionList, SessionMessagesQuery,
};
use super::prompt_context::{
    build_dynamic_session_context_preview, enrich_prompt_request, load_or_refresh_session_context,
};
use super::sse::sse_session_status_notice;

#[derive(Debug, Deserialize)]
pub struct OpencodeModelProbeQuery {
    #[serde(default, alias = "providerID")]
    pub provider_id: Option<String>,
    #[serde(default, alias = "modelID")]
    pub model_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OpencodeModelProbeResponse {
    pub reachable: bool,
    pub provider_id: String,
    pub model_id: String,
    pub base_url: String,
    #[serde(default)]
    pub latency_ms: Option<u128>,
    #[serde(default)]
    pub status_code: Option<u16>,
    #[serde(default)]
    pub error: Option<String>,
}

pub async fn api_agent_config(State(state): State<AppState>) -> Response {
    Json(managed_agent_config_summary(&state)).into_response()
}

pub async fn api_agent_runtime(State(state): State<AppState>) -> Response {
    match managed_agent_runtime_status(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_skill(State(state): State<AppState>) -> Response {
    match managed_agent_skill_status(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_sync_skill(State(state): State<AppState>) -> Response {
    match sync_managed_agent_skill(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_health(State(state): State<AppState>) -> Response {
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

    let conn =
        match resolve_agent_conn(&state) {
            Ok(c) => c,
            Err(_) => return Json(BridgeHealthResponse {
                server_url: String::new(),
                healthy: false,
                version: String::new(),
                expected_worktree: Some(state.source_root.display().to_string()),
                project_worktree: None,
                vcs_detected: false,
                vcs_branch: None,
                history_available: false,
                history_reason: Some(
                    "内置助手未初始化；Undo/Redo 与自动刷新依赖正确的 worktree 和 Git/VCS 视角。"
                        .to_string(),
                ),
            })
            .into_response(),
        };
    match agent_health(&state, &conn).await {
        Ok(mut status) => {
            let expected_worktree = state.source_root.display().to_string();
            status.expected_worktree = Some(expected_worktree.clone());
            match agent_project_worktree(&state, &conn).await {
                Ok(project_worktree) => status.project_worktree = project_worktree,
                Err(error) => {
                    status.history_available = false;
                    status.history_reason = Some(format!("无法读取当前 worktree：{error}"));
                    return Json(status).into_response();
                }
            }
            match agent_vcs_summary(&state, &conn).await {
                Ok((vcs_detected, vcs_branch)) => {
                    status.vcs_detected = vcs_detected;
                    status.vcs_branch = vcs_branch;
                }
                Err(error) => {
                    status.history_available = false;
                    status.history_reason = Some(format!("无法读取 VCS 状态：{error}"));
                    return Json(status).into_response();
                }
            }
            let project_matches = status
                .project_worktree
                .as_deref()
                .is_some_and(|value| worktree_matches_expected(value, &expected_worktree));
            if !status.healthy {
                status.history_available = false;
                if status.history_reason.is_none() {
                    status.history_reason =
                        Some("内置助手未就绪；Undo/Redo 与自动刷新当前不可用。".to_string());
                }
            } else if !project_matches {
                status.history_available = false;
                status.history_reason = Some(format!(
                    "当前 worktree 为 {}，而 MeiLang 预期工作区为 {}；Undo/Redo 与自动刷新不可用。",
                    status.project_worktree.as_deref().unwrap_or("(unknown)"),
                    expected_worktree
                ));
            } else if !status.vcs_detected {
                status.history_available = false;
                status.history_reason = Some(
                    "当前 worktree 未检测到 Git/VCS；Undo/Redo 与自动刷新不可用。".to_string(),
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

pub async fn api_agent_model_probe(Query(query): Query<OpencodeModelProbeQuery>) -> Response {
    let provider_id = query
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let model_id = query
        .model_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let model_ref = if provider_id.is_some() || model_id.is_some() {
        Some(BridgeModelRef {
            provider_id: provider_id.clone().unwrap_or_default(),
            model_id: model_id.clone().unwrap_or_default(),
        })
    } else {
        None
    };
    let conn = match llm_config::resolve_llm(model_ref.as_ref()) {
        Ok(value) => value,
        Err(error) => {
            return Json(OpencodeModelProbeResponse {
                reachable: false,
                provider_id: provider_id.unwrap_or_else(llm_config::default_provider_id_for_ui),
                model_id: model_id.unwrap_or_default(),
                base_url: String::new(),
                latency_ms: None,
                status_code: None,
                error: Some(error.to_string()),
            })
            .into_response();
        }
    };

    let models_url = format!("{}/models", conn.base_url.trim_end_matches('/'));
    let start = Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build();
    let Ok(client) = client else {
        return Json(OpencodeModelProbeResponse {
            reachable: false,
            provider_id: provider_id.unwrap_or_else(llm_config::default_provider_id_for_ui),
            model_id: conn.model.clone(),
            base_url: conn.base_url.clone(),
            latency_ms: None,
            status_code: None,
            error: Some("failed to initialize probe client".to_string()),
        })
        .into_response();
    };

    let result = client
        .get(&models_url)
        .header("Authorization", format!("Bearer {}", conn.api_key))
        .header("Content-Type", "application/json")
        .send()
        .await;
    match result {
        Ok(response) => {
            let status = response.status();
            Json(OpencodeModelProbeResponse {
                reachable: status.is_success(),
                provider_id: provider_id.unwrap_or_else(llm_config::default_provider_id_for_ui),
                model_id: conn.model.clone(),
                base_url: conn.base_url.clone(),
                latency_ms: Some(start.elapsed().as_millis()),
                status_code: Some(status.as_u16()),
                error: (!status.is_success()).then(|| format!("probe status {}", status.as_u16())),
            })
            .into_response()
        }
        Err(error) => Json(OpencodeModelProbeResponse {
            reachable: false,
            provider_id: provider_id.unwrap_or_else(llm_config::default_provider_id_for_ui),
            model_id: conn.model.clone(),
            base_url: conn.base_url.clone(),
            latency_ms: Some(start.elapsed().as_millis()),
            status_code: None,
            error: Some(error.to_string()),
        })
        .into_response(),
    }
}

pub async fn api_agent_start(
    State(state): State<AppState>,
    Json(request): Json<StartManagedOpencodeRequest>,
) -> Response {
    match start_managed_agent(&state, request).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_stop(State(state): State<AppState>) -> Response {
    match stop_managed_agent(&state) {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

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

#[derive(Debug, Deserialize)]
pub struct OpencodeContextPreviewQuery {
    pub app_id: String,
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub target_file: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default, alias = "routeMode")]
    pub route_mode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OpencodeContextPreviewResponse {
    pub app_id: String,
    #[serde(default)]
    pub scene_id: Option<String>,
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

pub async fn api_agent_context_preview(
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
        target_file: query.target_file.clone(),
        system: None,
        mode: query.mode.clone(),
        route_mode: query.route_mode.clone(),
        agent: None,
        model: None,
    };
    let policy = AgentModePolicy::from_request(&request);
    if let Err(error) = policy.validate() {
        return error_response(error);
    }
    policy.apply_to_request(&mut request);
    let scope = WorldScope {
        scene_id: query.scene_id.clone(),
        target_file: query.target_file.clone(),
    };
    let snapshot_result = build_world_context_snapshot_cached(&state, app_id, Some(&scope));
    let preview_error = snapshot_result.as_ref().err().map(|error| {
        // 上下文预览属于辅助信息，不应因为 scope 不匹配/编译中间态持续返回 500。
        tracing::debug!(
            app_id = %app_id,
            scene_id = ?query.scene_id,
            target_file = ?query.target_file,
            %error,
            "degraded context preview snapshot"
        );
        error.to_string()
    });
    let session_context = build_dynamic_session_context_preview(
        &state,
        &request,
        snapshot_result.as_ref().ok(),
        preview_error.as_deref(),
    )
    .unwrap_or_else(|| String::new());
    request = enrich_prompt_request(&state, Some(&session_context), request);
    let (tools, resource_inventory) = match snapshot_result {
        Ok(snapshot) => {
            let tools = if snapshot.query_tools.is_empty() {
                default_resource_query_tools()
            } else {
                snapshot.query_tools.clone()
            };
            (
                tools,
                serde_json::to_value(snapshot.resource_inventory).unwrap_or(Value::Null),
            )
        }
        Err(_) => (default_resource_query_tools(), Value::Null),
    };
    let query_tools = tools
        .into_iter()
        .map(|item| serde_json::to_value(item).unwrap_or(Value::Null))
        .collect::<Vec<_>>();
    let skill_status = crate::agent_runtime::runtime::managed_agent_skill_status(&state)
        .ok()
        .and_then(|item| serde_json::to_value(item).ok());
    Json(OpencodeContextPreviewResponse {
        app_id: app_id.to_string(),
        scene_id: query.scene_id,
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

pub async fn api_agent_send_message(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(mut request): Json<BridgePromptRequest>,
) -> Response {
    let conn = match resolve_agent_conn(&state) {
        Ok(c) => c,
        Err(error) => return error_response(error),
    };
    let policy = AgentModePolicy::from_request(&request);
    if let Err(error) = policy.validate() {
        return error_response(error);
    }
    policy.apply_to_request(&mut request);
    let session_context = load_or_refresh_session_context(&state, &session_id, &request);
    let request = enrich_prompt_request(&state, session_context.as_deref(), request);
    match agent_send_prompt(&state, &conn, &session_id, request).await {
        Ok(summary) => Json::<BridgePromptSummary>(summary).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn api_agent_session_events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    use axum::body::Body;
    use axum::http::{header::CONTENT_TYPE, HeaderValue};

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
