use super::walker::{build_scene_ui_structure, UiStructureBuildResult};

use std::collections::{BTreeMap, HashSet};

use crate::compile::build_experience_index::panels_for_scene_from_maps;
use crate::compile::reachability_tree::{ReachabilityTreeNode, ReachabilityTreeRoot};
use crate::model::{
    BuildNodeId, CompiledApp, CompiledSceneRoute, ReachabilityTreeNodeSnapshot,
    ReachabilityTreeRootSnapshot, UiLayoutIndex, UiScopeNode, UiScopeRole, UiSourceAnchor,
};

pub struct UiLayoutIndexResult {
    pub index: UiLayoutIndex,
    pub tree_root: ReachabilityTreeRoot,
    pub duplicate_node_ids: Vec<String>,
}

pub fn build_ui_layout_index(compiled: &CompiledApp) -> UiLayoutIndexResult {
    let contracts = scene_contracts_from_compiled(compiled);
    let mut index = UiLayoutIndex::default();
    let mut tree_children = Vec::new();
    let mut duplicate_node_ids = Vec::new();
    let mut seen_scene_ids = HashSet::new();
    let active_scene = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    for route in scene_routes_for_ui_tree(&compiled.scene_routes, active_scene.as_deref()) {
        if !seen_scene_ids.insert(route.scene_id.clone()) {
            continue;
        }
        let Some(panels) = panels_for_scene_from_maps(
            route.scene_id.as_str(),
            &compiled.scene_projection_assembly_by_id,
            &contracts,
        ) else {
            continue;
        };
        if panels.is_empty() {
            continue;
        }
        let scene_label = scene_route_label(route);
        let result = build_scene_ui_structure(
            route.scene_id.as_str(),
            scene_label.as_str(),
            &panels,
            compiled.app_id.as_str(),
        );
        merge_build_result(&mut index, result, &mut duplicate_node_ids);
        append_t2_board_structure(&mut index, compiled, route.scene_id.as_str());
        if let Some(scene_node) = index.nodes.get(&scene_root_id(route.scene_id.as_str())) {
            if let Some(tree_node) = ui_scope_to_tree_node(scene_node, &index) {
                tree_children.push(tree_node);
            }
        }
    }

    let label_counts = count_content_labels(&index);
    for scene in &mut tree_children {
        relabel_tree_for_duplicate_labels(scene, &label_counts, None);
    }

    index.scene_roots = tree_children
        .iter()
        .filter_map(|node| BuildNodeId::parse(&node.node_id).map(|id| id.key.clone()))
        .collect();

    let tree_root = ReachabilityTreeRoot {
        group: "ui_structure".to_string(),
        label: "结构".to_string(),
        default_open: true,
        children: tree_children,
    };
    UiLayoutIndexResult {
        index,
        tree_root,
        duplicate_node_ids,
    }
}

fn scene_routes_for_ui_tree<'a>(
    routes: &'a [CompiledSceneRoute],
    active_scene: Option<&str>,
) -> Vec<&'a CompiledSceneRoute> {
    routes
        .iter()
        .filter(|route| {
            !route.target_file.ends_with(".board.mei") && !route.target_file.ends_with(".page.mei")
        })
        .filter(|route| active_scene.is_none_or(|scene| route.scene_id.as_str() == scene))
        .collect()
}

fn scene_route_label(route: &CompiledSceneRoute) -> String {
    route
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| route.scene_id.clone())
}

fn scene_root_id(scene_id: &str) -> String {
    BuildNodeId::ui_scope(scene_id, scene_id).encode()
}

