use std::collections::BTreeMap;

use serde_json::Value;

use crate::compile::backing_refs_from_block_props;
use crate::compile::reachability_tree::{ReachabilityTreeNode, ReachabilityTreeRoot};
use crate::model::{
    BoardFileEntry, BoardSlotEntry, BuildBoardIndex, BuildNodeId, SceneContract, WorkspaceNode,
};

pub struct BuildBoardIndexResult {
    pub index: BuildBoardIndex,
    pub tree_root: ReachabilityTreeRoot,
}

pub fn build_board_index(
    file_tree: &[WorkspaceNode],
    scene_contracts_by_id: &BTreeMap<String, SceneContract>,
    scene_projection_assembly_by_id: &BTreeMap<String, Value>,
) -> BuildBoardIndexResult {
    let mut boards = BTreeMap::new();
    let mut tree_children = Vec::new();
    collect_board_files(file_tree, &mut boards, &mut tree_children, scene_contracts_by_id, scene_projection_assembly_by_id);
    let index = BuildBoardIndex { boards };
    let tree_root = ReachabilityTreeRoot {
        group: "boards".to_string(),
        label: "Boards".to_string(),
        default_open: false,
        children: tree_children,
    };
    BuildBoardIndexResult { index, tree_root }
}

fn collect_board_files(
    nodes: &[WorkspaceNode],
    boards: &mut BTreeMap<String, BoardFileEntry>,
    tree_children: &mut Vec<ReachabilityTreeNode>,
    scene_contracts_by_id: &BTreeMap<String, SceneContract>,
    scene_projection_assembly_by_id: &BTreeMap<String, Value>,
) {
    for node in nodes {
        if node.kind == "dir" {
            collect_board_files(
                &node.children,
                boards,
                tree_children,
                scene_contracts_by_id,
                scene_projection_assembly_by_id,
            );
            continue;
        }
        if node.kind != "file" || !node.path.ends_with(".board.mei") {
            continue;
        }
        let board_file = node.path.clone();
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
            let board_key = format!("{board_file}#{scene_id}");
            let entry = BoardFileEntry {
                board_file: board_file.clone(),
                scene_id: scene_id.to_string(),
                label: label.clone(),
                layout_mode,
                slots: slots.clone(),
                popup_consumers: Vec::new(),
                params_summary,
            };
            boards.insert(board_key.clone(), entry);

            let file_node_id = BuildNodeId::board_file(board_key.as_str()).encode();
            let slot_nodes = slots
                .iter()
                .map(|slot| {
                    let node = BuildNodeId::board_slot(board_key.as_str(), slot.slot_id.as_str());
                    ReachabilityTreeNode {
                        id: format!("board-slot-{}-{}", board_file, slot.slot_id),
                        node_id: node.encode(),
                        kind: "board_slot".to_string(),
                        label: slot
                            .label
                            .clone()
                            .unwrap_or_else(|| slot.slot_id.clone()),
                        badges: slot
                            .component
                            .clone()
                            .into_iter()
                            .collect(),
                        children: Vec::new(),
                    }
                })
                .collect();
            tree_children.push(ReachabilityTreeNode {
                id: format!("board-file-{board_file}-{scene_id}"),
                node_id: file_node_id,
                kind: "board_file".to_string(),
                label,
                badges: vec![scene_id.to_string()],
                children: slot_nodes,
            });
        }
    }
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
    Some(
        params
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn slots_from_sources(
    contract: Option<&SceneContract>,
    assembly: Option<&Value>,
) -> Vec<BoardSlotEntry> {
    if let Some(slots) = assembly
        .and_then(|value| value.get("projection_slots"))
        .and_then(Value::as_array)
    {
        return slots
            .iter()
            .filter_map(|slot| parse_projection_slot(slot))
            .collect();
    }
    contract
        .map(|value| zones_from_shell_contract(&value.scene.local_nav, &value.panels))
        .unwrap_or_default()
}

fn parse_projection_slot(value: &Value) -> Option<BoardSlotEntry> {
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
    let mut backing_refs = Vec::new();
    collect_backing_refs(value, &mut backing_refs);
    Some(BoardSlotEntry {
        slot_id,
        component,
        label,
        backing_refs,
    })
}

fn zones_from_shell_contract(_local_nav: &Value, _panels: &[crate::model::PanelDecl]) -> Vec<BoardSlotEntry> {
    Vec::new()
}

fn collect_backing_refs(value: &Value, out: &mut Vec<String>) {
    backing_refs_from_block_props(value).into_iter().for_each(|item| {
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
                "projection_slots": [
                    { "id": "hero", "component": "metric_card", "metric": { "__ref": "metric", "id": "total" } }
                ]
            }),
        );
        let result = build_board_index(&tree, &BTreeMap::new(), &assembly);
        assert_eq!(result.index.boards.len(), 1);
        let entry = result
            .index
            .boards
            .get("scenes/05-监督预警.board.mei#supervision_items_analytics_board")
            .expect("board entry");
        assert_eq!(entry.scene_id, "supervision_items_analytics_board");
        assert_eq!(entry.layout_mode.as_deref(), Some("analytics"));
        assert_eq!(entry.slots.len(), 1);
        assert_eq!(result.tree_root.children.len(), 1);
        assert_eq!(result.tree_root.children[0].children.len(), 1);
    }
}
