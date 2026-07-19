use super::insert_hydrated_link_projection_assembly_entry;

use std::{collections::BTreeMap, path::Path};

use serde_json::Value;

use crate::compile::entry_payload::compile_scene_payload_for_target_uncached;
use crate::evaluate_mei_file;
use crate::model::{CompiledSceneRoute, ComponentAsset, Diagnostic, SceneContract, WorkspaceNode};
use crate::typed_refs::SceneRegistry;

use super::super::super::scene::SceneRouteRegistry;

pub(super) fn hydrate_board_capsules_from_file_tree(
    app_root: &Path,
    app_main: &Path,
    asset_map: &BTreeMap<String, ComponentAsset>,
    route_registry: &SceneRouteRegistry,
    file_tree: &[WorkspaceNode],
    scene_projection_assembly_by_id: &mut BTreeMap<String, Value>,
    scene_bindings_by_id: &mut BTreeMap<String, Value>,
    scene_examples_by_id: &mut BTreeMap<String, Value>,
    scene_local_nav_by_target: &mut BTreeMap<String, Value>,
    target_scene_contracts: &mut BTreeMap<String, SceneContract>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Ok(app_decls) = evaluate_mei_file(app_main) else {
        return;
    };
    let scene_registry = SceneRegistry::build_from_routes(&route_registry.routes);
    walk_hydrate_board_nodes(
        app_root,
        &app_decls,
        asset_map,
        &scene_registry,
        file_tree,
        scene_projection_assembly_by_id,
        scene_bindings_by_id,
        scene_examples_by_id,
        scene_local_nav_by_target,
        target_scene_contracts,
        diagnostics,
    );
}

fn walk_hydrate_board_nodes(
    app_root: &Path,
    app_decls: &Value,
    asset_map: &BTreeMap<String, ComponentAsset>,
    scene_registry: &SceneRegistry,
    nodes: &[WorkspaceNode],
    scene_projection_assembly_by_id: &mut BTreeMap<String, Value>,
    scene_bindings_by_id: &mut BTreeMap<String, Value>,
    scene_examples_by_id: &mut BTreeMap<String, Value>,
    scene_local_nav_by_target: &mut BTreeMap<String, Value>,
    target_scene_contracts: &mut BTreeMap<String, SceneContract>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node in nodes {
        if node.kind == "dir" {
            walk_hydrate_board_nodes(
                app_root,
                app_decls,
                asset_map,
                scene_registry,
                &node.children,
                scene_projection_assembly_by_id,
                scene_bindings_by_id,
                scene_examples_by_id,
                scene_local_nav_by_target,
                target_scene_contracts,
                diagnostics,
            );
            continue;
        }
        if node.kind != "file"
            || !(node.path.ends_with(".board.mei") || node.path.ends_with(".page.mei"))
        {
            continue;
        }
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
            if scene_projection_assembly_by_id.contains_key(scene_id)
                && target_scene_contracts.contains_key(scene_id)
            {
                continue;
            }
            let route_meta = CompiledSceneRoute {
                scene_id: scene_id.to_string(),
                frame_id: None,
                target_file: node.path.clone(),
                kind: "board_capsule".to_string(),
                title: Some(export.name.clone()),
                short_title: None,
                is_default: false,
                access_export: false,
            };
            let payload = compile_scene_payload_for_target_uncached(
                app_root,
                app_decls,
                asset_map,
                node.path.as_str(),
                Some(&route_meta),
                scene_registry,
            );
            diagnostics.extend(payload.diagnostics.clone());
            let Some(contract) = payload.scene_contract.as_ref() else {
                continue;
            };
            target_scene_contracts.insert(scene_id.to_string(), contract.clone());
            insert_hydrated_link_projection_assembly_entry(
                scene_projection_assembly_by_id,
                scene_bindings_by_id,
                scene_examples_by_id,
                scene_local_nav_by_target,
                scene_id,
                node.path.as_str(),
                contract,
                &payload.resources,
                diagnostics,
            );
        }
    }
}
