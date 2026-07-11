use super::{
    insert_hydrated_link_projection_assembly_entry, insert_scene_projection_assembly_entry,
};

use std::{collections::BTreeMap, path::Path};

use serde_json::Value;

use crate::compile::entry_payload::CompiledScenePayload;
use crate::evaluate_mei_file;
use crate::model::{ComponentAsset, Diagnostic};
use crate::typed_refs::SceneRegistry;

use super::super::super::dependency_graph::DependencyGraph;
use super::super::super::scene::SceneRouteRegistry;
use super::super::super::scene_payload_cache::compile_scene_payload_for_target;

pub(super) fn build_scene_projection_maps(
    route_registry: &SceneRouteRegistry,
    official_results: &BTreeMap<String, CompiledScenePayload>,
    active_scene: Option<&str>,
    active_target_file: &str,
    active_payload: &CompiledScenePayload,
    hydrated_link_targets: &BTreeMap<String, (String, CompiledScenePayload)>,
    diagnostics: &mut Vec<Diagnostic>,
) -> (
    BTreeMap<String, Value>,
    BTreeMap<String, Value>,
    BTreeMap<String, Value>,
    BTreeMap<String, Value>,
) {
    let mut scene_local_nav_by_target = BTreeMap::new();
    let mut scene_bindings_by_id = BTreeMap::new();
    let mut scene_examples_by_id = BTreeMap::new();
    let mut scene_projection_assembly_by_id = BTreeMap::new();
    for route in &route_registry.routes {
        let Some(payload) = official_results.get(&route.scene_id) else {
            continue;
        };
        let Some(contract) = payload.scene_contract.as_ref() else {
            continue;
        };
        insert_scene_projection_assembly_entry(
            &mut scene_projection_assembly_by_id,
            &mut scene_bindings_by_id,
            &mut scene_examples_by_id,
            &mut scene_local_nav_by_target,
            &route.scene_id,
            &route.target_file,
            Some(route.kind.as_str()),
            route.title.as_deref(),
            contract,
            &payload.resources,
            diagnostics,
        );
    }
    for (scene_id, (target_file, payload)) in hydrated_link_targets {
        if scene_projection_assembly_by_id.contains_key(scene_id.as_str()) {
            continue;
        }
        let Some(contract) = payload.scene_contract.as_ref() else {
            continue;
        };
        insert_hydrated_link_projection_assembly_entry(
            &mut scene_projection_assembly_by_id,
            &mut scene_bindings_by_id,
            &mut scene_examples_by_id,
            &mut scene_local_nav_by_target,
            scene_id,
            target_file,
            contract,
            &payload.resources,
            diagnostics,
        );
    }
    if let Some(contract) = active_payload.scene_contract.as_ref() {
        if let Some(active_scene_id) = active_scene {
            let assembly_entry = scene_projection_assembly_by_id
                .entry(active_scene_id.to_string())
                .or_insert_with(|| {
                    let mut assembly = serde_json::Map::new();
                    assembly.insert(
                        "scene_id".to_string(),
                        Value::String(active_scene_id.to_string()),
                    );
                    assembly.insert(
                        "target_file".to_string(),
                        Value::String(active_target_file.to_string()),
                    );
                    Value::Object(assembly)
                });
            if let Some(assembly_map) = assembly_entry.as_object_mut() {
                assembly_map.insert(
                    "target_file".to_string(),
                    Value::String(active_target_file.to_string()),
                );
                if !contract.scene.bindings.is_null() {
                    assembly_map.insert("bindings".to_string(), contract.scene.bindings.clone());
                }
                if !contract.scene.examples.is_null() {
                    assembly_map.insert("examples".to_string(), contract.scene.examples.clone());
                }
                if !contract.scene.local_nav.is_null() {
                    assembly_map.insert("local_nav".to_string(), contract.scene.local_nav.clone());
                }
                if !contract.scene.params.is_null() {
                    assembly_map.insert("params".to_string(), contract.scene.params.clone());
                    assembly_map.insert("accepts".to_string(), contract.scene.params.clone());
                }
                if !contract.scene.capabilities.is_null() {
                    assembly_map.insert(
                        "capabilities".to_string(),
                        contract.scene.capabilities.clone(),
                    );
                }
                if let Some(frame) = contract.frame.as_ref() {
                    assembly_map.insert(
                        "frame".to_string(),
                        serde_json::to_value(frame).unwrap_or(Value::Null),
                    );
                }
                if !contract.panels.is_empty() {
                    assembly_map.insert(
                        "panels".to_string(),
                        serde_json::to_value(&contract.panels).unwrap_or(Value::Null),
                    );
                }
                if let Some(shell_contract) =
                    crate::compile::projection_assembly::scene_shell_contract_from_scene_contract(
                        contract,
                    )
                {
                    assembly_map
                        .insert("shell_contract".to_string(), Value::Object(shell_contract));
                }
                crate::compile::projection_assembly::enrich_scene_projection_assembly_preview(
                    assembly_map,
                    contract,
                    &active_payload.resources,
                    active_target_file,
                    diagnostics,
                );
            }
            if !contract.scene.bindings.is_null() {
                scene_bindings_by_id
                    .insert(active_scene_id.to_string(), contract.scene.bindings.clone());
            }
            if !contract.scene.examples.is_null() {
                scene_examples_by_id
                    .insert(active_scene_id.to_string(), contract.scene.examples.clone());
            }
        }
        if !contract.scene.local_nav.is_null() {
            scene_local_nav_by_target.insert(
                active_target_file.to_string(),
                contract.scene.local_nav.clone(),
            );
        }
    }
    (
        scene_local_nav_by_target,
        scene_bindings_by_id,
        scene_examples_by_id,
        scene_projection_assembly_by_id,
    )
}

