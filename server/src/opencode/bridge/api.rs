use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

use super::summarize::{
    prompt_body, summarize_diff, summarize_prompt_response, summarize_session,
};
use super::types::{
    BridgeAbortSummary, BridgeCreateSessionRequest, BridgeDiffSummary, BridgeHealthResponse,
    BridgePendingPermission, BridgePermissionResponseRequest, BridgePermissionResponseSummary,
    BridgePromptRequest, BridgePromptSummary, BridgeRevertRequest, BridgeRevertSummary,
    BridgeSessionMessageRaw, BridgeSessionSummary, BridgeUnrevertSummary,
};
use super::upstream::{
    UpstreamFileDiff, UpstreamHealth, UpstreamProjectCurrent, UpstreamPromptResponse,
    UpstreamSession,
};
use super::wire::{
    decode_applied_response, get_json, get_json_with_timeout, normalize_server_url, post_json,
};

pub(crate) async fn health(client: &Client, server_url: &str) -> Result<BridgeHealthResponse> {
    let server_url = normalize_server_url(server_url);
    let upstream =
        get_json::<UpstreamHealth>(client, &format!("{server_url}/global/health")).await?;
    Ok(BridgeHealthResponse {
        server_url,
        healthy: upstream.healthy,
        version: upstream.version,
        expected_worktree: None,
        project_worktree: None,
        vcs_detected: false,
        vcs_branch: None,
        history_available: false,
        history_reason: None,
    })
}

