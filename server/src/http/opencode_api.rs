use std::{
    fs,
    path::{Component, Path as FsPath, PathBuf},
};

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header::CONTENT_TYPE, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::{
    opencode::{
        bridge::{
            abort_session as bridge_abort_session, create_session as bridge_create_session,
            global_event as bridge_global_event, health as bridge_health,
            list_sessions as bridge_list_sessions,
            list_pending_permissions as bridge_list_pending_permissions,
            project_current_worktree as bridge_project_current_worktree,
            respond_permission as bridge_respond_permission,
            revert_session_message as bridge_revert_session_message,
            send_prompt as bridge_send_prompt, session_diff as bridge_session_diff,
            session_messages as bridge_session_messages,
            unrevert_session as bridge_unrevert_session, vcs_summary as bridge_vcs_summary,
            BridgeCreateSessionRequest, BridgeHealthResponse, BridgePendingPermission,
            BridgePermissionResponseRequest, BridgePromptRequest, BridgeRevertRequest,
            BridgeSessionDiffQuery, BridgeSessionSummary,
        },
        events::{
            extract_sse_data, looks_like_meilang_skill_path, normalize_global_event_to_host_event,
            normalize_upstream_message_to_snapshot, HostOpencodeEvent, HostOpencodeMessageList,
        },
        runtime::{
            load_managed_opencode_skill_prompt, managed_opencode_config_summary,
            managed_opencode_runtime_status, managed_opencode_server_url,
            managed_opencode_skill_status, start_managed_opencode, stop_managed_opencode,
            sync_managed_opencode_skill,
        },
        StartManagedOpencodeRequest,
    },
    AppState, SessionContextSnapshot,
};

use super::error_response;

#[derive(Debug, Deserialize)]
pub struct SessionMessagesQuery {
    limit: Option<usize>,
}

const DEFAULT_SESSION_MESSAGES_LIMIT: usize = 80;
const MAX_SESSION_MESSAGES_LIMIT: usize = 300;

#[derive(Debug, Clone, Serialize)]
struct HostBlockedPermissionNotice {
    permission_id: String,
    permission: String,
    path: Option<String>,
    patterns: Vec<String>,
    requires_admin: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct HostBlockedPermissionList {
    session_id: String,
    pending: Vec<HostBlockedPermissionNotice>,
}

fn normalize_session_messages_limit(limit: Option<usize>) -> usize {
    let resolved = limit.unwrap_or(DEFAULT_SESSION_MESSAGES_LIMIT);
    resolved.clamp(1, MAX_SESSION_MESSAGES_LIMIT)
}

fn classify_blocked_permission(
    permission: &str,
    patterns: &[String],
) -> (Option<String>, bool, String) {
    let path = patterns
        .iter()
        .map(String::as_str)
        .find(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string());
    if permission == "external_directory" {
        let all_skill = !patterns.is_empty()
            && patterns
                .iter()
                .all(|pattern| looks_like_meilang_skill_path(pattern));
        if all_skill {
            return (
                path,
                true,
                "系统尝试读取 MeiLang skill 目录，但当前 OpenCode 白名单未生效；请联系管理员检查权限配置。"
                    .to_string(),
            );
        }
        return (
            path,
            true,
            "你尝试访问了未授权的文件夹。请检查任务路径是否正确；若这是系统预期目录，请联系管理员加入白名单。"
                .to_string(),
        );
    }
    (
        path,
        true,
        format!(
            "触发了未支持的运行时授权请求（permission={permission}）。请联系管理员检查策略。"
        ),
    )
}

fn blocked_notice_from_pending(item: BridgePendingPermission) -> HostBlockedPermissionNotice {
    let (path, requires_admin, message) = classify_blocked_permission(&item.permission, &item.patterns);
    HostBlockedPermissionNotice {
        permission_id: item.id,
        permission: item.permission,
        path,
        patterns: item.patterns,
        requires_admin,
        message,
    }
}

async fn collect_and_reject_blocked_permissions(
    state: &AppState,
    server_url: &str,
    session_id: &str,
) -> anyhow::Result<Vec<HostBlockedPermissionNotice>> {
    let items = bridge_list_pending_permissions(&state.opencode_http, server_url).await?;
    let mut notices = Vec::new();
    for item in items.into_iter().filter(|item| item.session_id == session_id) {
        let permission_id = item.id.trim().to_string();
        let mut notice = blocked_notice_from_pending(item);
        if !permission_id.is_empty() {
            match bridge_respond_permission(
                &state.opencode_http,
                server_url,
                session_id,
                &permission_id,
                BridgePermissionResponseRequest {
                    response: "reject".to_string(),
                },
            )
            .await
            {
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(
                        session_id = %session_id,
                        permission_id = %permission_id,
                        %error,
                        "failed to auto-reject pending opencode permission"
                    );
                    notice.message = format!("{}（自动拒绝失败：{}）", notice.message, error);
                }
            }
        }
        notices.push(notice);
    }
    Ok(notices)
}

