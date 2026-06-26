use super::{build_experience_index, merge_build_view_tree_roots};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::compile::reachability_tree::{
        ReachabilityTreeNode,
        ReachabilityTreeRoot,
    };
use crate::model::{
    CompiledApp,
    ComponentAsset, PanelDecl,
    ReachabilityTreeNodeSnapshot, ReachabilityTreeRootSnapshot, SceneContract, SceneDecl,
};

pub(super) fn build_view_reachability_stale(compiled: &CompiledApp) -> bool {
    let snapshot = &compiled.build_experience_index.reachability_snapshot;
    if snapshot.is_empty() {
        return true;
    }
    let has_boards = snapshot.iter().any(|root| root.group == "boards");
    let expects_boards = file_tree_has_board_capsules(&compiled.file_tree)
        || !compiled.build_board_index.boards.is_empty();
    if expects_boards && !has_boards {
        return true;
    }
    false
}

fn file_tree_has_board_capsules(nodes: &[crate::model::WorkspaceNode]) -> bool {
    nodes.iter().any(|node| {
        if node.kind == "file" && node.path.ends_with(".board.mei") {
            return true;
        }
        node.kind == "dir" && file_tree_has_board_capsules(&node.children)
    })
}

pub(super) fn rebuild_reachability_tree_from_compiled(compiled: &CompiledApp) -> Vec<ReachabilityTreeRoot> {
    let contracts = scene_contracts_from_compiled(compiled);
    let mut file_tree = compiled.file_tree.clone();
    let app_root = Path::new(compiled.app_root.as_str());
    if app_root.is_dir() {
        let _ = crate::compile::source_tree_enrich::enrich_source_tree_with_scene_exports(
            app_root,
            &mut file_tree,
        );
    }
    let experience = build_experience_index(
        &compiled.scene_routes,
        &compiled.scene_projection_assembly_by_id,
        &contracts,
        compiled,
    );
    let board = crate::compile::build_board_index(
        &file_tree,
        &contracts,
        &compiled.scene_projection_assembly_by_id,
    );
    let (template_root, template_files_root) =
        if crate::mei_config::is_stock_catalog_app(compiled.app_id.as_str()) {
            let template = crate::compile::build_template_index(
                &template_catalog_for_tree(compiled, source_root_from_app(compiled).as_path()),
                &contracts,
                &experience.node_manifest,
            );
            let template_files = crate::compile::build_template_index::build_stock_template_files_root(
                &source_root_from_app(compiled),
            );
            (template.tree_root, template_files)
        } else {
            let empty = |group: &str, label: &str| ReachabilityTreeRoot {
                group: group.to_string(),
                label: label.to_string(),
                default_open: false,
                children: Vec::new(),
            };
            (empty("templates", "Components"), empty("template_files", "Templates"))
        };
    merge_build_view_tree_roots(
        experience.reachability_snapshot,
        board.tree_root,
        template_root,
        template_files_root,
    )
    .into_iter()
    .map(|snapshot| snapshot_to_root(&snapshot))
    .collect()
}

fn scene_contracts_from_compiled(compiled: &CompiledApp) -> BTreeMap<String, SceneContract> {
    let mut map = BTreeMap::new();
    for (scene_id, assembly) in &compiled.scene_projection_assembly_by_id {
        let panels = assembly
            .get("panels")
            .and_then(|value| serde_json::from_value::<Vec<PanelDecl>>(value.clone()).ok())
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
        if let Some(scene_id) = compiled
            .active_scene
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            map.insert(scene_id.to_string(), contract.clone());
        }
    }
    map
}

