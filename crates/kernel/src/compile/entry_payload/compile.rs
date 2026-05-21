use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use crate::model::{
    CompiledSceneRoute, ComponentAsset, Diagnostic, FlowDecl, FrameDecl, PanelDecl, SceneContract,
    SceneDecl, Severity, ThemeDecl,
};

use super::super::decls::{
    FrameSetLayoutDecl, LegacyDatasetDecl, LegacyMetricPackDecl, WorldAddEntityDecl,
    WorldAddResourceDecl, WorldSetTopologyDecl,
};
use super::super::load_external::{
    load_flow_from_file, load_frame_from_file, load_panel_from_scene_file, load_world_from_file,
};
use super::super::materialize::{materialize_legacy_datasets, materialize_metric_packs};
use super::super::mutations::{apply_frame_mutations, apply_world_mutations};
use super::super::resources::load_resources;
use super::super::scene_binding::{
    decode_scene_decl, parse_flow_binding, parse_frame_binding, parse_world_binding,
    pick_only_frame, pick_only_world, SceneBinding,
};
use super::super::ui_data_policy::validate_scene_ui_data_bindings;
use super::helpers::{
    all_world_resource_decls, collect_asset_keys_from_nodes, decode_world_dataset_decl,
    decode_world_metric_pack_decl, insert_resource_checked, partition_world_resources,
};
use crate::typed_refs::{decode_ref_value, RefKind, SceneRegistry};
use super::CompiledScenePayload;
pub(super) fn compile_scene_payload(
    app_root: &Path,
    asset_map: &std::collections::BTreeMap<String, ComponentAsset>,
    target_file: &str,
    entry_decls: &Value,
    route_meta: Option<&CompiledSceneRoute>,
    scene_registry: &SceneRegistry,
) -> Result<CompiledScenePayload> {
    let mut diagnostics = Vec::new();
    let mut scenes: BTreeMap<String, SceneDecl> = BTreeMap::new();
    let mut frames: BTreeMap<String, FrameDecl> = BTreeMap::new();
    let mut worlds: BTreeMap<String, crate::model::WorldDecl> = BTreeMap::new();
    let mut flows: BTreeMap<String, FlowDecl> = BTreeMap::new();
    let mut scene_decl_count = 0usize;
    let mut frame_decl_count = 0usize;
    let mut world_decl_count = 0usize;
    let mut world_topology_set_count = 0usize;
    let mut frame_layout_set_count = 0usize;
    let mut frame_default: Option<FrameDecl> = None;
    let mut world_default: Option<crate::model::WorldDecl> = None;
    let mut flow_default: Option<FlowDecl> = None;
    let mut pending_world_resources = Vec::new();
    let mut pending_world_entities = Vec::new();
    let mut pending_world_topology: Option<crate::model::WorldGridDecl> = None;
    let mut pending_frame_layout: Option<crate::model::LayoutDecl> = None;
    let mut themes: Vec<ThemeDecl> = Vec::new();
    let mut panels: Vec<PanelDecl> = Vec::new();
    let mut top_level_legacy_dataset_count = 0usize;
    let mut top_level_legacy_dataset_view_count = 0usize;
    let mut top_level_legacy_metric_pack_count = 0usize;
    let mut seen_world_decl = false;
    let mut first_scene_decl_index: Option<usize> = None;
    let mut first_world_decl_index: Option<usize> = None;

    if let Some(values) = entry_decls.as_array() {
        for (decl_index, value) in values.iter().enumerate() {
            if value.get("dataset").is_some() && value.get("schema_version").is_some() {
                top_level_legacy_dataset_count += 1;
                continue;
            }
            if value.get("metric_pack").is_some() && value.get("schema_version").is_some() {
                top_level_legacy_metric_pack_count += 1;
                continue;
            }
            let Some(kind) = value.get("kind").and_then(Value::as_str) else {
                if let Some(component) = value.get("component") {
                    if matches!(
                        component.get("block_kind").and_then(Value::as_str),
                        Some("panel_ref")
                            | Some("panel_capsule_ref")
                            | Some("frame_ref")
                    ) {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "top_level_panel_ref_embed".to_string(),
                            message: "panel_ref(scene_file=..., area=...) block embed must appear inside frame.add_panel(...).blocks, not at scene top level"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                }
                continue;
            };
            match kind {
                "frame" => {
                    frame_decl_count += 1;
                    let frame_decl = serde_json::from_value::<FrameDecl>(value.clone())?;
                    if let Some(id) = frame_decl
                        .id
                        .as_deref()
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                    {
                        frames.insert(id.to_string(), frame_decl);
                    } else {
                        if frame_default.is_none() {
                            frame_default = Some(frame_decl);
                        }
                    }
                }
                "scene" => {
                    if first_scene_decl_index.is_none() {
                        first_scene_decl_index = Some(decl_index);
                    }
                    scene_decl_count += 1;
                    let scene_decl = decode_scene_decl(value, target_file)?;
                    scenes.insert(scene_decl.id.clone(), scene_decl);
                }
                "world" => {
                    if first_world_decl_index.is_none() {
                        first_world_decl_index = Some(decl_index);
                    }
                    world_decl_count += 1;
                    let world_decl =
                        serde_json::from_value::<crate::model::WorldDecl>(value.clone())?;
                    if let Some(id) = world_decl
                        .id
                        .as_deref()
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                    {
                        worlds.insert(id.to_string(), world_decl);
                    } else {
                        if world_default.is_none() {
                            world_default = Some(world_decl);
                        }
                    }
                    seen_world_decl = true;
                }
                "world_add_resource" => {
                    if !seen_world_decl {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "world_mutation_before_world_decl".to_string(),
                            message: "`world.add_*` / `world.set_topology(...)` must appear after `world(...)` in the same file (_declare order)"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                    let decl = serde_json::from_value::<WorldAddResourceDecl>(value.clone())?;
                    if decl.kind == "world_add_resource" {
                        pending_world_resources.push(decl.resource);
                    }
                }
                "world_add_entity" => {
                    if !seen_world_decl {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "world_mutation_before_world_decl".to_string(),
                            message: "`world.add_*` / `world.set_topology(...)` must appear after `world(...)` in the same file (_declare order)"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                    let decl = serde_json::from_value::<WorldAddEntityDecl>(value.clone())?;
                    if decl.kind == "world_add_entity" {
                        pending_world_entities.push(decl.entity);
                    }
                }
                "world_set_topology" => {
                    if !seen_world_decl {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "world_mutation_before_world_decl".to_string(),
                            message: "`world.add_*` / `world.set_topology(...)` must appear after `world(...)` in the same file (_declare order)"
                                .to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                    }
                    let decl = serde_json::from_value::<WorldSetTopologyDecl>(value.clone())?;
                    if decl.kind == "world_set_topology" {
                        world_topology_set_count += 1;
                        if pending_world_topology.is_none() {
                            pending_world_topology = Some(decl.topology);
                        }
                    }
                }
                "frame_set_layout" => {
                    let decl = serde_json::from_value::<FrameSetLayoutDecl>(value.clone())?;
                    if decl.kind == "frame_set_layout" {
                        frame_layout_set_count += 1;
                        if pending_frame_layout.is_none() {
                            pending_frame_layout = Some(serde_json::from_value::<
                                crate::model::LayoutDecl,
                            >(decl.layout)?);
                        }
                    }
                }
                "flow" => {
                    let flow_decl = serde_json::from_value::<FlowDecl>(value.clone())?;
                    if let Some(id) = flow_decl
                        .id
                        .as_deref()
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                    {
                        flows.insert(id.to_string(), flow_decl);
                    } else {
                        flow_default = Some(flow_decl);
                    }
                }
                "theme" => themes.push(serde_json::from_value(value.clone())?),
                "panel" => panels.push(serde_json::from_value(value.clone())?),
                "dataset_view" => top_level_legacy_dataset_view_count += 1,
                "app" | "app_scene_ref" => {}
                _ => diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "unknown_decl".to_string(),
                    message: format!("unknown declaration kind `{kind}`"),
                    source_path: Some(target_file.to_string()),
                }),
            }
        }
    }
    if let (Some(scene_idx), Some(world_idx)) = (first_scene_decl_index, first_world_decl_index) {
        if world_idx < scene_idx {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "world_before_scene_decl".to_string(),
                message: "`world(...)` must appear after `scene(...)` in the same file when both are declared (_declare order)"
                    .to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
    }
    let has_dataset_library_content = top_level_legacy_dataset_count > 0
        || top_level_legacy_metric_pack_count > 0
        || top_level_legacy_dataset_view_count > 0;
    let had_pending_topology = pending_world_topology.is_some();
    let had_pending_frame_layout = pending_frame_layout.is_some();
    let has_authoring_surface = scene_decl_count > 0
        || frame_decl_count > 0
        || world_decl_count > 0
        || !flows.is_empty()
        || flow_default.is_some()
        || !panels.is_empty()
        || !themes.is_empty()
        || world_topology_set_count > 0
        || frame_layout_set_count > 0
        || !pending_world_resources.is_empty()
        || !pending_world_entities.is_empty()
        || had_pending_topology
        || had_pending_frame_layout;
    let dataset_library_only =
        has_dataset_library_content && !has_authoring_surface && target_file != "main.mei";

    if top_level_legacy_dataset_count > 0
        || top_level_legacy_dataset_view_count > 0
        || top_level_legacy_metric_pack_count > 0
    {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "forbidden_top_level_dataset_decls".to_string(),
            message: "world-only mode forbids top-level dataset()/dataset_view()/metric_pack(); use world.add_dataset()/world.add_dataset_view()/world.add_metric_pack() or world resources list".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    if world_topology_set_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_world_topologies".to_string(),
            message: format!(
                "file `{target_file}` declares {world_topology_set_count} world.set_topology(...) blocks, expected at most one"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    if frame_layout_set_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_frame_layouts".to_string(),
            message: format!(
                "file `{target_file}` declares {frame_layout_set_count} frame.set_layout(...) blocks, expected at most one"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    apply_world_mutations(
        &mut worlds,
        &mut world_default,
        &pending_world_resources,
        &pending_world_entities,
        pending_world_topology,
        &mut diagnostics,
        target_file,
        world_decl_count,
    );
    apply_frame_mutations(
        &mut frames,
        &mut frame_default,
        pending_frame_layout,
        &mut diagnostics,
        target_file,
        frame_decl_count,
    );
    merge_frame_panel_slots(
        app_root,
        &frames,
        frame_default.as_ref(),
        &mut panels,
        scene_registry,
        &mut diagnostics,
        target_file,
    );
    if scene_decl_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_scenes".to_string(),
            message: format!(
                "file `{target_file}` declares {scene_decl_count} scene(...) blocks, expected exactly one"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    if world_decl_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_worlds".to_string(),
            message: format!(
                "file `{target_file}` declares {world_decl_count} world(...) blocks, expected exactly one"
            ),
            source_path: Some(target_file.to_string()),
        });
    }
    if frame_decl_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_frames".to_string(),
            message: format!(
                "file `{target_file}` declares {frame_decl_count} frame(...) blocks, expected exactly one"
            ),
            source_path: Some(target_file.to_string()),
        });
    }

    let mut asset_keys = BTreeSet::new();
    for panel in &panels {
        collect_asset_keys_from_nodes(&panel.blocks, &mut asset_keys);
    }
    let component_assets = asset_keys
        .into_iter()
        .filter_map(|key| asset_map.get(&key).cloned())
        .collect::<Vec<ComponentAsset>>();

    let selected_scene = route_meta
        .and_then(|meta| scenes.get(meta.scene_id.as_str()).cloned())
        .or_else(|| {
            if scenes.len() == 1 {
                scenes.values().next().cloned()
            } else {
                None
            }
        });
    let requires_scene_contract =
        (route_meta.is_some() || target_file != "main.mei") && !dataset_library_only;
    if requires_scene_contract && selected_scene.is_none() {
        let is_legacy_fragment = frame_decl_count > 0
            || !panels.is_empty()
            || world_decl_count > 0
            || frame_default.is_some()
            || world_default.is_some();
        if is_legacy_fragment {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "public_fragment_file_deprecated".to_string(),
                message: "legacy frame/world/panel fragment without scene(...); migrate to a minimal scene capsule with scene(...) and typed refs (world_ref/frame_ref)".to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_scene".to_string(),
            message: "scene file must declare scene(...) for scene-first authoring".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    let frame = if let Some(frame_id) = route_meta
        .and_then(|meta| meta.frame_id.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| id.to_string())
    {
        let matched = frames.get(frame_id.as_str()).cloned();
        if matched.is_none() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_bound_frame".to_string(),
                message: format!("declared frame `{frame_id}` was not found"),
                source_path: Some(target_file.to_string()),
            });
        }
        matched
    } else if let Some(scene_decl) = selected_scene.as_ref() {
        let binding = scene_decl
            .frame
            .as_ref()
            .map(|value| parse_frame_binding(value, Some(scene_registry)));
        match binding {
            Some(Ok(SceneBinding::LocalId(frame_id))) => {
                let matched = frames.get(frame_id.as_str()).cloned();
                if matched.is_none() {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "missing_bound_frame".to_string(),
                        message: format!("declared frame `{frame_id}` was not found"),
                        source_path: Some(target_file.to_string()),
                    });
                }
                matched
            }
            Some(Ok(SceneBinding::FileRef { path, id })) => {
                match load_frame_from_file(app_root, path.as_str(), id.as_deref()) {
                    Ok(frame_decl) => Some(frame_decl),
                    Err(error) => {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "load_frame_ref_failed".to_string(),
                            message: error.to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                        None
                    }
                }
            }
            Some(Ok(SceneBinding::Absent)) => pick_only_frame(&frames, frame_default.clone()),
            Some(Err(message)) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "invalid_scene_frame_binding".to_string(),
                    message: message.to_string(),
                    source_path: Some(target_file.to_string()),
                });
                None
            }
            None => pick_only_frame(&frames, frame_default.clone()),
        }
    } else {
        pick_only_frame(&frames, frame_default.clone())
    };
    if selected_scene.is_some() && frame.is_none() && frame_decl_count == 0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_frame".to_string(),
            message: "scene route requires a frame(...) declaration or frame_ref(...)"
                .to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    let world = if let Some(scene_decl) = selected_scene.as_ref() {
        let binding = scene_decl
            .world
            .as_ref()
            .map(|value| parse_world_binding(value, Some(scene_registry)));
        match binding {
            Some(Ok(SceneBinding::LocalId(world_id))) => {
                let matched = worlds.get(world_id.as_str()).cloned();
                if matched.is_none() {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "missing_bound_world".to_string(),
                        message: format!("declared world `{world_id}` was not found"),
                        source_path: Some(target_file.to_string()),
                    });
                }
                matched
            }
            Some(Ok(SceneBinding::FileRef { path, id })) => {
                match load_world_from_file(app_root, path.as_str(), id.as_deref()) {
                    Ok(world_decl) => Some(world_decl),
                    Err(error) => {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "load_world_ref_failed".to_string(),
                            message: error.to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                        None
                    }
                }
            }
            Some(Ok(SceneBinding::Absent)) => pick_only_world(&worlds, world_default.clone()),
            Some(Err(message)) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "invalid_scene_world_binding".to_string(),
                    message: message.to_string(),
                    source_path: Some(target_file.to_string()),
                });
                None
            }
            None => pick_only_world(&worlds, world_default.clone()),
        }
    } else {
        pick_only_world(&worlds, world_default.clone())
    };
    if selected_scene.is_some() && world.is_none() && world_decl_count == 0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_world".to_string(),
            message: "scene entry requires a world(...) declaration or world_ref(...)"
                .to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    let flow = selected_scene
        .as_ref()
        .and_then(|scene| scene.flow.as_ref())
        .and_then(|value| {
            resolve_flow_binding(
                value,
                &flows,
                app_root,
                Some(scene_registry),
                &mut diagnostics,
                target_file,
            )
        })
        .or_else(|| {
            flow_default.clone().or_else(|| {
                (flows.len() == 1)
                    .then(|| flows.values().next().cloned())
                    .flatten()
            })
        });

    let mut resources = Vec::new();
    let mut world_dataset_decls: Vec<LegacyDatasetDecl> = Vec::new();
    let mut world_metric_pack_decls: Vec<LegacyMetricPackDecl> = Vec::new();
    if let Some(world_decl) = world.as_ref() {
        let (normal_resources, dataset_resources) =
            partition_world_resources(&all_world_resource_decls(world_decl));
        resources = load_resources(app_root, &normal_resources)?;
        for resource in dataset_resources {
            if resource.id == "__source_path__" || resource.id.ends_with(".mei") {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "forbidden_legacy_resource_id".to_string(),
                    message: format!(
                        "resource id `{}` is forbidden in world-only mode; use a stable explicit id",
                        resource.id
                    ),
                    source_path: Some(target_file.to_string()),
                });
                continue;
            }
            match resource.kind.as_str() {
                "dataset" | "dataset_view" => match decode_world_dataset_decl(resource.clone()) {
                    Ok(decl) => world_dataset_decls.push(decl),
                    Err(message) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_world_dataset_decl_failed".to_string(),
                        message,
                        source_path: Some(target_file.to_string()),
                    }),
                },
                "metric_pack" => match decode_world_metric_pack_decl(resource.clone()) {
                    Ok(decl) => world_metric_pack_decls.push(decl),
                    Err(message) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_world_metric_pack_decl_failed".to_string(),
                        message,
                        source_path: Some(target_file.to_string()),
                    }),
                },
                _ => diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "unsupported_world_resource_kind".to_string(),
                    message: format!(
                        "resource `{}` has unsupported kind `{}` in world-only mode",
                        resource.id, resource.kind
                    ),
                    source_path: Some(target_file.to_string()),
                }),
            }
        }
    }
    if !world_dataset_decls.is_empty() {
        let derived = materialize_legacy_datasets(app_root, &resources, &world_dataset_decls)?;
        for resource in derived {
            insert_resource_checked(&mut resources, resource, target_file, &mut diagnostics);
        }
    }
    if !world_metric_pack_decls.is_empty() {
        let derived = materialize_metric_packs(&resources, &world_metric_pack_decls)?;
        for resource in derived {
            insert_resource_checked(&mut resources, resource, target_file, &mut diagnostics);
        }
    }

    let scene_contract = selected_scene.map(|scene_decl| SceneContract {
        scene: scene_decl,
        themes,
        world,
        flow,
        frame,
        panels,
    });
    if let Some(ref contract) = scene_contract {
        super::helpers::merge_implicit_embed_capsule_resources(
            app_root,
            &contract.panels,
            &mut resources,
            target_file,
            &mut diagnostics,
        );
        validate_scene_ui_data_bindings(
            contract,
            &resources,
            app_root,
            target_file,
            &mut diagnostics,
        );
    }

    Ok(CompiledScenePayload {
        scene_contract,
        resources,
        component_assets,
        diagnostics,
    })
}

