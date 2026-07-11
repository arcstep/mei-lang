use super::{
    backing_refs_from_block_props, find_block_in_panel, find_panel_by_path, split_block_key,
    split_panel_key,
};

use std::collections::BTreeMap;

use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp, ExperienceNodeManifest, UiTreeNode};

pub fn build_overview_backing(compiled: &CompiledApp, node: &BuildNodeId) -> Vec<String> {
    if let Some(manifest) = ExperienceNodeManifest::lookup(compiled, node) {
        if !manifest.backing_refs.is_empty() {
            return manifest.backing_refs.clone();
        }
    }
    build_overview_backing_runtime(compiled, node)
}

pub fn experience_mount_chain(
    compiled: &CompiledApp,
    node: &BuildNodeId,
) -> Vec<crate::model::MountChainEntry> {
    ExperienceNodeManifest::lookup(compiled, node)
        .map(|manifest| manifest.mount_chain.clone())
        .unwrap_or_default()
}

pub fn experience_layout_hint(compiled: &CompiledApp, node: &BuildNodeId) -> Option<String> {
    ExperienceNodeManifest::lookup(compiled, node).and_then(|manifest| manifest.layout_hint.clone())
}

fn build_overview_backing_runtime(compiled: &CompiledApp, node: &BuildNodeId) -> Vec<String> {
    use BuildNodeKind::*;
    match node.kind {
        SceneBlock => {
            let (scene_id, panel_path, block_id) = split_block_key(&node.key);
            let Some(panel) = find_panel_by_path(compiled, &scene_id, panel_path.as_str()) else {
                return Vec::new();
            };
            let Some(block) = find_block_in_panel(&panel, block_id.as_str()) else {
                return Vec::new();
            };
            backing_refs_from_block_props(&block.props)
        }
        ScenePanel => {
            let (scene_id, panel_path) = split_panel_key(&node.key);
            let Some(panel) = find_panel_by_path(compiled, &scene_id, panel_path.as_str()) else {
                return Vec::new();
            };
            let mut refs = Vec::new();
            for ui_node in &panel.blocks {
                if let UiTreeNode::Block(block) = ui_node {
                    refs.extend(backing_refs_from_block_props(&block.props));
                }
            }
            dedupe_preserve_order(&mut refs);
            refs
        }
        WorldDataset | WorldMetric => {
            vec![format!("world: {}", node.key.replace('#', " › "))]
        }
        Dataset => vec![format!("resource: {}", node.key)],
        _ => Vec::new(),
    }
}

pub fn format_experience_path(path: &[String]) -> String {
    path.join(" › ")
}

pub(super) fn dedupe_preserve_order(items: &mut Vec<String>) {
    let mut seen = BTreeMap::<String, ()>::new();
    items.retain(|item| {
        if seen.contains_key(item) {
            false
        } else {
            seen.insert(item.clone(), ());
            true
        }
    });
}
