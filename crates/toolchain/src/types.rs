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
pub struct WorldBusinessResourceSummary {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metric_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub related_to_target: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBusinessEntitySummary {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledRouteSummary {
    pub scene_id: String,
    pub target_file: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub is_default: bool,
    pub access_export: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedResourceSummary {
    pub id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub has_dataset: bool,
    pub has_document: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentAssetSummary {
    pub key: String,
    pub tag: String,
    pub script: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiagnosticCountSummary {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBusinessSummary {
    pub app_id: String,
    pub app_title: String,
    pub app_kind: String,
    pub scene_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_scene: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_scene_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_goal: Option<String>,
    pub active_target_file: String,
    pub business_focus: String,
    pub business_explanation: String,
    pub panel_count: usize,
    pub flow_interaction_count: usize,
    pub flow_subject_timer_count: usize,
    pub has_timer: bool,
    pub world_has_topology: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_layout_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub narrative: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_hints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resource_kind_counts: BTreeMap<String, usize>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub entity_kind_counts: BTreeMap<String, usize>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub loaded_resource_kind_counts: BTreeMap<String, usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_resources: Vec<WorldBusinessResourceSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_entities: Vec<WorldBusinessEntitySummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scene_routes: Vec<CompiledRouteSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loaded_resources: Vec<LoadedResourceSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub component_assets: Vec<ComponentAssetSummary>,
    pub diagnostics: DiagnosticCountSummary,
    pub runtime_summary: WorldRuntimeSummary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query_tools: Vec<ResourceQueryToolSpec>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceAppSummary {
    pub app_id: String,
    pub app_root: String,
    pub entry_main: String,
    pub layout_ok: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_scene: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_scene_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub business_explanation: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compile_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_resource_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_resource_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_asset_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compile_diagnostics: Option<DiagnosticCountSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub source_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_label: Option<String>,
    pub app_count: usize,
    pub healthy_app_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discover_skip_directories: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub app_aliases: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub menu_group_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub narrative: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<WorkspaceAppSummary>,
}