pub(super) fn ensure_board_and_template_roots(roots: &mut Vec<ReachabilityTreeRoot>, compiled: &CompiledApp) {
    if !roots.iter().any(|root| root.group == "boards") {
        let board_root = if !compiled.build_board_index.boards.is_empty() {
            Some(
                crate::compile::build_board_index::board_tree_root_from_index(
                    &compiled.build_board_index,
                ),
            )
        } else if file_tree_has_board_capsules(&compiled.file_tree) {
            let contracts = scene_contracts_from_compiled(compiled);
            Some(
                crate::compile::build_board_index::build_board_index(
                    &compiled.file_tree,
                    &contracts,
                    &compiled.scene_projection_assembly_by_id,
                )
                .tree_root,
            )
        } else {
            None
        };
        if let Some(board_root) = board_root {
            if !board_root.children.is_empty() {
                let insert_at = roots
                    .iter()
                    .position(|root| root.group == "scenes")
                    .map(|idx| idx + 1)
                    .unwrap_or(roots.len());
                roots.insert(insert_at, board_root);
            }
        }
    }
    if !crate::mei_config::is_stock_catalog_app(compiled.app_id.as_str()) {
        return;
    }
    let template_root = {
        let contracts = scene_contracts_from_compiled(compiled);
        let catalog =
            template_catalog_for_tree(compiled, source_root_from_app(compiled).as_path());
        if catalog.is_empty() {
            None
        } else {
            Some(
                crate::compile::build_template_index::build_template_index(
                    &catalog,
                    &contracts,
                    &compiled.build_experience_index.node_manifest,
                )
                .tree_root,
            )
        }
    };
    if let Some(template_root) = template_root {
        if !template_root.children.is_empty() {
            if let Some(existing) = roots.iter_mut().find(|root| root.group == "templates") {
                *existing = template_root;
            } else {
                let insert_at = roots
                    .iter()
                    .position(|root| root.group == "boards")
                    .map(|idx| idx + 1)
                    .unwrap_or(
                        roots
                            .iter()
                            .position(|root| root.group == "scenes")
                            .map(|idx| idx + 1)
                            .unwrap_or(roots.len()),
                    );
                roots.insert(insert_at, template_root);
            }
        }
    }
    if !roots.iter().any(|root| root.group == "template_files") {
        let template_files = crate::compile::build_template_index::build_stock_template_files_root(
            source_root_from_app(compiled).as_path(),
        );
        if !template_files.children.is_empty() {
            let insert_at = roots
                .iter()
                .position(|root| root.group == "templates")
                .map(|idx| idx + 1)
                .unwrap_or(
                    roots
                        .iter()
                        .position(|root| root.group == "boards")
                        .map(|idx| idx + 1)
                        .unwrap_or(
                            roots
                                .iter()
                                .position(|root| root.group == "scenes")
                                .map(|idx| idx + 1)
                                .unwrap_or(roots.len()),
                        ),
                );
            roots.insert(insert_at, template_files);
        }
    } else if let Some(existing) = roots.iter_mut().find(|root| root.group == "template_files") {
        if existing.children.is_empty() {
            let template_files =
                crate::compile::build_template_index::build_stock_template_files_root(
                    source_root_from_app(compiled).as_path(),
                );
            if !template_files.children.is_empty() {
                *existing = template_files;
            }
        }
    }
}

pub(super) fn source_root_from_app(compiled: &CompiledApp) -> PathBuf {
    crate::mei_config::resolve_workspace_source_root_from_app_root(Path::new(
        compiled.app_root.as_str(),
    ))
}

fn template_catalog_for_tree(compiled: &CompiledApp, source_root: &Path) -> Vec<ComponentAsset> {
    crate::compile::build_template_index::merged_component_catalog(source_root, compiled)
}

pub(super) fn root_to_snapshot(root: ReachabilityTreeRoot) -> ReachabilityTreeRootSnapshot {
    ReachabilityTreeRootSnapshot {
        group: root.group,
        label: root.label,
        default_open: root.default_open,
        children: root.children.into_iter().map(node_to_snapshot).collect(),
    }
}

pub(super) fn snapshot_to_root(snapshot: &ReachabilityTreeRootSnapshot) -> ReachabilityTreeRoot {
    ReachabilityTreeRoot {
        group: snapshot.group.clone(),
        label: snapshot.label.clone(),
        default_open: snapshot.default_open,
        children: snapshot
            .children
            .iter()
            .map(node_snapshot_to_runtime)
            .collect(),
    }
}

fn node_to_snapshot(node: ReachabilityTreeNode) -> ReachabilityTreeNodeSnapshot {
    ReachabilityTreeNodeSnapshot {
        id: node.id,
        node_id: node.node_id,
        kind: node.kind,
        label: node.label,
        badges: node.badges,
        compile_scene: node.compile_scene,
        compile_target: node.compile_target,
        board_layout_zone: node.board_layout_zone,
        children: node.children.into_iter().map(node_to_snapshot).collect(),
    }
}

fn node_snapshot_to_runtime(node: &ReachabilityTreeNodeSnapshot) -> ReachabilityTreeNode {
    ReachabilityTreeNode {
        id: node.id.clone(),
        node_id: node.node_id.clone(),
        kind: node.kind.clone(),
        label: node.label.clone(),
        badges: node.badges.clone(),
        compile_scene: node.compile_scene.clone(),
        compile_target: node.compile_target.clone(),
        board_layout_zone: node.board_layout_zone.clone(),
        children: node.children.iter().map(node_snapshot_to_runtime).collect(),
    }
}

