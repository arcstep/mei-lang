use super::rebuild::root_to_snapshot;
use super::{disambiguate_tree_node_labels, projection_children, scene_route_label};

use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use crate::compile::reachability_tree::{
    artifacts_root, datasets_root, routes_root, world_root, ReachabilityTreeNode,
    ReachabilityTreeRoot,
};
use crate::model::{
    BuildExperienceIndex, BuildNodeId, CompiledApp, CompiledSceneRoute, SceneContract,
};

fn scene_routes_for_build_tree<'a>(
    routes: &'a [CompiledSceneRoute],
) -> Vec<&'a CompiledSceneRoute> {
    routes
        .iter()
        .filter(|route| {
            !route.target_file.ends_with(".board.mei") && !route.target_file.ends_with(".page.mei")
        })
        .collect()
}

pub fn build_experience_index(
    scene_routes: &[CompiledSceneRoute],
    scene_projection_assembly_by_id: &BTreeMap<String, Value>,
    _scene_contracts_by_id: &BTreeMap<String, SceneContract>,
    compiled_for_roots: &CompiledApp,
) -> BuildExperienceIndex {
    let mut index = BuildExperienceIndex::default();
    let mut scene_children = Vec::new();
    let mut seen_scene_ids = HashSet::new();

    for route in scene_routes_for_build_tree(scene_routes) {
        if !seen_scene_ids.insert(route.scene_id.clone()) {
            continue;
        }
        let scene_node = BuildNodeId::scene(route.scene_id.clone());
        let mut children = Vec::new();

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
