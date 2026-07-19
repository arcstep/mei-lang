use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::content_capability_abi::ContentCapability;
use super::contract::SceneContract;
use super::dataset::WorldMetricLedgerEntry;
use super::diagnostic::Diagnostic;
use super::narration_abi::NarrationCatalog;
use super::resource::LoadedResource;
use super::scene_slot_abi::SceneSlotModule;
use super::stage_program::{StageProgramIndex, StageSlideInput};
use super::stage_registry::StageRegistry;
use super::workspace::{ComponentAsset, WorkspaceNode};
use super::world_semantic::WorldSemanticFileIndex;

fn default_access_export() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompiledSceneRoute {
    /// Product Stage id (wire: `stage_id`; Phase 9 still accepts legacy `scene_id` on read).
    #[serde(rename = "stage_id", alias = "scene_id")]
    pub scene_id: String,
    #[serde(default)]
    pub frame_id: Option<String>,
    pub target_file: String,
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_title: Option<String>,
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
    /// Active Access Stage id (wire: `active_stage`; still accepts `active_scene`).
    #[serde(
        default,
        rename = "active_stage",
        alias = "active_scene",
        skip_serializing_if = "Option::is_none"
    )]
    pub active_scene: Option<String>,
    /// Phase 1 additive Stage product registry (derived from routes; T2 excluded).
    #[serde(default)]
    pub stage_registry: StageRegistry,
    /// Phase 2 additive StageProgram index (adapted from Registry + deck/scene).
    #[serde(default)]
    pub stage_programs: StageProgramIndex,
    /// Phase 3 Scene Slot ABI modules (keyed by `scene:{stage_id}`).
    #[serde(default)]
    pub scene_slot_modules: BTreeMap<String, SceneSlotModule>,
    /// Phase 3 Content Capability ABI (keyed by capability id).
    #[serde(default)]
    pub content_capabilities: BTreeMap<String, ContentCapability>,
    /// Phase 3 Narration catalogs (keyed by `narration:{stage_id}`).
    #[serde(default)]
    pub narration_catalogs: BTreeMap<String, NarrationCatalog>,
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
    pub build_t2_page_index: super::build_view_index::BuildT2PageIndex,
    #[serde(default)]
    pub build_template_index: super::build_view_index::BuildTemplateIndex,
    /// UI structure tree for build-view (scene → plane → region → section → micro → slot → content).
    #[serde(default)]
    pub ui_layout_index: super::ui_layout_index::UiLayoutIndex,
}

impl CompiledApp {
    /// Ensure `stage_registry` matches current `scene_routes` (call after routes mutate).
    pub fn rebuild_stage_registry(&mut self) {
        self.stage_registry = StageRegistry::from_compiled_routes(&self.scene_routes);
    }

    /// Rebuild StagePrograms from Registry (optional per-stage slide lists for Slides).
    pub fn rebuild_stage_programs(
        &mut self,
        slides_by_stage: &std::collections::BTreeMap<String, Vec<StageSlideInput>>,
    ) {
        self.stage_programs =
            StageProgramIndex::from_registry(&self.stage_registry, slides_by_stage);
    }

    /// Rebuild Registry then Programs with an empty slide map (deck units filled later).
    pub fn rebuild_stage_identity(&mut self) {
        self.rebuild_stage_registry();
        self.rebuild_stage_programs(&std::collections::BTreeMap::new());
    }

    /// Phase 3: project Slot/Capability/Narration ABI and bind digests onto StagePrograms.
    pub fn rebuild_abi_projection(&mut self, presentation_map: Option<&Value>) {
        use super::abi_project::{
            bind_programs_to_abi, project_abi, validate_abi_against_programs, AbiProjectionInput,
        };
        use super::stage_registry::StageProfile;

        let stage_id = self.active_stage_id().map(str::to_string);
        let profile = stage_id.as_deref().and_then(|id| {
            self.stage_registry
                .get(id)
                .map(|d| d.profile)
                .or_else(|| self.stage_programs.get(id).map(|p| p.profile))
        });
        let source_anchor = stage_id.as_deref().and_then(|id| {
            self.stage_registry
                .get(id)
                .map(|d| d.source_anchor.as_str())
                .or_else(|| {
                    self.stage_programs
                        .get(id)
                        .map(|p| p.source_anchor.as_str())
                })
        });
        let input = AbiProjectionInput {
            stage_id: stage_id.as_deref(),
            stage_source_anchor: source_anchor,
            profile: profile.or(Some(StageProfile::Cockpit)),
            scene_contract: self.scene_contract.as_ref(),
            ui_layout_index: Some(&self.ui_layout_index),
            presentation_map,
        };
        let mut projection = project_abi(&input);
        let mut more = validate_abi_against_programs(
            &self.stage_programs,
            &projection.scene_slot_modules,
            &projection.content_capabilities,
            &projection.narration_catalogs,
        );
        projection.diagnostics.append(&mut more);
        self.scene_slot_modules = projection.scene_slot_modules.clone();
        self.content_capabilities = projection.content_capabilities.clone();
        self.narration_catalogs = projection.narration_catalogs.clone();
        bind_programs_to_abi(&mut self.stage_programs, &projection);
        self.diagnostics.extend(projection.diagnostics);
    }

    /// Active Stage id (legacy `active_scene` alias).
    pub fn active_stage_id(&self) -> Option<&str> {
        self.active_scene.as_deref().or_else(|| {
            self.stage_registry
                .default_stage_id
                .as_ref()
                .map(|id| id.as_str())
        })
    }
}
