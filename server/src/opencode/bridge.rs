use anyhow::{Context, Result};
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BridgeCreateSessionRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, alias = "parentID")]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BridgeModelRef {
    #[serde(alias = "providerID")]
    pub provider_id: String,
    #[serde(alias = "modelID")]
    pub model_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BridgePromptRequest {
    pub text: String,
    #[serde(default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub entry_id: Option<String>,
    #[serde(default)]
    pub target_file: Option<String>,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<BridgeModelRef>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct BridgeHealthResponse {
    pub server_url: String,
    pub healthy: bool,
    pub version: String,
    pub expected_worktree: Option<String>,
    pub project_worktree: Option<String>,
    pub vcs_detected: bool,
    pub vcs_branch: Option<String>,
    pub history_available: bool,
    pub history_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BridgeSessionSummary {
    pub id: String,
    pub title: String,
    pub directory: String,
    pub created_at_ms: Option<u64>,
    pub updated_at_ms: Option<u64>,
    pub additions: u64,
    pub deletions: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct BridgePromptSummary {
    pub session_id: String,
    pub message_id: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub finish: Option<String>,
    pub texts: Vec<String>,
    pub part_types: Vec<String>,
    pub error: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BridgeSessionMessageRaw {
    pub info: Value,
    #[serde(default)]
    pub parts: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BridgePermissionResponseRequest {
    pub response: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BridgePendingPermission {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub permission: String,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BridgeSessionDiffQuery {
    #[serde(default, alias = "messageID")]
    pub message_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BridgeRevertRequest {
    #[serde(alias = "messageID")]
    pub message_id: String,
    #[serde(default, alias = "partID")]
    pub part_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BridgePermissionResponseSummary {
    pub session_id: String,
    pub permission_id: String,
    pub response: String,
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BridgeAbortSummary {
    pub session_id: String,
    pub aborted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BridgeFileDiffSummary {
    pub file: String,
    pub additions: u64,
    pub deletions: u64,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BridgeDiffSummary {
    pub session_id: String,
    pub message_id: Option<String>,
    pub additions: u64,
    pub deletions: u64,
    pub files: Vec<BridgeFileDiffSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BridgeRevertSummary {
    pub session_id: String,
    pub message_id: String,
    pub part_id: Option<String>,
    pub reverted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BridgeUnrevertSummary {
    pub session_id: String,
    pub restored: bool,
}

#[derive(Debug, Deserialize)]
struct UpstreamHealth {
    healthy: bool,
    version: String,
}

#[derive(Debug, Deserialize)]
struct UpstreamProjectCurrent {
    #[serde(default)]
    worktree: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamSessionTime {
    #[serde(default)]
    created: Option<u64>,
    #[serde(default)]
    updated: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct UpstreamSessionSummary {
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
    #[serde(default)]
    files: u64,
}

#[derive(Debug, Deserialize)]
struct UpstreamSession {
    id: String,
    title: String,
    directory: String,
    time: UpstreamSessionTime,
    #[serde(default)]
    summary: UpstreamSessionSummary,
}

#[derive(Debug, Deserialize)]
struct UpstreamAssistantInfo {
    id: String,
    #[serde(rename = "sessionID")]
    session_id: String,
    #[serde(default, rename = "providerID")]
    provider_id: Option<String>,
    #[serde(default, rename = "modelID")]
    model_id: Option<String>,
    #[serde(default)]
    finish: Option<String>,
    #[serde(default)]
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct UpstreamPromptResponse {
    info: UpstreamAssistantInfo,
    #[serde(default)]
    parts: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct UpstreamFileDiff {
    file: String,
    before: String,
    after: String,
    additions: u64,
    deletions: u64,
}

fn normalize_server_url(server_url: &str) -> String {
    server_url.trim_end_matches('/').to_string()
}

async fn get_json<T: serde::de::DeserializeOwned>(client: &Client, url: &str) -> Result<T> {
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to GET {url}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("failed to read response body from {url}"))?;
    if !status.is_success() {
        anyhow::bail!("GET {url} returned {status}: {body}");
    }
    serde_json::from_str::<T>(&body)
        .with_context(|| format!("failed to decode JSON from {url}: {body}"))
}

async fn get_json_with_timeout<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    timeout: Duration,
) -> Result<T> {
    let response = client
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .with_context(|| format!("failed to GET {url}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("failed to read response body from {url}"))?;
    if !status.is_success() {
        anyhow::bail!("GET {url} returned {status}: {body}");
    }
    serde_json::from_str::<T>(&body)
        .with_context(|| format!("failed to decode JSON from {url}: {body}"))
}

async fn post_json<T: DeserializeOwned>(client: &Client, url: &str, body: Value) -> Result<T> {
    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("failed to POST {url}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("failed to read response body from {url}"))?;
    if !status.is_success() {
        anyhow::bail!("POST {url} returned {status}: {text}");
    }
    serde_json::from_str::<T>(&text)
        .with_context(|| format!("failed to decode JSON from {url}: {text}"))
}

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

fn summarize_session(session: UpstreamSession) -> BridgeSessionSummary {
    BridgeSessionSummary {
        id: session.id,
        title: session.title,
        directory: session.directory,
        created_at_ms: session.time.created,
        updated_at_ms: session.time.updated,
        additions: session.summary.additions,
        deletions: session.summary.deletions,
        files: session.summary.files,
    }
}

fn summarize_prompt_response(response: UpstreamPromptResponse) -> BridgePromptSummary {
    let mut texts = Vec::new();
    let mut part_types = Vec::new();
    for part in response.parts {
        if let Some(part_type) = part.get("type").and_then(Value::as_str) {
            part_types.push(part_type.to_string());
            if part_type == "text" {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    texts.push(text.to_string());
                }
            }
        }
    }
    BridgePromptSummary {
        session_id: response.info.session_id,
        message_id: response.info.id,
        provider_id: response.info.provider_id,
        model_id: response.info.model_id,
        finish: response.info.finish,
        texts,
        part_types,
        error: response.info.error,
    }
}

fn summarize_diff(
    session_id: &str,
    message_id: Option<String>,
    files: Vec<UpstreamFileDiff>,
) -> BridgeDiffSummary {
    let mut additions = 0;
    let mut deletions = 0;
    let files = files
        .into_iter()
        .map(|item| {
            additions += item.additions;
            deletions += item.deletions;
            BridgeFileDiffSummary {
                file: item.file,
                additions: item.additions,
                deletions: item.deletions,
                before: item.before,
                after: item.after,
            }
        })
        .collect::<Vec<_>>();
    BridgeDiffSummary {
        session_id: session_id.to_string(),
        message_id,
        additions,
        deletions,
        files,
    }
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

fn prompt_body(request: BridgePromptRequest) -> Value {
    let mut body = json!({
        "parts": [{
            "type": "text",
            "text": request.text,
        }]
    });
    if let Some(system) = request.system {
        body["system"] = Value::String(system);
    }
    if let Some(agent) = request.agent {
        body["agent"] = Value::String(agent);
    }
    if let Some(model) = request.model {
        body["model"] = json!({
            "providerID": model.provider_id,
            "modelID": model.model_id,
        });
    }
    body
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
    let reverted = post_json::<bool>(
        client,
        &format!("{server_url}/session/{session_id}/revert"),
        body,
    )
    .await?;
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
    let restored = post_json::<bool>(
        client,
        &format!("{server_url}/session/{session_id}/unrevert"),
        json!({}),
    )
    .await?;
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
