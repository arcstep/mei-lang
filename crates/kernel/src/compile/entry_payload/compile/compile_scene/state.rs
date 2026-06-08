use std::collections::{BTreeMap, BTreeSet};

use crate::mei_config::MeiConfig;
use crate::model::{
    ComponentAsset, Diagnostic, EntityDecl, FlowDecl, FrameDecl, LayoutDecl, PanelDecl,
    ResourceDecl, SceneDecl, ThemeDecl, WorldDecl, WorldGridDecl,
};

pub(super) struct CompileSceneCtx {
    pub config: MeiConfig,
    pub diagnostics: Vec<Diagnostic>,
    pub app_entry_main: String,
    pub scenes: BTreeMap<String, SceneDecl>,
    pub frames: BTreeMap<String, FrameDecl>,
    pub worlds: BTreeMap<String, WorldDecl>,
    pub flows: BTreeMap<String, FlowDecl>,
    pub scene_decl_count: usize,
    pub frame_decl_count: usize,
    pub world_decl_count: usize,
    pub world_topology_set_count: usize,
    pub frame_layout_set_count: usize,
    pub frame_default: Option<FrameDecl>,
    pub world_default: Option<WorldDecl>,
    pub flow_default: Option<FlowDecl>,
    pub pending_world_resources: Vec<ResourceDecl>,
    pub pending_world_entities: Vec<EntityDecl>,
    pub pending_world_metrics: Vec<serde_json::Value>,
    pub pending_world_topology: Option<WorldGridDecl>,
    pub pending_frame_layout: Option<LayoutDecl>,
    pub themes: Vec<ThemeDecl>,
    pub panels: Vec<PanelDecl>,
    pub top_level_legacy_dataset_count: usize,
    pub top_level_legacy_dataset_view_count: usize,
    pub top_level_legacy_metric_pack_count: usize,
    pub ref_scene_files: BTreeSet<String>,
    pub seen_world_decl: bool,
    pub first_scene_decl_index: Option<usize>,
    pub first_world_decl_index: Option<usize>,
    pub dataset_library_only: bool,
    pub component_assets: Vec<ComponentAsset>,
    pub selected_scene: Option<SceneDecl>,
    pub frame: Option<FrameDecl>,
    pub world: Option<WorldDecl>,
    pub flow: Option<FlowDecl>,
}

impl CompileSceneCtx {
    pub(super) fn new(app_root: &std::path::Path) -> Self {
        let config = crate::mei_config::load_mei_config_for_app(app_root, None);
        let app_entry_main = config.entry.main_rel();
        Self {
            config,
            diagnostics: Vec::new(),
            app_entry_main,
            scenes: BTreeMap::new(),
            frames: BTreeMap::new(),
            worlds: BTreeMap::new(),
            flows: BTreeMap::new(),
            scene_decl_count: 0,
            frame_decl_count: 0,
            world_decl_count: 0,
            world_topology_set_count: 0,
            frame_layout_set_count: 0,
            frame_default: None,
            world_default: None,
            flow_default: None,
            pending_world_resources: Vec::new(),
            pending_world_entities: Vec::new(),
            pending_world_metrics: Vec::new(),
            pending_world_topology: None,
            pending_frame_layout: None,
            themes: Vec::new(),
            panels: Vec::new(),
            top_level_legacy_dataset_count: 0,
            top_level_legacy_dataset_view_count: 0,
            top_level_legacy_metric_pack_count: 0,
            ref_scene_files: BTreeSet::new(),
            seen_world_decl: false,
            first_scene_decl_index: None,
            first_world_decl_index: None,
            dataset_library_only: false,
            component_assets: Vec::new(),
            selected_scene: None,
            frame: None,
            world: None,
            flow: None,
        }
    }
}
