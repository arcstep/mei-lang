use std::collections::BTreeMap;

use serde_json::Value;

use crate::compile::backing_refs_from_block_props;
use crate::compile::reachability_tree::{ReachabilityTreeNode, ReachabilityTreeRoot};
use crate::model::{
    BuildNodeId, BuildT2PageIndex, SceneContract, T2PageFileEntry, T2PageSlotEntry, WorkspaceNode,
};

pub struct BuildT2PageIndexResult {
    pub index: BuildT2PageIndex,
    pub tree_root: ReachabilityTreeRoot,
}

pub fn build_t2_page_index(
    file_tree: &[WorkspaceNode],
    scene_contracts_by_id: &BTreeMap<String, SceneContract>,
    scene_projection_assembly_by_id: &BTreeMap<String, Value>,
) -> BuildT2PageIndexResult {
    let mut pages = BTreeMap::new();
    let mut tree_children = Vec::new();
    collect_page_files(
        file_tree,
        &mut pages,
        &mut tree_children,
        scene_contracts_by_id,
        scene_projection_assembly_by_id,
    );
    let index = BuildT2PageIndex { pages };
    let tree_root = ReachabilityTreeRoot {
        group: "t2_pages".to_string(),
        label: "T2 Pages".to_string(),
        default_open: false,
        children: tree_children,
    };
    BuildT2PageIndexResult { index, tree_root }
}

fn collect_page_files(
    nodes: &[WorkspaceNode],
    pages: &mut BTreeMap<String, T2PageFileEntry>,
    tree_children: &mut Vec<ReachabilityTreeNode>,
    scene_contracts_by_id: &BTreeMap<String, SceneContract>,
    scene_projection_assembly_by_id: &BTreeMap<String, Value>,
) {
    for node in nodes {
        if node.kind == "dir" {
            collect_page_files(
                &node.children,
                pages,
                tree_children,
                scene_contracts_by_id,
                scene_projection_assembly_by_id,
            );
            continue;
        }
        if node.kind != "file" || !is_t2_page_capsule(node.path.as_str()) {
            continue;
        }
        let page_file = node.path.clone();
        for export in &node.children {
            if export.kind != "scene_export" {
                continue;
            }
            let Some(scene_id) = export
                .scene_export_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let label = export.name.clone();
            let contract = scene_contracts_by_id.get(scene_id);
            let assembly = scene_projection_assembly_by_id.get(scene_id);
            let layout_mode = layout_mode_from_sources(contract, assembly);
            let slots = slots_from_sources(contract, assembly);
            let params_summary = params_summary_from_contract(contract);
            let board_key = format!("{page_file}#{scene_id}");
            let entry = T2PageFileEntry {
                page_file: page_file.clone(),
                scene_id: scene_id.to_string(),
                label: label.clone(),
                layout_mode,
                slots: slots.clone(),
                popup_consumers: Vec::new(),
                params_summary,
            };
            pages.insert(board_key.clone(), entry.clone());

            push_page_file_tree_node(tree_children, board_key.as_str(), &entry);
        }
    }
}

fn push_page_file_tree_node(
    tree_children: &mut Vec<ReachabilityTreeNode>,
    board_key: &str,
    entry: &T2PageFileEntry,
) {
    let page_file = entry.page_file.as_str();
    let scene_id = entry.scene_id.as_str();
    let label = entry.label.clone();
    let slots = &entry.slots;
    let file_node_id = BuildNodeId::board_file(board_key).encode();
    let slot_nodes = slots
        .iter()
        .map(|slot| {
            let node = BuildNodeId::board_slot(board_key, slot.slot_id.as_str());
            ReachabilityTreeNode {
                id: format!("board-slot-{}-{}", page_file, slot.slot_id),
                node_id: node.encode(),
                kind: "board_slot".to_string(),
                label: slot.label.clone().unwrap_or_else(|| slot.slot_id.clone()),
                badges: slot.component.clone().into_iter().collect(),
                board_layout_zone: slot.layout_zone.clone().unwrap_or_default(),
                children: Vec::new(),
                ..Default::default()
            }
        })
        .collect();
    let file_name = page_file
        .rsplit('/')
        .next()
        .unwrap_or(page_file)
        .trim_end_matches(".board.mei")
        .trim_end_matches(".page.mei");
    let display_label = if label.trim().is_empty() {
        scene_id.to_string()
    } else {
        label
    };
    tree_children.push(ReachabilityTreeNode {
        id: format!("board-file-{page_file}-{scene_id}"),
        node_id: file_node_id,
        kind: "page_file".to_string(),
        label: display_label,
        badges: vec![file_name.to_string(), scene_id.to_string()],
        children: slot_nodes,
        ..Default::default()
    });
}

/// Rebuild Boards reachability group from compile-time index (e.g. stale snapshot fallback).
pub fn board_tree_root_from_index(index: &BuildT2PageIndex) -> ReachabilityTreeRoot {
    let mut tree_children = Vec::new();
    for (board_key, entry) in &index.pages {
        push_page_file_tree_node(&mut tree_children, board_key.as_str(), entry);
    }
    ReachabilityTreeRoot {
        group: "t2_pages".to_string(),
        label: "T2 Pages".to_string(),
        default_open: false,
        children: tree_children,
    }
}

fn is_t2_page_capsule(path: &str) -> bool {
    path.ends_with(".page.mei") || path.ends_with(".board.mei")
}

