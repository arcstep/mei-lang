use std::collections::BTreeMap;

use mei_lang_kernel::{CompiledApp, RuntimeSceneView, RuntimeState, SceneContract};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct WorldRuntimeBundle {
    pub compiled: CompiledApp,
    pub active_target_file: String,
    pub contract: SceneContract,
    pub state: RuntimeState,
    pub scene_view: RuntimeSceneView,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct WorldScope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQueryToolSpec {
    pub id: String,
    pub status: String,
    pub purpose: String,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInventoryItem {
    pub id: String,
    pub resource_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    pub related_to_target: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInventorySnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    pub total_items: usize,
    pub items: Vec<ResourceInventoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldRuntimeSummary {
    pub phase: String,
    pub result: String,
    pub countdown: i64,
    pub scene_view_entities: usize,
    pub scene_view_cells: usize,
    pub available_actions: Vec<String>,
    pub recent_trace_messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshotSummary {
    pub scene_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_id: Option<String>,
    pub world_resource_count: usize,
    pub world_entity_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_topology: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub world_resource_kind_counts: BTreeMap<String, usize>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub world_entity_kind_counts: BTreeMap<String, usize>,
    pub world_key_resource_ids: Vec<String>,
    pub world_key_entity_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldContextSnapshot {
    pub app_id: String,
    pub active_target_file: String,
    pub world_snapshot: WorldSnapshotSummary,
    pub resource_inventory: ResourceInventorySnapshot,
    pub runtime_summary: WorldRuntimeSummary,
    pub query_tools: Vec<ResourceQueryToolSpec>,
    /// 仅用于上层注入模型上下文；常规 JSON API/CLI 不依赖它。
    #[serde(skip)]
    pub prompt_catalog_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldAssetListItem {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldAssetListResponse {
    pub app_id: String,
    pub scene_id: String,
    pub query_kind: String,
    pub total: usize,
    pub items: Vec<WorldAssetListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldAssetGetResponse {
    pub app_id: String,
    pub scene_id: String,
    pub id: String,
    pub kind: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldRuntimePeekResponse {
    pub app_id: String,
    pub scene_id: String,
    pub phase: String,
    pub result: String,
    pub countdown: i64,
    pub available_actions: Vec<String>,
    pub recent_trace_messages: Vec<String>,
}
