use mei_lang_kernel::{RuntimeIntent, RuntimeSceneView, RuntimeState, RuntimeTraceItem};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct SimStepRequest {
    #[serde(default)]
    pub state: Option<RuntimeState>,
    pub intent: RuntimeIntent,
}

#[derive(Debug, Serialize)]
pub struct SimStepResponse {
    pub state: RuntimeState,
    pub scene_view: RuntimeSceneView,
    #[serde(default)]
    pub trace_delta: Vec<RuntimeTraceItem>,
    pub html: String,
}

#[derive(Debug, Clone)]
pub(crate) struct WorldRuntimeBundle {
    pub(crate) entry_target: String,
    pub(crate) contract: mei_lang_kernel::SceneContract,
    pub(crate) state: RuntimeState,
    pub(crate) scene_view: RuntimeSceneView,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WorldScope {
    pub scene_id: Option<String>,
    pub entry_id: Option<String>,
    pub target_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldQueryCapabilitySummary {
    pub id: String,
    pub status: String,
    pub purpose: String,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldRuntimeSummary {
    pub phase: String,
    pub result: String,
    pub countdown: i64,
    pub scene_view_entities: usize,
    pub scene_view_cells: usize,
    pub available_actions: Vec<String>,
    pub recent_trace_messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldSnapshotSummary {
    pub scene_id: String,
    pub world_id: Option<String>,
    pub world_resource_count: usize,
    pub world_entity_count: usize,
    pub world_topology: Option<String>,
    pub world_resource_kind_counts: BTreeMap<String, usize>,
    pub world_entity_kind_counts: BTreeMap<String, usize>,
    pub world_key_resource_ids: Vec<String>,
    pub world_key_entity_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldContextSnapshot {
    pub app_id: String,
    pub entry_target: String,
    pub world_snapshot: WorldSnapshotSummary,
    pub runtime_summary: WorldRuntimeSummary,
    pub query_capabilities: Vec<WorldQueryCapabilitySummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldAssetListItem {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldAssetListResponse {
    pub app_id: String,
    pub scene_id: String,
    pub query_kind: String,
    pub total: usize,
    pub items: Vec<WorldAssetListItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldAssetGetResponse {
    pub app_id: String,
    pub scene_id: String,
    pub id: String,
    pub kind: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldRuntimePeekResponse {
    pub app_id: String,
    pub scene_id: String,
    pub phase: String,
    pub result: String,
    pub countdown: i64,
    pub available_actions: Vec<String>,
    pub recent_trace_messages: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct WorldAssetListQuery {
    #[serde(flatten)]
    pub scope: WorldScopeQuery,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct WorldAssetGetQuery {
    #[serde(flatten)]
    pub scope: WorldScopeQuery,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct WorldRuntimePeekQuery {
    #[serde(flatten)]
    pub scope: WorldScopeQuery,
    #[serde(default)]
    pub trace_limit: Option<usize>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct WorldScopeQuery {
    #[serde(default)]
    pub scene_id: Option<String>,
    #[serde(default)]
    pub entry_id: Option<String>,
    #[serde(default)]
    pub target_file: Option<String>,
}

impl WorldScopeQuery {
    pub(crate) fn to_scope(&self) -> WorldScope {
        WorldScope {
            scene_id: self.scene_id.clone(),
            entry_id: self.entry_id.clone(),
            target_file: self.target_file.clone(),
        }
    }
}