fn layout_mode_from_sources(
    contract: Option<&SceneContract>,
    assembly: Option<&Value>,
) -> Option<String> {
    if let Some(mode) = assembly
        .and_then(|value| value.get("shell_contract"))
        .and_then(Value::as_object)
        .and_then(|shell| shell.get("layout_mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(mode.to_string());
    }
    contract.and_then(|value| {
        value
            .scene
            .local_nav
            .get("kind")
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn params_summary_from_contract(contract: Option<&SceneContract>) -> Option<String> {
    let params = contract?.scene.params.as_object()?;
    if params.is_empty() {
        return None;
    }
    Some(params.keys().cloned().collect::<Vec<_>>().join(", "))
}

fn slots_from_sources(
    contract: Option<&SceneContract>,
    assembly: Option<&Value>,
) -> Vec<T2PageSlotEntry> {
    let mut slots = if let Some(slots) = assembly
        .and_then(|value| value.get("projection_slots"))
        .and_then(Value::as_array)
    {
        slots
            .iter()
            .filter_map(|slot| parse_projection_slot(slot))
            .collect()
    } else {
        contract
            .map(|value| zones_from_shell_contract(&value.scene.local_nav, &value.panels))
            .unwrap_or_default()
    };
    if let Some(assembly) = assembly {
        if assembly_has_filter_schema(assembly)
            && !slots.iter().any(|slot| slot.slot_id == "filter")
        {
            slots.insert(
                0,
                T2PageSlotEntry {
                    slot_id: "filter".to_string(),
                    component: Some("filter".to_string()),
                    label: Some("过滤面板".to_string()),
                    layout_zone: Some("filter".to_string()),
                    ..Default::default()
                },
            );
        }
    }
    slots
}

fn assembly_has_filter_schema(assembly: &Value) -> bool {
    assembly
        .get("filter_schema")
        .and_then(|value| value.get("fields"))
        .and_then(Value::as_array)
        .map(|fields| !fields.is_empty())
        .unwrap_or(false)
}

fn parse_projection_slot(value: &Value) -> Option<T2PageSlotEntry> {
    let object = value.as_object()?;
    let slot_id = object
        .get("id")
        .or_else(|| object.get("slot_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let component = object
        .get("component")
        .and_then(Value::as_str)
        .map(str::to_string);
    let label = object
        .get("title")
        .or_else(|| object.get("label"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let layout_zone = object
        .get("layout_zone")
        .or_else(|| object.get("layoutZone"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let mut backing_refs = Vec::new();
    collect_backing_refs(value, &mut backing_refs);
    Some(T2PageSlotEntry {
        slot_id,
        component,
        label,
        layout_zone,
        backing_refs,
    })
}

fn zones_from_shell_contract(
    _local_nav: &Value,
    _panels: &[crate::model::UiNodeDecl],
) -> Vec<T2PageSlotEntry> {
    Vec::new()
}

fn collect_backing_refs(value: &Value, out: &mut Vec<String>) {
    backing_refs_from_block_props(value)
        .into_iter()
        .for_each(|item| {
            if !out.contains(&item) {
                out.push(item);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_index_collects_board_mei_exports() {
        let tree = vec![WorkspaceNode {
            name: "05-监督预警.board.mei".to_string(),
            path: "scenes/05-监督预警.board.mei".to_string(),
            kind: "file".to_string(),
            mei_kind: Some("board".to_string()),
            scene_export_id: None,
            world_dataset_id: None,
            world_metric_id: None,
            explain_block_id: None,
            semantic_label: None,
            children: vec![WorkspaceNode {
                name: "监督事项分析".to_string(),
                path: "scenes/05-监督预警.board.mei".to_string(),
                kind: "scene_export".to_string(),
                mei_kind: Some("board".to_string()),
                scene_export_id: Some("supervision_items_analytics_board".to_string()),
                world_dataset_id: None,
                world_metric_id: None,
                explain_block_id: None,
                semantic_label: None,
                children: Vec::new(),
            }],
        }];
        let mut assembly = BTreeMap::new();
        assembly.insert(
            "supervision_items_analytics_board".to_string(),
            serde_json::json!({
                "shell_contract": { "layout_mode": "analytics" },
                "filter_schema": {
                    "fields": [{ "key": "dept", "label": "部门", "column": "部门" }]
                },
                "projection_slots": [
                    {
                        "id": "hero",
                        "component": "metric_card",
                        "layout_zone": "hero",
                        "metric": { "__ref": "metric", "id": "total" }
                    }
                ]
            }),
        );
        let result = build_t2_page_index(&tree, &BTreeMap::new(), &assembly);
        assert_eq!(result.index.pages.len(), 1);
        let entry = result
            .index
            .pages
            .get("scenes/05-监督预警.board.mei#supervision_items_analytics_board")
            .expect("board entry");
        assert_eq!(entry.scene_id, "supervision_items_analytics_board");
        assert_eq!(entry.layout_mode.as_deref(), Some("analytics"));
        assert_eq!(entry.slots.len(), 2);
        assert_eq!(entry.slots[0].slot_id, "filter");
        assert_eq!(entry.slots[1].slot_id, "hero");
        assert_eq!(entry.slots[1].layout_zone.as_deref(), Some("hero"));
        assert_eq!(result.tree_root.children.len(), 1);
        assert_eq!(result.tree_root.children[0].children.len(), 2);
        assert_eq!(
            result.tree_root.children[0].children[0].board_layout_zone,
            "filter"
        );
        assert_eq!(
            result.tree_root.children[0].children[1].board_layout_zone,
            "hero"
        );
    }
}