fn resolve_flow_binding(
    value: &Value,
    flows: &BTreeMap<String, FlowDecl>,
    app_root: &Path,
    scene_registry: Option<&SceneRegistry>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) -> Option<FlowDecl> {
    if let Some(id) = value.as_str().map(str::trim).filter(|id| !id.is_empty()) {
        if let Some(flow) = flows.get(id) {
            return Some(flow.clone());
        }
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_bound_flow".to_string(),
            message: format!("declared flow `{id}` was not found"),
            source_path: Some(target_file.to_string()),
        });
        return None;
    }
    match parse_flow_binding(value, scene_registry) {
        Ok(SceneBinding::LocalId(id)) => {
            if let Some(flow) = flows.get(id.as_str()) {
                return Some(flow.clone());
            }
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_bound_flow".to_string(),
                message: format!("declared flow `{id}` was not found"),
                source_path: Some(target_file.to_string()),
            });
            None
        }
        Ok(SceneBinding::FileRef { path, id }) => match load_flow_from_file(app_root, path.as_str(), id.as_deref())
        {
            Ok(flow_decl) => Some(flow_decl),
            Err(error) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: "load_flow_ref_failed".to_string(),
                    message: error.to_string(),
                    source_path: Some(target_file.to_string()),
                });
                None
            }
        },
        Ok(SceneBinding::Absent) => None,
        Err(message) => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_scene_flow_binding".to_string(),
                message: message.to_string(),
                source_path: Some(target_file.to_string()),
            });
            None
        }
    }
}