pub(crate) async fn project_current_worktree(
    client: &Client,
    server_url: &str,
) -> Result<Option<String>> {
    let server_url = normalize_server_url(server_url);
    let upstream =
        get_json::<UpstreamProjectCurrent>(client, &format!("{server_url}/project/current"))
            .await?;
    Ok(upstream
        .worktree
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

pub(crate) async fn vcs_summary(
    client: &Client,
    server_url: &str,
) -> Result<(bool, Option<String>)> {
    let server_url = normalize_server_url(server_url);
    let upstream = get_json::<Value>(client, &format!("{server_url}/vcs")).await?;
    let detected = upstream
        .as_object()
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    let branch = upstream
        .get("branch")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Ok((detected, branch))
}

pub(crate) async fn create_session(
    client: &Client,
    server_url: &str,
    request: BridgeCreateSessionRequest,
) -> Result<BridgeSessionSummary> {
    let server_url = normalize_server_url(server_url);
    let mut body = serde_json::Map::new();
    if let Some(title) = request.title {
        body.insert("title".to_string(), Value::String(title));
    }
    if let Some(parent_id) = request.parent_id {
        body.insert("parentID".to_string(), Value::String(parent_id));
    }
    let upstream = post_json::<UpstreamSession>(
        client,
        &format!("{server_url}/session"),
        Value::Object(body),
    )
    .await?;
    Ok(summarize_session(upstream))
}

pub(crate) async fn list_sessions(
    client: &Client,
    server_url: &str,
) -> Result<Vec<BridgeSessionSummary>> {
    let server_url = normalize_server_url(server_url);
    let upstream = get_json_with_timeout::<Vec<UpstreamSession>>(
        client,
        &format!("{server_url}/session"),
        Duration::from_secs(5),
    )
    .await?;
    Ok(upstream
        .into_iter()
        .map(summarize_session)
        .collect::<Vec<_>>())
}

pub(crate) async fn list_pending_permissions(
    client: &Client,
    server_url: &str,
) -> Result<Vec<BridgePendingPermission>> {
    let server_url = normalize_server_url(server_url);
    get_json_with_timeout::<Vec<BridgePendingPermission>>(
        client,
        &format!("{server_url}/permission"),
        Duration::from_secs(5),
    )
    .await
}

pub(crate) async fn send_prompt(
    client: &Client,
    server_url: &str,
    session_id: &str,
    request: BridgePromptRequest,
) -> Result<BridgePromptSummary> {
    let server_url = normalize_server_url(server_url);
    let upstream = post_json::<UpstreamPromptResponse>(
        client,
        &format!("{server_url}/session/{session_id}/message"),
        prompt_body(request),
    )
    .await?;
    Ok(summarize_prompt_response(upstream))
}

pub(crate) async fn session_messages(
    client: &Client,
    server_url: &str,
    session_id: &str,
) -> Result<Vec<BridgeSessionMessageRaw>> {
    let server_url = normalize_server_url(server_url);
    get_json_with_timeout::<Vec<BridgeSessionMessageRaw>>(
        client,
        &format!("{server_url}/session/{session_id}/message"),
        Duration::from_secs(6),
    )
    .await
}

pub(crate) async fn session_diff(
    client: &Client,
    server_url: &str,
    session_id: &str,
    message_id: Option<&str>,
) -> Result<BridgeDiffSummary> {
    let server_url = normalize_server_url(server_url);
    let mut url = format!("{server_url}/session/{session_id}/diff");
    let requested_message_id = message_id.map(str::trim).filter(|value| !value.is_empty());
    if let Some(mid) = requested_message_id {
        url.push_str("?messageID=");
        url.push_str(mid);
    }
    let upstream = get_json::<Vec<UpstreamFileDiff>>(client, &url).await?;
    Ok(summarize_diff(
        session_id,
        requested_message_id.map(ToString::to_string),
        upstream,
    ))
}

pub(crate) async fn abort_session(
    client: &Client,
    server_url: &str,
    session_id: &str,
) -> Result<BridgeAbortSummary> {
    let server_url = normalize_server_url(server_url);
    let aborted = post_json::<bool>(
        client,
        &format!("{server_url}/session/{session_id}/abort"),
        json!({}),
    )
    .await?;
    Ok(BridgeAbortSummary {
        session_id: session_id.to_string(),
        aborted,
    })
}

pub(crate) async fn revert_session_message(
    client: &Client,
    server_url: &str,
    session_id: &str,
    request: BridgeRevertRequest,
) -> Result<BridgeRevertSummary> {
    let message_id = request.message_id.trim().to_string();
    if message_id.is_empty() {
        anyhow::bail!("message_id is required");
    }
    let part_id = request
        .part_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let server_url = normalize_server_url(server_url);
    let mut body = json!({ "messageID": message_id });
    if let Some(part_id) = part_id.as_deref() {
        body["partID"] = Value::String(part_id.to_string());
    }
    let reverted = decode_applied_response(
        "revert",
        post_json::<Value>(
            client,
            &format!("{server_url}/session/{session_id}/revert"),
            body,
        )
        .await?,
    )?;
    Ok(BridgeRevertSummary {
        session_id: session_id.to_string(),
        message_id,
        part_id,
        reverted,
    })
}

pub(crate) async fn unrevert_session(
    client: &Client,
    server_url: &str,
    session_id: &str,
) -> Result<BridgeUnrevertSummary> {
    let server_url = normalize_server_url(server_url);
    let restored = decode_applied_response(
        "unrevert",
        post_json::<Value>(
            client,
            &format!("{server_url}/session/{session_id}/unrevert"),
            json!({}),
        )
        .await?,
    )?;
    Ok(BridgeUnrevertSummary {
        session_id: session_id.to_string(),
        restored,
    })
}

pub(crate) async fn respond_permission(
    client: &Client,
    server_url: &str,
    session_id: &str,
    permission_id: &str,
    request: BridgePermissionResponseRequest,
) -> Result<BridgePermissionResponseSummary> {
    let response = request.response.trim().to_string();
    match response.as_str() {
        "once" | "always" | "reject" => {}
        _ => anyhow::bail!("unsupported permission response: {response}"),
    }
    let server_url = normalize_server_url(server_url);
    let applied = post_json::<bool>(
        client,
        &format!("{server_url}/session/{session_id}/permissions/{permission_id}"),
        json!({ "response": response }),
    )
    .await?;
    Ok(BridgePermissionResponseSummary {
        session_id: session_id.to_string(),
        permission_id: permission_id.to_string(),
        response,
        applied,
    })
}

pub(crate) async fn global_event(client: &Client, server_url: &str) -> Result<reqwest::Response> {
    let server_url = normalize_server_url(server_url);
    client
        .get(format!("{server_url}/global/event"))
        .send()
        .await
        .with_context(|| format!("failed to GET {server_url}/global/event"))?
        .error_for_status()
        .with_context(|| format!("GET {server_url}/global/event returned error status"))
}
