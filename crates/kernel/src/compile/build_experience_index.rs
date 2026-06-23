use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::compile::{
    aggregate_use_key_badges, backing_refs_from_block_props, block_instance_id,
    reachability_tree::{
        artifacts_root, datasets_root, routes_root, world_root, ReachabilityTreeNode,
        ReachabilityTreeRoot,
    },
};
use crate::model::{
    BlockDecl, BuildExperienceIndex, BuildNodeId, BuildNodeKind, CompiledApp, CompiledSceneRoute,
    ComponentAsset, ExperienceNodeManifest, MountChainEntry, PanelDecl,
    ReachabilityTreeNodeSnapshot, ReachabilityTreeRootSnapshot, SceneContract, SceneDecl,
    UiNodeDecl,
};

const MAX_BLOCK_CHILDREN_IN_TREE: usize = 8;

fn scene_routes_for_build_tree<'a>(
    routes: &'a [CompiledSceneRoute],
) -> Vec<&'a CompiledSceneRoute> {
    routes
        .iter()
        .filter(|route| !route.target_file.ends_with(".board.mei"))
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

pub fn merge_build_view_tree_roots(
    experience_snapshot: Vec<ReachabilityTreeRootSnapshot>,
    board_root: ReachabilityTreeRoot,
    template_root: ReachabilityTreeRoot,
    template_files_root: ReachabilityTreeRoot,
) -> Vec<ReachabilityTreeRootSnapshot> {
    let mut merged = experience_snapshot;
    if !board_root.children.is_empty() {
        if let Some(scenes) = merged.first_mut() {
            let _ = scenes;
        }
        merged.insert(1, root_to_snapshot(board_root));
    }
    if !template_root.children.is_empty() {
        let insert_at = if merged.len() > 1 && merged[1].group == "boards" {
            2
        } else {
            1
        };
        merged.insert(insert_at, root_to_snapshot(template_root));
    }
    if !template_files_root.children.is_empty() {
        let insert_at = merged
            .iter()
            .position(|root| root.group == "templates")
            .map(|idx| idx + 1)
            .unwrap_or_else(|| {
                if merged.len() > 1 && merged[1].group == "boards" {
                    2
                } else {
                    1
                }
            });
        merged.insert(insert_at, root_to_snapshot(template_files_root));
    }
    merged
}

pub fn reachability_roots_from_compiled(compiled: &CompiledApp) -> Vec<ReachabilityTreeRoot> {
    let mut roots = if build_view_reachability_stale(compiled) {
        rebuild_reachability_tree_from_compiled(compiled)
    } else {
        let mut roots = compiled
            .build_experience_index
            .reachability_snapshot
            .iter()
            .map(snapshot_to_root)
            .collect();
        ensure_board_and_template_roots(&mut roots, compiled);
        roots
    };
    enrich_reachability_tree_compile_coords(&mut roots, compiled);
    normalize_reachability_tree_roots(&mut roots);
    roots
}

fn normalize_reachability_tree_roots(roots: &mut [ReachabilityTreeRoot]) {
    for root in roots {
        if root.group == "templates" {
            root.label = "Components".to_string();
        }
        if root.group == "template_files" {
            root.label = "Templates".to_string();
        }
    }
}

pub fn enrich_reachability_tree_compile_coords(
    roots: &mut [ReachabilityTreeRoot],
    compiled: &CompiledApp,
) {
    for root in roots {
        for child in &mut root.children {
            enrich_node_compile_coords(child, compiled);
        }
    }
}

fn enrich_node_compile_coords(node: &mut ReachabilityTreeNode, compiled: &CompiledApp) {
    if let Some(parsed) = BuildNodeId::parse(&node.node_id) {
        if let Some(coord) = super::build_experience::compile_coordinate_for_node(&parsed, compiled)
        {
            node.compile_scene = coord.scene_id.unwrap_or_default();
            node.compile_target = coord.preview_target;
        }
    }
    for child in &mut node.children {
        enrich_node_compile_coords(child, compiled);
    }
}