fn append_t2_board_structure(index: &mut UiLayoutIndex, compiled: &CompiledApp, scene_id: &str) {
    let scene_node_id = scene_root_id(scene_id);
    let Some(_scene_node) = index.nodes.get(&scene_node_id) else {
        return;
    };
    let mut board_entries: Vec<_> = compiled
        .build_t2_page_index
        .pages
        .values()
        .filter(|entry| {
            entry
                .popup_consumers
                .iter()
                .any(|consumer| consumer == scene_id)
                || entry.scene_id == scene_id
        })
        .collect();
    if board_entries.is_empty() {
        return;
    }
    board_entries.sort_by(|left, right| left.label.cmp(&right.label));

    let plane_segments = vec![scene_id.to_string(), "T2".to_string()];
    let plane_node = UiScopeNode {
        node_id: BuildNodeId::ui_scope(scene_id, plane_segments.join("/")).encode(),
        role: UiScopeRole::Plane,
        label: "T2 · 二层".to_string(),
        scope_path: plane_segments.clone(),
        plane: Some("T2".to_string()),
        parent_id: Some(scene_node_id.clone()),
        children: Vec::new(),
        preview_scope: String::new(),
        budget: None,
        source_anchors: Vec::new(),
        content_kind: None,
        scene_id: Some(scene_id.to_string()),
    };
    let plane_id = plane_node.node_id.clone();
    index.nodes.insert(plane_id.clone(), plane_node);
    if let Some(scene_node) = index.nodes.get_mut(&scene_node_id) {
        if !scene_node.children.contains(&plane_id) {
            scene_node.children.push(plane_id.clone());
        }
    }

    for board in board_entries {
        let region_key = board.scene_id.clone();
        let mut region_segments = plane_segments.clone();
        region_segments.push(region_key.clone());
        let region_node = UiScopeNode {
            node_id: BuildNodeId::ui_scope(scene_id, region_segments.join("/")).encode(),
            role: UiScopeRole::Region,
            label: board.label.clone(),
            scope_path: region_segments.clone(),
            plane: Some("T2".to_string()),
            parent_id: Some(plane_id.clone()),
            children: Vec::new(),
            preview_scope: region_key.clone(),
            budget: None,
            source_anchors: vec![UiSourceAnchor {
                file: board.page_file.clone(),
                symbol_id: board.scene_id.clone(),
            }],
            content_kind: Some("page_instance".to_string()),
            scene_id: Some(scene_id.to_string()),
        };
        let region_id = region_node.node_id.clone();
        index.nodes.insert(region_id.clone(), region_node);
        if let Some(plane) = index.nodes.get_mut(&plane_id) {
            if !plane.children.contains(&region_id) {
                plane.children.push(region_id.clone());
            }
        }
        for slot in &board.slots {
            let mut section_segments = region_segments.clone();
            section_segments.push(slot.slot_id.clone());
            let section_node = UiScopeNode {
                node_id: BuildNodeId::ui_scope(scene_id, section_segments.join("/")).encode(),
                role: UiScopeRole::Section,
                label: slot.label.clone().unwrap_or_else(|| slot.slot_id.clone()),
                scope_path: section_segments.clone(),
                plane: Some("T2".to_string()),
                parent_id: Some(region_id.clone()),
                children: Vec::new(),
                preview_scope: format!(
                    "{}/{}",
                    region_key,
                    slot.layout_zone.as_deref().unwrap_or(slot.slot_id.as_str())
                ),
                budget: None,
                source_anchors: Vec::new(),
                content_kind: slot.component.clone(),
                scene_id: Some(scene_id.to_string()),
            };
            let section_id = section_node.node_id.clone();
            index.nodes.insert(section_id.clone(), section_node);
            if let Some(region) = index.nodes.get_mut(&region_id) {
                if !region.children.contains(&section_id) {
                    region.children.push(section_id);
                }
            }
        }
    }
}

fn count_content_labels(index: &UiLayoutIndex) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for node in index.nodes.values() {
        if node.role == UiScopeRole::Content {
            *counts.entry(node.label.clone()).or_default() += 1;
        }
    }
    counts
}

fn relabel_tree_for_duplicate_labels(
    node: &mut ReachabilityTreeNode,
    label_counts: &BTreeMap<String, usize>,
    section_label: Option<String>,
) {
    let current_section = if node.ui_role == "section" {
        Some(node.label.clone())
    } else {
        section_label
    };
    if node.ui_role == "content" {
        if label_counts.get(&node.label).copied().unwrap_or(0) > 1 {
            if let Some(section) = current_section.as_deref() {
                let short = section_short_label(section);
                if !node.label.starts_with(&short) {
                    node.label = format!("{short}·{}", node.label);
                }
            }
        }
    }
    for child in &mut node.children {
        relabel_tree_for_duplicate_labels(child, label_counts, current_section.clone());
    }
}

