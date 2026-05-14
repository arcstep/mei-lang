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
use serde::Deserialize;
use serde_json::{json, Value};
use walkdir::WalkDir;

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
            load_managed_opencode_skill_prompt, managed_opencode_server_url,
            managed_opencode_skill_status,
            start_managed_opencode, stop_managed_opencode, sync_managed_opencode_skill,
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

fn normalize_session_messages_limit(limit: Option<usize>) -> usize {
    let resolved = limit.unwrap_or(DEFAULT_SESSION_MESSAGES_LIMIT);
    resolved.clamp(1, MAX_SESSION_MESSAGES_LIMIT)
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
    lines.push("language: MeiLang .mei (Starlark-hosted DSL), not Music Encoding Initiative XML".to_string());
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
    Some(format!("app={app_id}|entry={entry_id}|target={target_file}"))
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
    request.system = build_meilang_system_prompt(
        state,
        request.system.as_deref(),
        session_context,
    );
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
