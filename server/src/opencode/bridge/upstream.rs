use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamHealth {
    pub(super) healthy: bool,
    pub(super) version: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamProjectCurrent {
    #[serde(default)]
    pub(super) worktree: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamSessionTime {
    #[serde(default)]
    pub(super) created: Option<u64>,
    #[serde(default)]
    pub(super) updated: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct UpstreamSessionSummary {
    #[serde(default)]
    pub(super) additions: u64,
    #[serde(default)]
    pub(super) deletions: u64,
    #[serde(default)]
    pub(super) files: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamSession {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) directory: String,
    pub(super) time: UpstreamSessionTime,
    #[serde(default)]
    pub(super) summary: UpstreamSessionSummary,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamAssistantInfo {
    pub(super) id: String,
    #[serde(rename = "sessionID")]
    pub(super) session_id: String,
    #[serde(default, rename = "providerID")]
    pub(super) provider_id: Option<String>,
    #[serde(default, rename = "modelID")]
    pub(super) model_id: Option<String>,
    #[serde(default)]
    pub(super) finish: Option<String>,
    #[serde(default)]
    pub(super) error: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamPromptResponse {
    pub(super) info: UpstreamAssistantInfo,
    #[serde(default)]
    pub(super) parts: Vec<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpstreamFileDiff {
    pub(super) file: String,
    pub(super) before: String,
    pub(super) after: String,
    pub(super) additions: u64,
    pub(super) deletions: u64,
}