fn sanitize_relative_path(value: &str) -> Option<String> {
    let mut parts = Vec::new();
    for component in FsPath::new(value).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn resolve_app_root(state: &AppState, request: &BridgePromptRequest) -> Option<(String, PathBuf)> {
    let app_id = request.app_id.as_deref()?.trim();
    if app_id.is_empty() {
        return None;
    }
    let root = state.source_root.join(app_id);
    if !root.exists() {
        return None;
    }
    Some((app_id.to_string(), root))
}

fn collect_mei_files(app_root: &FsPath) -> Vec<String> {
    let mut files = WalkDir::new(app_root)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("mei"))
        .filter_map(|entry| {
            entry
                .path()
                .strip_prefix(app_root)
                .ok()
                .and_then(|path| path.to_str())
                .map(|path| path.replace('\\', "/"))
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn current_target_path(
    app_root: &FsPath,
    request: &BridgePromptRequest,
) -> Option<(String, PathBuf)> {
    let target = sanitize_relative_path(request.target_file.as_deref()?.trim())?;
    let path = app_root.join(&target);
    if !path.exists() || !path.is_file() {
        return None;
    }
    Some((target, path))
}

fn build_dynamic_mei_context(state: &AppState, request: &BridgePromptRequest) -> Option<String> {
    let (app_id, app_root) = resolve_app_root(state, request)?;
    let entry_id = request
        .entry_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let mei_files = collect_mei_files(&app_root);
    let target_context = current_target_path(&app_root, request).and_then(|(target, path)| {
        fs::read_to_string(&path)
            .ok()
            .map(|content| (target, content))
    });
    let mut lines = vec![
        "[MeiLang Runtime Context]".to_string(),
        format!("app: {app_id}"),
        format!("entry: {entry_id}"),
    ];
    if let Some((target, _)) = &target_context {
        lines.push(format!("target: {target}"));
    } else if let Some(target) = request.target_file.as_deref() {
        lines.push(format!("target: {}", target.trim()));
    }
    lines.push(
        "language: MeiLang .mei (Starlark-hosted DSL), not Music Encoding Initiative XML"
            .to_string(),
    );
    lines.push(String::new());
    lines.push("[Application Mei Files]".to_string());
    if mei_files.is_empty() {
        lines.push("- (none)".to_string());
    } else {
        lines.extend(mei_files.iter().map(|item| format!("- {item}")));
    }
    if let Some((target, content)) = target_context {
        lines.push(String::new());
        lines.push(format!("[Current File: {target}]"));
        lines.push("```mei".to_string());
        lines.push(content);
        lines.push("```".to_string());
    }
    Some(lines.join("\n"))
}

fn build_context_signature(request: &BridgePromptRequest) -> Option<String> {
    let app_id = request.app_id.as_deref()?.trim();
    if app_id.is_empty() {
        return None;
    }
    let entry_id = request.entry_id.as_deref().map(str::trim).unwrap_or("");
    let target_file = request.target_file.as_deref().map(str::trim).unwrap_or("");
    Some(format!(
        "app={app_id}|entry={entry_id}|target={target_file}"
    ))
}

fn load_or_refresh_session_context(
    state: &AppState,
    session_id: &str,
    request: &BridgePromptRequest,
) -> Option<String> {
    let signature = build_context_signature(request)?;
    {
        let Ok(cache) = state.opencode_session_context.lock() else {
            tracing::warn!("opencode session context cache lock poisoned; fallback to rebuild");
            return build_dynamic_mei_context(state, request);
        };
        if let Some(snapshot) = cache.get(session_id) {
            if snapshot.signature == signature {
                return Some(snapshot.context.clone());
            }
        }
    }
    let context = build_dynamic_mei_context(state, request)?;
    let Ok(mut cache) = state.opencode_session_context.lock() else {
        tracing::warn!("opencode session context cache lock poisoned; skip cache write");
        return Some(context);
    };
    cache.insert(
        session_id.to_string(),
        SessionContextSnapshot {
            signature,
            context: context.clone(),
        },
    );
    Some(context)
}

fn build_meilang_system_prompt(
    state: &AppState,
    existing: Option<&str>,
    session_context: Option<&str>,
) -> Option<String> {
    let mut blocks = Vec::new();
    if let Some(system) = existing.map(str::trim).filter(|value| !value.is_empty()) {
        blocks.push(system.to_string());
    }
    blocks.push(
        "You are a MeiLang authoring assistant. Treat `.mei` as MeiLang scene-first DSL hosted on restricted Starlark, not Music Encoding Initiative XML.".to_string(),
    );
    blocks.push(
        "Prefer declarative bindings: app(entries=[entry(...)]), scene(world/flow/frame=...), world(id=...), flow(id=...), frame(id=...), frame.add_panel(...).".to_string(),
    );
    blocks.push(
        "Default to Chinese (Simplified Chinese) for all responses, plans, progress updates, and explanations unless the user explicitly requests another language.".to_string(),
    );
    blocks.push(
        "When presenting a plan, keep the execution-oriented content in Chinese and avoid switching to English by default.".to_string(),
    );
    match load_managed_opencode_skill_prompt(state) {
        Ok(Some(skill_prompt)) => {
            let mut block = String::new();
            block.push_str("[MeiLang Claude Skill Entry]\n");
            block.push_str(&skill_prompt.entry_markdown);
            block.push_str("\n\n[Skill Home]\n");
            block.push_str(&format!(
                "source_kind: {}\npath: {}",
                skill_prompt.source_kind, skill_prompt.skill_home
            ));
            block.push_str(
                "\n\n[Important]\nCompanion files are relative to skill_home. Resolve them as `skill_home/<file>` before reading.",
            );
            if !skill_prompt.companion_files.is_empty() {
                block.push_str("\n\n[Companion Files]\n");
                for item in skill_prompt.companion_files {
                    block.push_str(&format!("- rel: {item}\n"));
                }
            }
            blocks.push(block.trim().to_string());
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(%error, "failed to load mei-lang skill prompt");
        }
    }
    if let Some(context) = session_context {
        blocks.push(format!("[MeiLang Session Context]\n{context}"));
    }
    if blocks.is_empty() {
        None
    } else {
        Some(blocks.join("\n\n"))
    }
}

fn enrich_prompt_request(
    state: &AppState,
    session_context: Option<&str>,
    mut request: BridgePromptRequest,
) -> BridgePromptRequest {
    let user_text = request.text.trim().to_string();
    request.text = user_text;
    request.system = build_meilang_system_prompt(state, request.system.as_deref(), session_context);
    request
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
                .map(normalize_path)
                .is_some_and(|value| value == normalize_path(&expected_worktree));
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

fn take_sse_frame(buffer: &mut String) -> Option<String> {
    let idx = buffer.find("\n\n")?;
    let frame = buffer[..idx].to_string();
    let rest = buffer[idx + 2..].to_string();
    *buffer = rest;
    Some(frame)
}

/// OpenCode 未启动或上游不可用时，仍返回 **200 + event-stream**，避免浏览器 EventSource 对非 2xx 无限重连，
/// 并由前端收到 `session_status` 后主动 `close()` 停止重连。
fn sse_session_status_notice(
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
        Ok(pending) => Json(HostBlockedPermissionList { session_id, pending }).into_response(),
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