fn merge_frame_panel_slots(
    app_root: &Path,
    frames: &BTreeMap<String, FrameDecl>,
    frame_default: Option<&FrameDecl>,
    panels: &mut Vec<PanelDecl>,
    scene_registry: &SceneRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
) {
    let mut sources: Vec<&FrameDecl> = frames.values().collect();
    if let Some(frame) = frame_default {
        sources.push(frame);
    }
    for frame in sources {
        for slot in &frame.panels {
            if let Some(panel) = decode_panel_slot(slot) {
                upsert_panel(panels, panel);
                continue;
            }
            if let Some(expr) = decode_ref_value(slot) {
                if expr.kind == RefKind::Panel {
                    let panel_id = expr.id.as_deref().unwrap_or_default().trim();
                    if panel_id.is_empty() {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "invalid_panel_ref".to_string(),
                            message: "panel_ref(...) requires panel id".to_string(),
                            source_path: Some(target_file.to_string()),
                        });
                        continue;
                    }
                    let path = if let Some(path) = expr
                        .locator
                        .scene_file
                        .as_deref()
                        .map(str::trim)
                        .filter(|path| !path.is_empty())
                    {
                        path.to_string()
                    } else {
                        match scene_registry.resolve_target(&expr.locator) {
                            Ok((_, path)) => path,
                            Err(message) => {
                                diagnostics.push(Diagnostic {
                                    severity: Severity::Error,
                                    code: "panel_ref_not_resolved".to_string(),
                                    message,
                                    source_path: Some(target_file.to_string()),
                                });
                                continue;
                            }
                        }
                    };
                    match load_panel_from_scene_file(app_root, path.as_str(), panel_id) {
                        Ok(panel) => {
                            upsert_panel(panels, panel);
                            continue;
                        }
                        Err(error) => {
                            diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                code: "panel_ref_not_resolved".to_string(),
                                message: error.to_string(),
                                source_path: Some(target_file.to_string()),
                            });
                            continue;
                        }
                    }
                }
            }
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_frame_panel_slot".to_string(),
                message: "frame.panels entries must be panel(...) or panel_ref(...)".to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
    }
}

fn decode_panel_slot(value: &Value) -> Option<PanelDecl> {
    if value.get("kind").and_then(Value::as_str) == Some("panel") {
        return serde_json::from_value::<PanelDecl>(value.clone()).ok();
    }
    None
}

fn upsert_panel(panels: &mut Vec<PanelDecl>, panel: PanelDecl) {
    if let Some(existing) = panels.iter_mut().find(|item| item.id == panel.id) {
        *existing = panel;
        return;
    }
    panels.push(panel);
}
