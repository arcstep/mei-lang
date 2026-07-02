use super::rebuild::root_to_snapshot;
use super::{collect_panel_subtree, disambiguate_tree_node_labels, panels_for_scene_from_maps, projection_children, scene_route_label};

use std::collections::BTreeMap;

use serde_json::Value;

use crate::compile::reachability_tree::{
        artifacts_root, datasets_root, routes_root, world_root, ReachabilityTreeNode,
        ReachabilityTreeRoot,
    };
use crate::model::{
    BuildExperienceIndex, BuildNodeId, CompiledApp, CompiledSceneRoute, SceneContract,
};

pub(super) const MAX_BLOCK_CHILDREN_IN_TREE: usize = 8;

fn scene_routes_for_build_tree<'a>(
    routes: &'a [CompiledSceneRoute],
) -> Vec<&'a CompiledSceneRoute> {
    routes
        .iter()
        .filter(|route| {
            !route.target_file.ends_with(".board.mei")
                && !route.target_file.ends_with(".page.mei")
        })
        .collect()
}

pub fn build_experience_index(
    scene_routes: &[CompiledSceneRoute],
    scene_projection_assembly_by_id: &BTreeMap<String, Value>,
    scene_contracts_by_id: &BTreeMap<String, SceneContract>,
    compiled_for_roots: &CompiledApp,
) -> BuildExperienceIndex {
    let mut index = BuildExperienceIndex::default();
    let mut scene_children = Vec::new();

    for route in scene_routes_for_build_tree(scene_routes) {
        let scene_node = BuildNodeId::scene(route.scene_id.clone());
        let mut children = Vec::new();

        let panels = panels_for_scene_from_maps(
            route.scene_id.as_str(),
            scene_projection_assembly_by_id,
            scene_contracts_by_id,
        );

        if let Some(panels) = panels {
            if !panels.is_empty() {
                let panel_nodes = panels
                    .iter()
                    .flat_map(|panel| {
                        collect_panel_subtree(
                            route.scene_id.as_str(),
                            panel,
                            panel.id.as_str(),
                            &mut index.node_manifest,
                            &scene_route_label(route),
                        )
                    })
                    .collect();
                children.push(ReachabilityTreeNode {
                    id: format!("scene-panels-{}", route.scene_id),
                    node_id: String::new(),
                    kind: "scene_group".to_string(),
                    label: "Panels".to_string(),
                    badges: Vec::new(),
                    children: panel_nodes,
                    ..Default::default()
                });
            }
        } else if !scene_projection_assembly_by_id.contains_key(&route.scene_id) {
            let target = route.target_file.replace('\\', "/");
            let is_stock_pack_preview = target.contains("/stock/components/")
                || target.contains("/stock/templates/")
                || target.starts_with("../../stock/");
            if !is_stock_pack_preview {
                children.push(ReachabilityTreeNode {
                    id: format!("scene-gate-{}", route.scene_id),
                    node_id: String::new(),
                    kind: "scene_group".to_string(),
                    label: "Panels".to_string(),
                    badges: vec!["gate:missing".to_string()],
                    children: Vec::new(),
                    ..Default::default()
                });
            }
        }

        if let Some(assembly) = scene_projection_assembly_by_id.get(&route.scene_id) {
            children.extend(projection_children(
                route.scene_id.as_str(),
                assembly,
                "board",
            ));
            children.extend(projection_children(
                route.scene_id.as_str(),
                assembly,
                "overlay",
            ));
        }

        scene_children.push(ReachabilityTreeNode {
            id: format!("scene-{}", route.scene_id),
            node_id: scene_node.encode(),
            kind: "scene".to_string(),
            label: scene_route_label(route),
            badges: vec![route.target_file.clone()],
            children,
            ..Default::default()
        });
    }

    disambiguate_tree_node_labels(&mut scene_children);

    let runtime_roots = vec![
        ReachabilityTreeRoot {
            group: "scenes".to_string(),
            label: "Scenes".to_string(),
            default_open: false,
            children: scene_children,
        },
        routes_root(compiled_for_roots),
        world_root(compiled_for_roots),
        datasets_root(compiled_for_roots),
        artifacts_root(compiled_for_roots),
    ];
    index.reachability_snapshot = runtime_roots.into_iter().map(root_to_snapshot).collect();
    index
}

