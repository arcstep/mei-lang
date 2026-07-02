use super::index::MAX_BLOCK_CHILDREN_IN_TREE;
use std::collections::BTreeMap;

use serde_json::Value;

use crate::compile::{
    aggregate_use_key_badges, backing_refs_from_block_props, block_instance_id,
    reachability_tree::ReachabilityTreeNode,
};
use crate::model::{
    BlockDecl, BuildNodeId, BuildNodeKind, CompiledSceneRoute, ExperienceNodeManifest, MountChainEntry, PanelDecl, SceneContract, UiNodeDecl,
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

pub(super) fn collect_panel_subtree(
    scene_id: &str,
    panel: &PanelDecl,
    panel_path: &str,
    manifest: &mut BTreeMap<String, ExperienceNodeManifest>,
    scene_label: &str,
) -> Vec<ReachabilityTreeNode> {
    let node = BuildNodeId::scene_panel(scene_id, panel_path);
    let node_id = node.encode();
    let label = panel_title(panel);
    let mut backing = Vec::new();
    for ui_node in &panel.blocks {
        if let UiNodeDecl::Block(block) = ui_node {
            backing.extend(backing_refs_from_block_props(&block.props));
        }
    }
    dedupe(&mut backing);

    let nested_panels = nested_panels_in(&panel.blocks);
    let blocks = blocks_in(&panel.blocks);
    let mut child_ids = Vec::new();
    let mut tree_children = Vec::new();

    for nested in &nested_panels {
        let nested_path = format!("{panel_path}/{}", nested.id);
        tree_children.extend(collect_panel_subtree(
            scene_id,
            nested,
            nested_path.as_str(),
            manifest,
            scene_label,
        ));
        let nested_node = BuildNodeId::scene_panel(scene_id, nested_path.as_str()).encode();
        child_ids.push(nested_node);
    }

    if blocks.len() <= MAX_BLOCK_CHILDREN_IN_TREE {
        for (ordinal, block) in blocks.iter().enumerate() {
            if let Some(block_node) = block_tree_node(
                scene_id,
                panel_path,
                block,
                ordinal,
                manifest,
                scene_label,
                panel,
            ) {
                child_ids.push(block_node.node_id.clone());
                tree_children.push(block_node);
            }
        }
    }

    let experience_path =
        build_panel_experience_path(scene_label, panel_path, panel, manifest, scene_id);
    manifest.insert(
        node_id.clone(),
        ExperienceNodeManifest {
            node_id: node_id.clone(),
            kind: BuildNodeKind::ScenePanel.slug().to_string(),
            label: label.clone(),
            experience_path,
            mount_chain: mount_chain_for_panel(panel),
            layout_hint: layout_hint_for_panel(panel),
            backing_refs: backing,
            tree_tier: if nested_panels.is_empty() {
                "coarse".to_string()
            } else {
                "section".to_string()
            },
            children: child_ids,
        },
    );

    vec![ReachabilityTreeNode {
        id: format!("scene-panel-{scene_id}-{panel_path}"),
        node_id,
        kind: "scene_panel".to_string(),
        label,
        badges: aggregate_use_key_badges(&panel.blocks),
        children: tree_children,
        ..Default::default()
    }]
}

fn block_tree_node(
    scene_id: &str,
    panel_path: &str,
    block: &BlockDecl,
    ordinal: usize,
    manifest: &mut BTreeMap<String, ExperienceNodeManifest>,
    scene_label: &str,
    parent_panel: &PanelDecl,
) -> Option<ReachabilityTreeNode> {
    let block_id = block_instance_id(block, ordinal);
    let node = BuildNodeId::scene_block(scene_id, panel_path, block_id.as_str());
    let node_id = node.encode();
    let label = block_title(block);
    let mut experience_path =
        build_panel_experience_path(scene_label, panel_path, parent_panel, manifest, scene_id);
    experience_path.push(label.clone());
    let backing = backing_refs_from_block_props(&block.props);
    manifest.insert(
        node_id.clone(),
        ExperienceNodeManifest {
            node_id: node_id.clone(),
            kind: BuildNodeKind::SceneBlock.slug().to_string(),
            label: label.clone(),
            experience_path,
            mount_chain: mount_chain_for_panel(parent_panel),
            layout_hint: None,
            backing_refs: backing.clone(),
            tree_tier: "fine".to_string(),
            children: Vec::new(),
        },
    );
    Some(ReachabilityTreeNode {
        id: format!("scene-block-{scene_id}-{panel_path}-{block_id}"),
        node_id,
        kind: "scene_block".to_string(),
        label,
        badges: {
            let mut badges = vec![block.use_key.clone()];
            badges.extend(backing);
            badges
        },
        children: Vec::new(),
        ..Default::default()
    })
}

fn build_panel_experience_path(
    scene_label: &str,
    panel_path: &str,
    panel: &PanelDecl,
    manifest: &BTreeMap<String, ExperienceNodeManifest>,
    scene_id: &str,
) -> Vec<String> {
    let mut path = vec![scene_label.to_string()];
    let segments: Vec<&str> = panel_path.split('/').collect();
    let mut cumulative = String::new();
    for (idx, segment) in segments.iter().enumerate() {
        cumulative = if idx == 0 {
            (*segment).to_string()
        } else {
            format!("{cumulative}/{segment}")
        };
        let lookup = BuildNodeId::scene_panel(scene_id, cumulative.as_str()).encode();
        if let Some(entry) = manifest.get(&lookup) {
            path.push(entry.label.clone());
        } else if idx == segments.len() - 1 {
            path.push(panel_title(panel));
        } else {
            path.push((*segment).to_string());
        }
    }
    path
}

fn mount_chain_for_panel(panel: &PanelDecl) -> Vec<MountChainEntry> {
    let mut chain = Vec::new();
    if let Some(scope) = panel
        .import_scope
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        chain.push(MountChainEntry {
            file: scope.to_string(),
            panel_id: panel.id.clone(),
            role: "panel_ref".to_string(),
        });
    }
    chain
}

fn layout_hint_for_panel(panel: &PanelDecl) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(area) = panel.area.as_deref().filter(|v| !v.trim().is_empty()) {
        parts.push(format!("area={area}"));
    }
    if let Some(object) = panel.props.as_object() {
        for key in [
            "position", "top", "right", "bottom", "left", "width", "height", "z_index",
        ] {
            if let Some(value) = object.get(key) {
                if let Some(text) = value.as_str() {
                    if !text.trim().is_empty() {
                        parts.push(format!("{key}={text}"));
                    }
                } else if !value.is_null() {
                    parts.push(format!("{key}={value}"));
                }
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

fn panel_title(panel: &PanelDecl) -> String {
    panel
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panel.id.clone())
}

fn block_title(block: &BlockDecl) -> String {
    block
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            block
                .id
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| block.use_key.clone())
        })
}

fn nested_panels_in(blocks: &[UiNodeDecl]) -> Vec<&PanelDecl> {
    blocks
        .iter()
        .filter_map(|node| match node {
            UiNodeDecl::Panel(panel) => Some(panel),
            _ => None,
        })
        .collect()
}

fn blocks_in(blocks: &[UiNodeDecl]) -> Vec<&BlockDecl> {
    blocks
        .iter()
        .filter_map(|node| match node {
            UiNodeDecl::Block(block) => Some(block),
            _ => None,
        })
        .collect()
}

fn dedupe(items: &mut Vec<String>) {
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

