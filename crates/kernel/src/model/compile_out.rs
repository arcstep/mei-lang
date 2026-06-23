use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::contract::SceneContract;
use super::dataset::WorldMetricLedgerEntry;
use super::diagnostic::Diagnostic;
use super::resource::LoadedResource;
use super::workspace::{ComponentAsset, WorkspaceNode};
use super::world_semantic::WorldSemanticFileIndex;

fn default_access_export() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompiledSceneRoute {
    pub scene_id: String,
    #[serde(default)]
    pub frame_id: Option<String>,
    pub target_file: String,
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_access_export")]
    pub access_export: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledApp {
    pub app_id: String,
    pub title: String,
    pub app_root: String,
    #[serde(default)]
    pub scene_routes: Vec<CompiledSceneRoute>,
    #[serde(default)]
    pub active_scene: Option<String>,
    pub active_target_file: String,
    pub file_tree: Vec<WorkspaceNode>,
    #[serde(default)]
    pub scene_contract: Option<SceneContract>,
    #[serde(default)]
    pub scene_local_nav_by_target: BTreeMap<String, Value>,
    #[serde(default)]
    pub scene_bindings_by_id: BTreeMap<String, Value>,
    #[serde(default)]
    pub scene_examples_by_id: BTreeMap<String, Value>,
    #[serde(default)]
    pub scene_projection_assembly_by_id: BTreeMap<String, Value>,
    #[serde(default)]
    pub resources: Vec<LoadedResource>,
    #[serde(default)]
    pub world_metrics: BTreeMap<String, WorldMetricLedgerEntry>,
    #[serde(default)]
    pub world_semantic_by_file: BTreeMap<String, WorldSemanticFileIndex>,
    #[serde(default)]
    pub component_assets: Vec<ComponentAsset>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    /// Build-view experience tree + node manifests (compile finish).
    #[serde(default)]
    pub build_experience_index: super::build_view_index::BuildExperienceIndex,
    #[serde(default)]
    pub build_board_index: super::build_view_index::BuildBoardIndex,
    #[serde(default)]
    pub build_template_index: super::build_view_index::BuildTemplateIndex,
}
