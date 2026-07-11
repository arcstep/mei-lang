use std::collections::BTreeMap;
use std::path::Path;

use crate::model::{BlockDecl, BuildNodeId, BuildNodeKind, CompiledApp, UiNodeDecl, UiTreeNode};

pub fn preview_target_from_build_node(node: &BuildNodeId) -> Option<String> {
    crate::compile::build_experience::preview_target_from_build_node_with_app(node, None)
}

fn panel_path_for_use_key(
    panel: &UiNodeDecl,
    parent_path: Option<&str>,
    use_key: &str,
) -> Option<String> {
    let panel_path = match parent_path {
        Some(parent) => format!("{parent}/{}", panel.id),
        None => panel.id.clone(),
    };
    for node in &panel.blocks {
        match node {
            UiTreeNode::Block(BlockDecl { use_key: key, .. }) if key.as_str() == use_key => {
                return Some(panel_path);
            }
            UiTreeNode::Panel(nested) => {
                if let Some(found) =
                    panel_path_for_use_key(nested, Some(panel_path.as_str()), use_key)
                {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

/// When build preview compiles an authoring example, SSR may scope to the panel that
/// hosts the selected component so one tree node maps to one preview surface.
pub fn build_preview_panel_scope(compiled: &CompiledApp, node: &BuildNodeId) -> Option<String> {
    match node.kind {
        BuildNodeKind::Component => {
            let contract = compiled.scene_contract.as_ref()?;
            let scene_id = contract.scene.id.as_str();
            for panel in &contract.panels {
                if let Some(panel_path) = panel_path_for_use_key(panel, None, node.key.as_str()) {
                    return Some(format!("{scene_id}/{panel_path}"));
                }
            }
            None
        }
        BuildNodeKind::ScenePanel => {
            let key = node.key.trim();
            if key.is_empty() {
                None
            } else {
                Some(key.to_string())
            }
        }
        BuildNodeKind::SceneBlock => node
            .key
            .rsplit_once('/')
            .map(|(panel_path, _)| panel_path.to_string()),
        BuildNodeKind::UiScope => compiled
            .ui_layout_index
            .lookup(node)
            .map(|entry| entry.preview_scope.clone())
            .filter(|value| !value.is_empty()),
        _ => None,
    }
}

/// Preview scope path for UI structure nodes (region/section/micro/slot/content).
pub fn build_preview_ui_scope(compiled: &CompiledApp, node: &BuildNodeId) -> Option<String> {
    match node.kind {
        BuildNodeKind::UiScope => compiled
            .ui_layout_index
            .lookup(node)
            .map(|entry| entry.preview_scope.clone())
            .filter(|value| !value.is_empty()),
        _ => None,
    }
}

pub fn catalog_preview_target_for_build_node(
    app_root: &Path,
    node: &BuildNodeId,
) -> Option<String> {
    let scene_routes = crate::catalog_app::catalog_scene_routes_from_app_root(app_root);
    if scene_routes.is_empty() {
        return None;
    }
    let active_target_file = scene_routes
        .first()
        .map(|route| route.target_file.clone())
        .unwrap_or_default();
    let active_scene = scene_routes.first().map(|route| route.scene_id.clone());
    let stub = CompiledApp {
        app_id: String::new(),
        title: String::new(),
        app_root: app_root.display().to_string(),
        scene_routes,
        active_scene,
        active_target_file,
        file_tree: Vec::new(),
        scene_contract: None,
        scene_local_nav_by_target: BTreeMap::new(),
        scene_bindings_by_id: BTreeMap::new(),
        scene_examples_by_id: BTreeMap::new(),
        scene_projection_assembly_by_id: BTreeMap::new(),
        resources: Vec::new(),
        world_metrics: BTreeMap::new(),
        world_semantic_by_file: BTreeMap::new(),
        component_assets: Vec::new(),
        diagnostics: Vec::new(),
        build_experience_index: Default::default(),
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    crate::compile::build_experience::preview_target_from_build_node_with_app(node, Some(&stub))
}