fn section_short_label(section: &str) -> String {
    section
        .split(['·', ' '])
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(section)
        .chars()
        .take(4)
        .collect()
}

fn merge_build_result(
    index: &mut UiLayoutIndex,
    result: UiStructureBuildResult,
    duplicate_node_ids: &mut Vec<String>,
) {
    for id in result.duplicate_node_ids {
        if !duplicate_node_ids.contains(&id) {
            duplicate_node_ids.push(id);
        }
    }
    for (id, node) in result.nodes {
        index.nodes.insert(id, node);
    }
}

fn ui_scope_to_tree_node(
    node: &UiScopeNode,
    index: &UiLayoutIndex,
) -> Option<ReachabilityTreeNode> {
    if matches!(node.role, UiScopeRole::Budget) {
        return None;
    }
    let parsed = BuildNodeId::parse(&node.node_id)?;
    let mut badges = vec![node.role.slug().to_string()];
    if let Some(plane) = node.plane.as_deref().filter(|v| !v.is_empty()) {
        badges.push(format!("plane:{plane}"));
    }
    if let Some(kind) = node.content_kind.as_deref().filter(|v| !v.is_empty()) {
        badges.push(kind.to_string());
    }
    let children = display_children_for_tree(node, index)
        .into_iter()
        .filter_map(|child| ui_scope_to_tree_node(child, index))
        .collect();
    let (source_file, source_symbol) = primary_source_anchor(node);
    Some(ReachabilityTreeNode {
        id: format!("ui-scope-{}", parsed.key.replace('/', "-")),
        node_id: node.node_id.clone(),
        kind: "ui_scope".to_string(),
        label: node.label.clone(),
        badges,
        ui_role: node.role.slug().to_string(),
        preview_scope: node.preview_scope.clone(),
        plane_tier: node.plane.clone().unwrap_or_default(),
        source_file,
        source_symbol,
        children,
        ..Default::default()
    })
}

fn primary_source_anchor(node: &UiScopeNode) -> (String, String) {
    node.source_anchors
        .first()
        .map(|anchor| (anchor.file.clone(), anchor.symbol_id.clone()))
        .unwrap_or_default()
}

fn display_children_for_tree<'a>(
    node: &UiScopeNode,
    index: &'a UiLayoutIndex,
) -> Vec<&'a UiScopeNode> {
    let raw = match node.role {
        UiScopeRole::Content if content_has_content_children(node, index) => node
            .children
            .iter()
            .filter_map(|child_id| index.nodes.get(child_id))
            .filter(|child| child.role == UiScopeRole::Content)
            .collect(),
        _ => node
            .children
            .iter()
            .filter_map(|child_id| index.nodes.get(child_id))
            .filter(|child| child.role != UiScopeRole::Budget)
            .collect(),
    };
    if matches!(node.role, UiScopeRole::Section | UiScopeRole::Slot) {
        flatten_compound_slot_wrappers(raw, index)
    } else {
        raw
    }
}

fn is_compound_slot_wrapper(node: &UiScopeNode, index: &UiLayoutIndex) -> bool {
    if node.role != UiScopeRole::Slot {
        return false;
    }
    let macro_hint = node
        .content_kind
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !macro_hint.contains("compound") && !macro_hint.contains("triptych") {
        return false;
    }
    let children = node
        .children
        .iter()
        .filter_map(|child_id| index.nodes.get(child_id))
        .filter(|child| child.role != UiScopeRole::Budget)
        .collect::<Vec<_>>();
    !children.is_empty() && children.iter().all(|child| child.role == UiScopeRole::Slot)
}

fn flatten_compound_slot_wrappers<'a>(
    children: Vec<&'a UiScopeNode>,
    index: &'a UiLayoutIndex,
) -> Vec<&'a UiScopeNode> {
    let mut out = Vec::new();
    for child in children {
        if is_compound_slot_wrapper(child, index) {
            for grandchild_id in &child.children {
                if let Some(grandchild) = index.nodes.get(grandchild_id) {
                    if grandchild.role != UiScopeRole::Budget {
                        out.push(grandchild);
                    }
                }
            }
        } else {
            out.push(child);
        }
    }
    out
}