fn assembly_has_panels(entry: Option<&Value>) -> bool {
    entry
        .and_then(|value| value.get("panels"))
        .and_then(|value| value.as_array())
        .is_some_and(|panels| !panels.is_empty())
}

pub(super) fn ensure_build_tree_entry_scene_assemblies(
    app_root: &Path,
    app_main: &Path,
    asset_map: &BTreeMap<String, ComponentAsset>,
    dependency_graph: &DependencyGraph,
    route_registry: &SceneRouteRegistry,
    active_target_file: &str,
    scene_projection_assembly_by_id: &mut BTreeMap<String, Value>,
    scene_bindings_by_id: &mut BTreeMap<String, Value>,
    scene_examples_by_id: &mut BTreeMap<String, Value>,
    scene_local_nav_by_target: &mut BTreeMap<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !is_t2_page_target(active_target_file) {
        return;
    }
    let Ok(app_decls) = evaluate_mei_file(app_main) else {
        return;
    };
    let source_root = app_root.parent().unwrap_or(app_root);
    let scene_registry = SceneRegistry::build_from_routes(&route_registry.routes);
    for route in &route_registry.routes {
        if is_t2_page_target(route.target_file.as_str()) {
            continue;
        }
        if assembly_has_panels(scene_projection_assembly_by_id.get(&route.scene_id)) {
            continue;
        }
        let dependency_fingerprint = dependency_graph.dependency_fingerprint_for_target(
            app_root,
            &app_decls,
            route.target_file.as_str(),
        );
        let payload = compile_scene_payload_for_target(
            app_root,
            source_root,
            &app_decls,
            asset_map,
            route.target_file.as_str(),
            Some(route),
            &scene_registry,
            dependency_fingerprint.as_deref(),
        );
        let Some(contract) = payload.scene_contract.as_ref() else {
            continue;
        };
        insert_scene_projection_assembly_entry(
            scene_projection_assembly_by_id,
            scene_bindings_by_id,
            scene_examples_by_id,
            scene_local_nav_by_target,
            &route.scene_id,
            &route.target_file,
            Some(route.kind.as_str()),
            route.title.as_deref(),
            contract,
            &payload.resources,
            diagnostics,
        );
    }
}

fn is_t2_page_target(target_file: &str) -> bool {
    target_file.ends_with(".page.mei") || target_file.ends_with(".board.mei")
}
