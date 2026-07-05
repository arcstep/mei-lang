use std::collections::BTreeMap;

use serde_json::Value;

use crate::compile::reachability_tree::ReachabilityTreeNode;
use crate::model::{
    BuildNodeId, BuildNodeKind, CompiledSceneRoute, PanelDecl, SceneContract,
};

pub fn panels_for_scene_from_maps(
    scene_id: &str,
    assembly_by_id: &BTreeMap<String, Value>,
    contracts_by_id: &BTreeMap<String, SceneContract>,
) -> Option<Vec<PanelDecl>> {
    if let Some(contract) = contracts_by_id.get(scene_id) {
        if !contract.panels.is_empty() {
            return Some(contract.panels.clone());
        }
    }
    assembly_by_id
        .get(scene_id)
        .and_then(|assembly| assembly.get("panels"))
        .and_then(|value| serde_json::from_value::<Vec<PanelDecl>>(value.clone()).ok())
}

pub(super) fn scene_route_label(route: &CompiledSceneRoute) -> String {
    route
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| route.scene_id.clone())
}

pub(super) fn disambiguate_tree_node_labels(nodes: &mut [ReachabilityTreeNode]) {
    let mut counts = BTreeMap::<String, usize>::new();
    for node in nodes.iter() {
        if !node.node_id.trim().is_empty() {
            *counts.entry(node.label.clone()).or_default() += 1;
        }
    }
    for node in nodes.iter_mut() {
        if node.node_id.trim().is_empty() {
            disambiguate_tree_node_labels(&mut node.children);
            continue;
        }
        if counts.get(&node.label).copied().unwrap_or(0) > 1 {
            if let Some(hint) = tree_label_hint(node) {
                node.label = format!("{} · {}", node.label, hint);
            }
        }
        disambiguate_tree_node_labels(&mut node.children);
    }
}

fn tree_label_hint(node: &ReachabilityTreeNode) -> Option<String> {
    let parsed = BuildNodeId::parse(&node.node_id)?;
    match parsed.kind {
        BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock => {
            let segments: Vec<&str> = parsed.key.split('/').filter(|s| !s.is_empty()).collect();
            if segments.len() >= 2 {
                Some(format!(
                    "{}/{}",
                    segments[segments.len() - 2],
                    segments[segments.len() - 1]
                ))
            } else {
                segments.last().map(|s| (*s).to_string())
            }
        }
        BuildNodeKind::BoardFile => parsed
            .key
            .split('#')
            .next()
            .and_then(|path| path.rsplit('/').next())
            .map(str::to_string),
        BuildNodeKind::BoardSlot => parsed
            .key
            .rsplit_once('/')
            .map(|(_, slot)| slot.to_string()),
        _ => Some(parsed.key.clone()),
    }
}

pub(super) fn projection_children(scene_id: &str, assembly: &Value, kind: &str) -> Vec<ReachabilityTreeNode> {
    let key = if kind == "board" {
        "boards"
    } else {
        "overlays"
    };
    let badge = if kind == "board" {
        "board".to_string()
    } else {
        "link-only".to_string()
    };
    let Some(object) = assembly.get(key).and_then(Value::as_object) else {
        return Vec::new();
    };
    if object.is_empty() {
        return Vec::new();
    }
    let label = if kind == "board" {
        "Boards".to_string()
    } else {
        "Overlays".to_string()
    };
    let nodes = object
        .keys()
        .map(|projection_id| {
            let node = BuildNodeId::projection(scene_id, projection_id);
            ReachabilityTreeNode {
                id: format!("{kind}-{scene_id}-{projection_id}"),
                node_id: node.encode(),
                kind: "projection".to_string(),
                label: projection_id.clone(),
                badges: vec![badge.clone()],
                children: Vec::new(),
                ..Default::default()
            }
        })
        .collect();
    vec![ReachabilityTreeNode {
        id: format!("scene-{kind}s-{scene_id}"),
        node_id: String::new(),
        kind: "scene_group".to_string(),
        label,
        badges: Vec::new(),
        children: nodes,
        ..Default::default()
    }]
}
