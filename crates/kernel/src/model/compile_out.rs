use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::contract::SceneContract;
use super::dataset::WorldMetricLedgerEntry;
use super::diagnostic::Diagnostic;
use super::resource::LoadedResource;
use super::workspace::{ComponentAsset, WorkspaceNode};

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
    pub resources: Vec<LoadedResource>,
    #[serde(default)]
    pub world_metrics: BTreeMap<String, WorldMetricLedgerEntry>,
    #[serde(default)]
    pub component_assets: Vec<ComponentAsset>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}