fn build_view_reachability_stale(compiled: &CompiledApp) -> bool {
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
    let expects_templates = !compiled.component_assets.is_empty()
        || !compiled.build_template_index.templates.is_empty()
        || workspace_component_catalog_from_app(compiled).is_some();
    if expects_templates {
        let templates_ok = snapshot
            .iter()
            .any(|root| root.group == "templates" && !root.children.is_empty());
        if !templates_ok {
            return true;
        }
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

fn rebuild_reachability_tree_from_compiled(compiled: &CompiledApp) -> Vec<ReachabilityTreeRoot> {
    let contracts = scene_contracts_from_compiled(compiled);
    let mut file_tree = compiled.file_tree.clone();
    let app_root = Path::new(compiled.app_root.as_str());
    if app_root.is_dir() {
        let _ = super::source_tree_enrich::enrich_source_tree_with_scene_exports(
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
    let template = crate::compile::build_template_index(
        &template_catalog_for_tree(compiled, source_root_from_app(compiled).as_path()),
        &contracts,
        &experience.node_manifest,
    );
    let template_files = crate::compile::build_template_index::build_stock_template_files_root(
        &source_root_from_app(compiled),
    );
    merge_build_view_tree_roots(
        experience.reachability_snapshot,
        board.tree_root,
        template.tree_root,
        template_files,
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

fn ensure_board_and_template_roots(roots: &mut Vec<ReachabilityTreeRoot>, compiled: &CompiledApp) {
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
    if !roots.iter().any(|root| root.group == "templates") {
        let template_root = if !compiled.build_template_index.templates.is_empty() {
            Some(
                crate::compile::build_template_index::template_tree_root_from_index(
                    &compiled.build_template_index,
                ),
            )
        } else {
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
    } else if let Some(existing) = roots.iter_mut().find(|root| root.group == "templates") {
        if existing.children.is_empty() {
            let template_root = if !compiled.build_template_index.templates.is_empty() {
                crate::compile::build_template_index::template_tree_root_from_index(
                    &compiled.build_template_index,
                )
            } else {
                let contracts = scene_contracts_from_compiled(compiled);
                let catalog =
                    template_catalog_for_tree(compiled, source_root_from_app(compiled).as_path());
                if !catalog.is_empty() {
                    crate::compile::build_template_index::build_template_index(
                        &catalog,
                        &contracts,
                        &compiled.build_experience_index.node_manifest,
                    )
                    .tree_root
                } else {
                    ReachabilityTreeRoot {
                        group: "templates".to_string(),
                        label: "Components".to_string(),
                        default_open: false,
                        children: Vec::new(),
                    }
                }
            };
            if !template_root.children.is_empty() {
                *existing = template_root;
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

fn workspace_component_catalog_from_app(compiled: &CompiledApp) -> Option<Vec<ComponentAsset>> {
    let app_root = Path::new(compiled.app_root.as_str());
    let source_root = app_root.parent()?;
    let map = crate::workspace::load_component_assets(source_root).ok()?;
    if map.is_empty() {
        return None;
    }
    Some(map.values().cloned().collect())
}

fn source_root_from_app(compiled: &CompiledApp) -> PathBuf {
    Path::new(compiled.app_root.as_str())
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(compiled.app_root.as_str()))
}

fn template_catalog_for_tree(compiled: &CompiledApp, source_root: &Path) -> Vec<ComponentAsset> {
    crate::compile::build_template_index::merged_component_catalog(source_root, compiled)
}

fn root_to_snapshot(root: ReachabilityTreeRoot) -> ReachabilityTreeRootSnapshot {
    ReachabilityTreeRootSnapshot {
        group: root.group,
        label: root.label,
        default_open: root.default_open,
        children: root.children.into_iter().map(node_to_snapshot).collect(),
    }
}

fn snapshot_to_root(snapshot: &ReachabilityTreeRootSnapshot) -> ReachabilityTreeRoot {
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

fn panels_for_scene_from_maps(
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

fn scene_route_label(route: &CompiledSceneRoute) -> String {
    route
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| route.scene_id.clone())
}

fn collect_panel_subtree(
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

fn disambiguate_tree_node_labels(nodes: &mut [ReachabilityTreeNode]) {
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

fn projection_children(scene_id: &str, assembly: &Value, kind: &str) -> Vec<ReachabilityTreeNode> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SceneDecl;

    fn sample_scene_contract(panels: Vec<PanelDecl>) -> SceneContract {
        SceneContract {
            scene: SceneDecl {
                kind: "scene".to_string(),
                id: "home".to_string(),
                profile: Some("cockpit".to_string()),
                state: Value::Null,
                world: None,
                flow: None,
                frame: None,
                theme: None,
                summary: None,
                goal: None,
                shared: Value::Null,
                local_nav: Value::Null,
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
        }
    }

    #[test]
    fn experience_index_expands_nested_panels() {
        let inner = PanelDecl {
            kind: "panel".to_string(),
            id: "supervision_warning_stats".to_string(),
            title: Some("监督预警".to_string()),
            head: None,
            area: Some("warning".to_string()),
            layout: None,
            blocks: vec![UiNodeDecl::Block(BlockDecl {
                kind: "block".to_string(),
                use_key: "cockpit.metric-card".to_string(),
                id: Some("card_one".to_string()),
                title: Some("预警数".to_string()),
                area: None,
                props: Value::Null,
                base: None,
                layout: None,
                blocks: Vec::new(),
                component: None,
                placement: None,
                interactions: Vec::new(),
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            slot: None,
            props: Value::Null,
            head_props: Value::Null,
            body_props: Value::Null,
            base: None,
            import_scope: Some("scenes/05-监督预警.mei".to_string()),
        };
        let shell = PanelDecl {
            kind: "panel".to_string(),
            id: "right_rail_float".to_string(),
            title: None,
            head: None,
            area: Some("body".to_string()),
            layout: None,
            blocks: vec![UiNodeDecl::Panel(inner)],
            slot: None,
            props: serde_json::json!({
                "position": "absolute",
                "top": "84px",
                "right": "0",
            }),
            head_props: Value::Null,
            body_props: Value::Null,
            base: None,
            import_scope: Some("scenes/layout-右栏.mei".to_string()),
        };
        let mut contracts = BTreeMap::new();
        contracts.insert("home".to_string(), sample_scene_contract(vec![shell]));
        let routes = vec![CompiledSceneRoute {
            scene_id: "home".to_string(),
            frame_id: None,
            target_file: "scenes/home.mei".to_string(),
            kind: "file_ref".to_string(),
            title: Some("首页".to_string()),
            is_default: true,
            access_export: true,
        }];
        let compiled_stub = CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: ".".to_string(),
            scene_routes: routes.clone(),
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
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
            build_experience_index: BuildExperienceIndex::default(),
            build_board_index: Default::default(),
            build_template_index: Default::default(),
        };
        let index = build_experience_index(&routes, &BTreeMap::new(), &contracts, &compiled_stub);
        let nested_id =
            BuildNodeId::scene_panel("home", "right_rail_float/supervision_warning_stats").encode();
        let nested = index
            .node_manifest
            .get(&nested_id)
            .expect("nested panel manifest");
        assert_eq!(nested.label, "监督预警");
        assert!(nested
            .mount_chain
            .iter()
            .any(|entry| entry.file.contains("05-监督预警")));
        let shell_id = BuildNodeId::scene_panel("home", "right_rail_float").encode();
        let shell = index
            .node_manifest
            .get(&shell_id)
            .expect("shell panel manifest");
        assert!(shell
            .mount_chain
            .iter()
            .any(|entry| entry.file.contains("layout")));
        let scenes = &index.reachability_snapshot[0];
        let home = scenes.children.first().expect("home scene");
        let panels = home
            .children
            .iter()
            .find(|node| node.label == "Panels")
            .expect("panels group");
        assert_eq!(panels.children.len(), 1);
        assert_eq!(panels.children[0].children.len(), 1);
        assert_eq!(panels.children[0].children[0].label, "监督预警");
    }

    #[test]
    fn stale_snapshot_rebuilds_boards_and_templates_groups() {
        use std::path::Path;

        use crate::compile::{compile_app_from_root_with_options, CompileOptions};

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root")
            .join("workspaces")
            .join("ws-spbjw");
        let app_root = source_root.join("zhifa");
        let compiled =
            compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
                .expect("compile zhifa");
        let mut stale = compiled;
        stale.build_experience_index = BuildExperienceIndex::default();
        stale.build_board_index = Default::default();
        stale.build_template_index = Default::default();

        let roots = reachability_roots_from_compiled(&stale);
        let groups: Vec<_> = roots.iter().map(|root| root.group.as_str()).collect();
        assert!(
            groups.contains(&"boards"),
            "expected boards group after stale rebuild, got {groups:?}"
        );
        assert!(
            groups.contains(&"templates"),
            "expected templates group after stale rebuild, got {groups:?}"
        );
        assert!(
            !groups.contains(&"components"),
            "stale rebuild should not fall back to legacy runtime-only components group"
        );
        let boards = roots
            .iter()
            .find(|root| root.group == "boards")
            .expect("boards group");
        assert!(
            !boards.children.is_empty(),
            "boards group should list board capsules"
        );
    }

    #[test]
    fn partial_snapshot_restores_templates_from_component_assets() {
        use std::path::Path;

        use crate::compile::{compile_app_from_root_with_options, CompileOptions};

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root")
            .join("workspaces")
            .join("ws-spbjw");
        let app_root = source_root.join("zhifa");
        let compiled =
            compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
                .expect("compile zhifa");
        assert!(
            !compiled.component_assets.is_empty(),
            "fixture should expose component assets"
        );
        let mut partial = compiled.clone();
        partial.build_template_index = Default::default();
        partial
            .build_experience_index
            .reachability_snapshot
            .retain(|root| root.group != "templates");

        let roots = reachability_roots_from_compiled(&partial);
        assert!(
            roots.iter().any(|root| root.group == "templates"),
            "templates group should be restored from component_assets, groups: {:?}",
            roots
                .iter()
                .map(|root| root.group.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn empty_templates_snapshot_is_treated_as_stale() {
        use std::path::Path;

        use crate::compile::{compile_app_from_root_with_options, CompileOptions};

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root")
            .join("workspaces")
            .join("ws-spbjw");
        let app_root = source_root.join("zhifa");
        let compiled =
            compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
                .expect("compile zhifa");
        let mut partial = compiled.clone();
        partial.build_experience_index.reachability_snapshot = partial
            .build_experience_index
            .reachability_snapshot
            .into_iter()
            .map(|mut root| {
                if root.group == "templates" {
                    root.children.clear();
                }
                root
            })
            .collect();

        let roots = reachability_roots_from_compiled(&partial);
        let templates = roots
            .iter()
            .find(|root| root.group == "templates")
            .expect("templates group");
        assert!(
            !templates.children.is_empty(),
            "empty templates snapshot should be rebuilt with component catalog entries"
        );
    }

    #[test]
    fn templates_group_renders_as_components_label() {
        use std::path::Path;

        use crate::compile::{compile_app_from_root_with_options, CompileOptions};

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root")
            .join("workspaces")
            .join("ws-spbjw");
        let app_root = source_root.join("zhifa");
        let compiled =
            compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
                .expect("compile zhifa");
        let roots = reachability_roots_from_compiled(&compiled);
        let components = roots
            .iter()
            .find(|root| root.group == "templates")
            .expect("templates/components group");
        assert_eq!(components.label, "Components");
    }
}
