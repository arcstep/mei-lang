//! `/api/opencode/*` 仅对接内置 Native Agent（不再使用上游 OpenCode HTTP）。

use std::sync::Arc;

use anyhow::Result;

use crate::{
    opencode::bridge::{
        BridgeAbortSummary, BridgeCreateSessionRequest, BridgeDiffSummary, BridgeHealthResponse,
        BridgePendingPermission, BridgePermissionResponseRequest, BridgePermissionResponseSummary,
        BridgePromptRequest, BridgePromptSummary, BridgeRevertRequest, BridgeRevertSummary,
        BridgeSessionMessageRaw, BridgeSessionSummary, BridgeUnrevertSummary,
    },
    AppState,
};

use super::native::NativeAgent;

pub(crate) type AgentConn = Arc<NativeAgent>;

pub(crate) fn resolve_agent_conn(state: &AppState) -> Result<AgentConn> {
    Ok(state.native_agent.clone())
}

pub(crate) async fn agent_health(
    _state: &AppState,
    conn: &AgentConn,
) -> Result<BridgeHealthResponse> {
    Ok(conn.health_response())
}

pub(crate) async fn agent_project_worktree(
    _state: &AppState,
    conn: &AgentConn,
) -> Result<Option<String>> {
    Ok(Some(conn.worktree_string()))
}

pub(crate) async fn agent_vcs_summary(
    _state: &AppState,
    conn: &AgentConn,
) -> Result<(bool, Option<String>)> {
    let agent = conn.clone();
    Ok(
        tokio::task::spawn_blocking(move || agent.vcs_summary_blocking())
            .await
            .map_err(|e| anyhow::anyhow!("vcs: {e}"))?,
    )
}

pub(crate) async fn agent_create_session(
    _state: &AppState,
    conn: &AgentConn,
    request: BridgeCreateSessionRequest,
) -> Result<BridgeSessionSummary> {
    let agent = conn.clone();
    Ok(
        tokio::task::spawn_blocking(move || agent.create_session_blocking(request))
            .await
            .map_err(|e| anyhow::anyhow!("create session: {e}"))??,
    )
}

pub(crate) async fn agent_list_sessions(
    _state: &AppState,
    conn: &AgentConn,
) -> Result<Vec<BridgeSessionSummary>> {
    let agent = conn.clone();
    Ok(
        tokio::task::spawn_blocking(move || agent.list_sessions_blocking())
            .await
            .map_err(|e| anyhow::anyhow!("list sessions: {e}"))??,
    )
}

pub(crate) async fn agent_send_prompt(
    _state: &AppState,
    conn: &AgentConn,
    session_id: &str,
    request: BridgePromptRequest,
) -> Result<BridgePromptSummary> {
    conn.send_prompt(session_id, request).await
}

pub(crate) async fn agent_session_messages(
    _state: &AppState,
    conn: &AgentConn,
    session_id: &str,
) -> Result<Vec<BridgeSessionMessageRaw>> {
    let agent = conn.clone();
    let sid = session_id.to_string();
    Ok(
        tokio::task::spawn_blocking(move || agent.session_messages_blocking(&sid))
            .await
            .map_err(|e| anyhow::anyhow!("messages: {e}"))??,
    )
}

pub(crate) async fn agent_session_diff(
    _state: &AppState,
    conn: &AgentConn,
    session_id: &str,
    message_id: Option<&str>,
) -> Result<BridgeDiffSummary> {
    let agent = conn.clone();
    let sid = session_id.to_string();
    let mid = message_id.map(|s| s.to_string());
    Ok(
        tokio::task::spawn_blocking(move || agent.session_diff_blocking(&sid, mid.as_deref()))
            .await
            .map_err(|e| anyhow::anyhow!("diff: {e}"))??,
    )
}

pub(crate) async fn agent_abort_session(
    _state: &AppState,
    conn: &AgentConn,
    session_id: &str,
) -> Result<BridgeAbortSummary> {
    let agent = conn.clone();
    let sid = session_id.to_string();
    Ok(
        tokio::task::spawn_blocking(move || agent.abort_session_blocking(&sid))
            .await
            .map_err(|e| anyhow::anyhow!("abort: {e}"))??,
    )
}

pub(crate) async fn agent_revert_session(
    _state: &AppState,
    conn: &AgentConn,
    session_id: &str,
    request: BridgeRevertRequest,
) -> Result<BridgeRevertSummary> {
    let agent = conn.clone();
    let sid = session_id.to_string();
    Ok(
        tokio::task::spawn_blocking(move || agent.revert_blocking(&sid, &request))
            .await
            .map_err(|e| anyhow::anyhow!("revert: {e}"))??,
    )
}

pub(crate) async fn agent_unrevert_session(
    _state: &AppState,
    conn: &AgentConn,
    session_id: &str,
) -> Result<BridgeUnrevertSummary> {
    let agent = conn.clone();
    let sid = session_id.to_string();
    Ok(
        tokio::task::spawn_blocking(move || agent.unrevert_blocking(&sid))
            .await
            .map_err(|e| anyhow::anyhow!("unrevert: {e}"))??,
    )
}

pub(crate) async fn agent_list_pending_permissions(
    _state: &AppState,
    conn: &AgentConn,
) -> Result<Vec<BridgePendingPermission>> {
    let agent = conn.clone();
    Ok(
        tokio::task::spawn_blocking(move || agent.list_pending_permissions_blocking())
            .await
            .map_err(|e| anyhow::anyhow!("permissions: {e}"))?,
    )
}

pub(crate) async fn agent_respond_permission(
    _state: &AppState,
    conn: &AgentConn,
    session_id: &str,
    permission_id: &str,
    request: BridgePermissionResponseRequest,
) -> Result<BridgePermissionResponseSummary> {
    let agent = conn.clone();
    let sid = session_id.to_string();
    let pid = permission_id.to_string();
    let req = request.clone();
    Ok(
        tokio::task::spawn_blocking(move || agent.respond_permission_blocking(&sid, &pid, &req))
            .await
            .map_err(|e| anyhow::anyhow!("permission reply: {e}"))??,
    )
}