fn content_has_content_children(node: &UiScopeNode, index: &UiLayoutIndex) -> bool {
    node.children.iter().any(|child_id| {
        index
            .nodes
            .get(child_id)
            .is_some_and(|child| child.role == UiScopeRole::Content)
    })
}

fn scene_contracts_from_compiled(
    compiled: &CompiledApp,
) -> BTreeMap<String, crate::model::SceneContract> {
    use crate::model::{SceneContract, SceneDecl, UiNodeDecl};
    use serde_json::Value;

    let mut map = BTreeMap::new();
    for (scene_id, assembly) in &compiled.scene_projection_assembly_by_id {
        let panels = assembly
            .get("panels")
            .and_then(|value| serde_json::from_value::<Vec<UiNodeDecl>>(value.clone()).ok())
            .unwrap_or_default();
        let local_nav = assembly
            .get("shell_contract")
            .or_else(|| assembly.get("local_nav"))
            .cloned()
            .unwrap_or(Value::Null);
        map.insert(
            scene_id.clone(),
            SceneContract {
                scene: SceneDecl {
                    kind: "scene".to_string(),
                    id: scene_id.clone(),
                    world: None,
                    flow: None,
                    frame: None,
                    profile: None,
                    theme: None,
                    summary: None,
                    goal: None,
                    state: Value::Null,
                    shared: Value::Null,
                    local_nav,
                    params: Value::Null,
                    capabilities: Value::Null,
                    bindings: Value::Null,
                    examples: Value::Null,
                    access_export: true,
                },
                themes: Vec::new(),
                shared: Value::Null,
                world: None,
                flow: None,
                frame: None,
                panels,
            },
        );
    }
    if let Some(contract) = &compiled.scene_contract {
        let scene_id = compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| contract.scene.id.clone());
        if !scene_id.trim().is_empty() {
            map.insert(scene_id, contract.clone());
        }
    }
    map
}

pub fn ui_structure_root_snapshot(root: ReachabilityTreeRoot) -> ReachabilityTreeRootSnapshot {
    ReachabilityTreeRootSnapshot {
        group: root.group,
        label: root.label,
        default_open: root.default_open,
        children: root
            .children
            .into_iter()
            .map(node_to_snapshot_local)
            .collect(),
    }
}

fn node_to_snapshot_local(node: ReachabilityTreeNode) -> ReachabilityTreeNodeSnapshot {
    ReachabilityTreeNodeSnapshot {
        id: node.id,
        node_id: node.node_id,
        kind: node.kind,
        label: node.label,
        badges: node.badges,
        compile_scene: node.compile_scene,
        compile_target: node.compile_target,
        board_layout_zone: node.board_layout_zone,
        ui_role: node.ui_role,
        preview_scope: node.preview_scope,
        plane_tier: node.plane_tier,
        source_file: node.source_file,
        source_symbol: node.source_symbol,
        children: node
            .children
            .into_iter()
            .map(node_to_snapshot_local)
            .collect(),
    }
}

pub fn merge_ui_structure_root(
    snapshot: &mut Vec<ReachabilityTreeRootSnapshot>,
    ui_root: ReachabilityTreeRoot,
) {
    if ui_root.children.is_empty() {
        return;
    }
    let ui_snapshot = ui_structure_root_snapshot(ui_root);
    if let Some(existing) = snapshot
        .iter()
        .position(|root| root.group == "ui_structure")
    {
        snapshot[existing] = ui_snapshot;
    } else {
        snapshot.insert(0, ui_snapshot);
    }
}

pub fn filter_roots_for_tree_mode(
    roots: &[ReachabilityTreeRoot],
    mode: &str,
) -> Vec<ReachabilityTreeRoot> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "structure" | "ui_structure" | "ui-structure" => roots
            .iter()
            .filter(|root| root.group == "ui_structure")
            .cloned()
            .collect(),
        "compile" => roots
            .iter()
            .filter(|root| {
                matches!(
                    root.group.as_str(),
                    "mcg" | "scenes" | "routes" | "world" | "datasets" | "artifacts"
                )
            })
            .cloned()
            .collect(),
        _ => roots.to_vec(),
    }
}
