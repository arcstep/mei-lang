use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub scene_id: Option<String>,
    #[serde(default)]
    pub target_file: Option<String>,
    #[serde(default)]
    pub system: Option<String>,
    /// 新模式字段：`ask` / `build`。迁移期可与 `agent` 并存。
    #[serde(default)]
    pub mode: Option<String>,
    /// 页面路由模式：`manage` / `access`（由前端显式传入，后端做强约束）。
    #[serde(default, alias = "routeMode")]
    pub route_mode: Option<String>,
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
    /// 相对工作区根的路径；与 `send_prompt` 的 `target_file` 一致时，diff 仅包含该文件（及旧快照在内存中过滤）。
    #[serde(default)]
    pub path: Option<String>,
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
