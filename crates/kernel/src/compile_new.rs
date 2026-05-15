use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::{
    eval::evaluate_mei_file,
    model::{
        AppDecl, CompiledApp, CompiledEntryMeta, ComponentAsset, Diagnostic, EntityDecl, FlowDecl,
        FrameDecl, PanelDecl, ResourceDecl, SceneContract, SceneDecl, Severity, ThemeDecl,
        UiNodeDecl, WorldGridDecl,
    },
    workspace::{load_component_assets, source_tree},
};

#[path = "compile/analysis/mod.rs"]
mod analysis;
#[path = "compile/decls.rs"]
mod decls;
#[path = "compile/materialize.rs"]
mod materialize;
#[path = "compile/resources.rs"]
mod resources;
#[path = "compile/scene.rs"]
mod scene;

use decls::{
    DatasetViewDecl, FrameFileRefDecl, FrameSetLayoutDecl, LegacyDatasetDecl, LegacyMetricPackDecl,
    WorldAddEntityDecl, WorldAddResourceDecl, WorldFileRefDecl, WorldSetTopologyDecl,
};
use materialize::{
    materialize_dataset_views, materialize_legacy_datasets, materialize_metric_packs,
};
use resources::load_resources;
use scene::{find_scene_entry, resolve_scene_entries, scene_name_from_path};

#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    pub entry: Option<String>,
    pub preview_target: Option<String>,
}

pub fn compile_app(source_root: &Path, app_id: &str) -> Result<CompiledApp> {
    compile_app_with_options(source_root, app_id, CompileOptions::default())
}

pub fn compile_app_with_options(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
) -> Result<CompiledApp> {
    let app_root = source_root.join(app_id);
    compile_app_from_root_with_options(source_root, &app_root, options)
}

pub fn compile_app_from_root(source_root: &Path, app_root: &Path) -> Result<CompiledApp> {
    compile_app_from_root_with_options(source_root, app_root, CompileOptions::default())
}

pub fn compile_app_from_root_with_options(
    source_root: &Path,
    app_root: &Path,
    options: CompileOptions,
) -> Result<CompiledApp> {
    let app_main = app_root.join("main.mei");
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let mut entry_registry =
        resolve_scene_entries(&app_main, &app_decl, &app_decls, &mut diagnostics);

    let asset_map = load_component_assets(source_root)?;
    let mut official_results: BTreeMap<String, CompiledEntryPayload> = BTreeMap::new();
    for entry in &entry_registry.entries {
        let result = compile_entry_payload_for_target(
            app_root,
            &app_decls,
            &asset_map,
            entry.target_file.as_str(),
            Some(entry),
        );
        official_results.insert(entry.entry_id.clone(), result);
    }

    let active_entry_meta = if let Some(requested_entry) = options.entry.as_deref() {
        let selected = find_scene_entry(&entry_registry.entries, requested_entry).cloned();
        if selected.is_none() {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "unknown_entry".to_string(),
                message: format!("entry `{requested_entry}` not found, fallback to default entry"),
                source_path: Some(app_main.to_string_lossy().to_string()),
            });
        }
        selected
    } else {
        entry_registry
            .default_entry_id
            .as_deref()
            .and_then(|entry_id| find_scene_entry(&entry_registry.entries, entry_id))
            .cloned()
            .or_else(|| entry_registry.entries.first().cloned())
    };

    let selected_target = options
        .preview_target
        .as_deref()
        .filter(|_| options.entry.is_none())
        .map(|value| value.to_string());

    let (active_entry, entry_target, mut active_payload) = if let Some(target_file) =
        selected_target
    {
        if let Some(scene_entry) = entry_registry
            .entries
            .iter()
            .find(|entry| entry.target_file == target_file)
            .cloned()
        {
            let payload = official_results
                .get(&scene_entry.entry_id)
                .cloned()
                .unwrap_or_else(|| {
                    compile_entry_payload_for_target(
                        app_root,
                        &app_decls,
                        &asset_map,
                        target_file.as_str(),
                        Some(&scene_entry),
                    )
                });
            (Some(scene_entry.entry_id), target_file, payload)
        } else {
            let payload = compile_entry_payload_for_target(
                app_root,
                &app_decls,
                &asset_map,
                target_file.as_str(),
                None,
            );
            if target_file == "main.mei" && payload.scene_contract.is_none() {
                let fallback_entry = active_entry_meta.clone().or_else(|| {
                    entry_registry
                        .default_entry_id
                        .as_deref()
                        .and_then(|entry_id| find_scene_entry(&entry_registry.entries, entry_id))
                        .cloned()
                });
                if let Some(entry_meta) = fallback_entry {
                    let fallback_payload = official_results
                        .get(&entry_meta.entry_id)
                        .cloned()
                        .unwrap_or_else(|| {
                            compile_entry_payload_for_target(
                                app_root,
                                &app_decls,
                                &asset_map,
                                entry_meta.target_file.as_str(),
                                Some(&entry_meta),
                            )
                        });
                    (Some(entry_meta.entry_id), target_file, fallback_payload)
                } else {
                    (None, target_file, payload)
                }
            } else {
                (None, target_file, payload)
            }
        }
    } else if let Some(entry_meta) = active_entry_meta {
        let payload = official_results
            .get(&entry_meta.entry_id)
            .cloned()
            .unwrap_or_else(|| {
                compile_entry_payload_for_target(
                    app_root,
                    &app_decls,
                    &asset_map,
                    entry_meta.target_file.as_str(),
                    Some(&entry_meta),
                )
            });
        (Some(entry_meta.entry_id), entry_meta.target_file, payload)
    } else {
        (
            None,
            "main.mei".to_string(),
            compile_entry_payload_for_target(app_root, &app_decls, &asset_map, "main.mei", None),
        )
    };

    diagnostics.append(&mut active_payload.diagnostics);

    if let Some(active_id) = active_entry.as_deref() {
        for entry in &mut entry_registry.entries {
            entry.is_default = entry.entry_id
                == entry_registry
                    .default_entry_id
                    .as_deref()
                    .unwrap_or(active_id);
        }
    }
    let title = app_decl
        .title
        .clone()
        .unwrap_or_else(|| app_decl.id.clone());

    Ok(CompiledApp {
        app_id: app_decl.id.clone(),
        title,
        app_root: app_root.to_string_lossy().to_string(),
        entries: entry_registry.entries,
        active_entry,
        entry_target,
        file_tree: source_tree(app_root)?,
        scene_contract: active_payload.scene_contract,
        resources: active_payload.resources,
        component_assets: active_payload.component_assets,
        diagnostics,
    })
}

#[derive(Debug, Clone, Default)]
struct CompiledEntryPayload {
    scene_contract: Option<SceneContract>,
    resources: Vec<crate::model::LoadedResource>,
    component_assets: Vec<ComponentAsset>,
    diagnostics: Vec<Diagnostic>,
}

fn compile_entry_payload_for_target(
    app_root: &Path,
    app_decls: &Value,
    asset_map: &std::collections::BTreeMap<String, ComponentAsset>,
    target_file: &str,
    entry_meta: Option<&CompiledEntryMeta>,
) -> CompiledEntryPayload {
    match load_entry_decls(app_root, app_decls, target_file) {
        Ok(entry_decls) => {
            match compile_entry_payload(app_root, asset_map, target_file, &entry_decls, entry_meta)
            {
                Ok(payload) => payload,
                Err(error) => CompiledEntryPayload {
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Error,
                        code: "compile_entry_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(target_file.to_string()),
                    }],
                    ..CompiledEntryPayload::default()
                },
            }
        }
        Err(error) => CompiledEntryPayload {
            diagnostics: vec![Diagnostic {
                severity: Severity::Error,
                code: "load_entry_failed".to_string(),
                message: error.to_string(),
                source_path: Some(target_file.to_string()),
            }],
            ..CompiledEntryPayload::default()
        },
    }
}

fn load_entry_decls(app_root: &Path, app_decls: &Value, target_file: &str) -> Result<Value> {
    if target_file == "main.mei" {
        Ok(app_decls.clone())
    } else {
        let entry_path = app_root.join(target_file);
        evaluate_mei_file(&entry_path)
    }
}

fn compile_entry_payload(
    app_root: &Path,
    asset_map: &std::collections::BTreeMap<String, ComponentAsset>,
    target_file: &str,
    entry_decls: &Value,
    entry_meta: Option<&CompiledEntryMeta>,
) -> Result<CompiledEntryPayload> {
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
    let mut dataset_views: Vec<DatasetViewDecl> = Vec::new();
    let mut legacy_datasets: Vec<LegacyDatasetDecl> = Vec::new();
    let mut legacy_metric_packs: Vec<LegacyMetricPackDecl> = Vec::new();

    if let Some(values) = entry_decls.as_array() {
        for value in values {
            if value.get("dataset").is_some() && value.get("schema_version").is_some() {
                match serde_json::from_value::<LegacyDatasetDecl>(value.clone()) {
                    Ok(decl) => legacy_datasets.push(decl),
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_legacy_dataset_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(target_file.to_string()),
                    }),
                }
                continue;
            }
            if value.get("metric_pack").is_some() && value.get("schema_version").is_some() {
                match serde_json::from_value::<LegacyMetricPackDecl>(value.clone()) {
                    Ok(decl) => legacy_metric_packs.push(decl),
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_metric_pack_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(target_file.to_string()),
                    }),
                }
                continue;
            }
            let Some(kind) = value.get("kind").and_then(Value::as_str) else {
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
                        frames.entry(id.to_string()).or_insert(frame_decl);
                    } else {
                        if frame_default.is_none() {
                            frame_default = Some(frame_decl);
                        }
                    }
                }
                "scene" => {
                    scene_decl_count += 1;
                    let scene_decl = decode_scene_decl(value, target_file)?;
                    scenes.entry(scene_decl.id.clone()).or_insert(scene_decl);
                }
                "world" => {
                    world_decl_count += 1;
                    let world_decl =
                        serde_json::from_value::<crate::model::WorldDecl>(value.clone())?;
                    if let Some(id) = world_decl
                        .id
                        .as_deref()
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                    {
                        worlds.entry(id.to_string()).or_insert(world_decl);
                    } else {
                        if world_default.is_none() {
                            world_default = Some(world_decl);
                        }
                    }
                }
                "world_add_resource" => {
                    let decl = serde_json::from_value::<WorldAddResourceDecl>(value.clone())?;
                    if decl.kind == "world_add_resource" {
                        pending_world_resources.push(decl.resource);
                    }
                }
                "world_add_entity" => {
                    let decl = serde_json::from_value::<WorldAddEntityDecl>(value.clone())?;
                    if decl.kind == "world_add_entity" {
                        pending_world_entities.push(decl.entity);
                    }
                }
                "world_set_topology" => {
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
                            pending_frame_layout =
                                Some(serde_json::from_value::<crate::model::LayoutDecl>(
                                    decl.layout,
                                )?);
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
                "dataset_view" => match serde_json::from_value::<DatasetViewDecl>(value.clone()) {
                    Ok(decl) => dataset_views.push(decl),
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_dataset_view_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(target_file.to_string()),
                    }),
                },
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

    let selected_scene = entry_meta
        .and_then(|meta| scenes.get(meta.scene_id.as_str()).cloned())
        .or_else(|| {
            if scenes.len() == 1 {
                scenes.values().next().cloned()
            } else {
                None
            }
        });
    let requires_scene_contract = entry_meta.is_some() || target_file != "main.mei";
    if requires_scene_contract && selected_scene.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_scene".to_string(),
            message: "entry file must declare scene(...) for scene-first authoring".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    let frame = if let Some(frame_id) = entry_meta
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
            .map(|value| parse_scene_binding(value, "frame_file_ref", "frame"));
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
                            code: "load_frame_file_ref_failed".to_string(),
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
            message: "scene entry requires a frame(...) declaration or frame_file_ref(...)".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    let world = if let Some(scene_decl) = selected_scene.as_ref() {
        let binding = scene_decl
            .world
            .as_ref()
            .map(|value| parse_scene_binding(value, "world_file_ref", "world"));
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
                            code: "load_world_file_ref_failed".to_string(),
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
            message: "scene entry requires a world(...) declaration or world_file_ref(...)".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    let flow = selected_scene
        .as_ref()
        .and_then(|scene| scene.flow.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .and_then(|id| flows.get(id).cloned())
        .or_else(|| {
            flow_default.clone().or_else(|| {
                (flows.len() == 1)
                    .then(|| flows.values().next().cloned())
                    .flatten()
            })
        });

    let mut resources = match world.as_ref() {
        Some(world_decl) => load_resources(app_root, &world_decl.resources)?,
        None => Vec::new(),
    };
    if !legacy_datasets.is_empty() {
        let derived = materialize_legacy_datasets(app_root, &resources, &legacy_datasets)?;
        for resource in derived {
            if let Some(index) = resources.iter().position(|item| item.id == resource.id) {
                resources[index] = resource;
            } else {
                resources.push(resource);
            }
        }
    }
    if !dataset_views.is_empty() {
        let derived = materialize_dataset_views(&resources, &dataset_views)?;
        for resource in derived {
            if let Some(index) = resources.iter().position(|item| item.id == resource.id) {
                resources[index] = resource;
            } else {
                resources.push(resource);
            }
        }
    }
    if !legacy_metric_packs.is_empty() {
        let derived = materialize_metric_packs(&resources, &legacy_metric_packs)?;
        for resource in derived {
            if let Some(index) = resources.iter().position(|item| item.id == resource.id) {
                resources[index] = resource;
            } else {
                resources.push(resource);
            }
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

    Ok(CompiledEntryPayload {
        scene_contract,
        resources,
        component_assets,
        diagnostics,
    })
}

fn decode_scene_decl(value: &Value, target_file: &str) -> Result<SceneDecl> {
    let mut raw = value.clone();
    let missing_scene_id = raw
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .map(|id| id.is_empty())
        .unwrap_or(true);
    if missing_scene_id {
        raw["id"] = Value::String(scene_name_from_path(target_file));
    }
    serde_json::from_value::<SceneDecl>(raw).map_err(Into::into)
}

#[derive(Debug, Clone)]
enum SceneBinding {
    Absent,
    LocalId(String),
    FileRef { path: String, id: Option<String> },
}

fn parse_scene_binding(value: &Value, expected_kind: &str, label: &str) -> Result<SceneBinding> {
    if value.is_null() {
        return Ok(SceneBinding::Absent);
    }
    if let Some(id) = value.as_str().map(str::trim) {
        if id.is_empty() {
            return Ok(SceneBinding::Absent);
        }
        return Ok(SceneBinding::LocalId(id.to_string()));
    }
    if expected_kind == "world_file_ref" {
        let world_ref = serde_json::from_value::<WorldFileRefDecl>(value.clone())
            .map_err(|error| anyhow!("invalid {label} binding: {error}"))?;
        if world_ref.kind != expected_kind {
            return Err(anyhow!(
                "invalid {label} binding kind `{}`, expected `{expected_kind}`",
                world_ref.kind
            ));
        }
        if world_ref.path.trim().is_empty() {
            return Err(anyhow!("{label}_file_ref path must not be empty"));
        }
        return Ok(SceneBinding::FileRef {
            path: world_ref.path,
            id: world_ref.id,
        });
    }
    if expected_kind == "frame_file_ref" {
        let frame_ref = serde_json::from_value::<FrameFileRefDecl>(value.clone())
            .map_err(|error| anyhow!("invalid {label} binding: {error}"))?;
        if frame_ref.kind != expected_kind {
            return Err(anyhow!(
                "invalid {label} binding kind `{}`, expected `{expected_kind}`",
                frame_ref.kind
            ));
        }
        if frame_ref.path.trim().is_empty() {
            return Err(anyhow!("{label}_file_ref path must not be empty"));
        }
        return Ok(SceneBinding::FileRef {
            path: frame_ref.path,
            id: frame_ref.id,
        });
    }
    Err(anyhow!(
        "unsupported {label} binding; expected local id string or {expected_kind}(...)"
    ))
}

fn pick_only_frame(
    frames: &BTreeMap<String, FrameDecl>,
    frame_default: Option<FrameDecl>,
) -> Option<FrameDecl> {
    if frames.len() + usize::from(frame_default.is_some()) != 1 {
        return None;
    }
    frame_default.or_else(|| frames.values().next().cloned())
}

fn apply_frame_mutations(
    frames: &mut BTreeMap<String, FrameDecl>,
    frame_default: &mut Option<FrameDecl>,
    layout: Option<crate::model::LayoutDecl>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    frame_decl_count: usize,
) {
    let Some(layout) = layout else {
        return;
    };
    match frame_decl_count {
        0 => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_frame_declaration".to_string(),
                message: "frame.set_layout(...) requires a frame(...) declaration in the same file".to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
        1 => {
            if let Some(frame_decl) = frame_default.as_mut() {
                frame_decl.layout = Some(layout);
                return;
            }
            if let Some((_id, frame_decl)) = frames.iter_mut().next() {
                frame_decl.layout = Some(layout);
            }
        }
        _ => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "ambiguous_frame_mutation".to_string(),
                message: "frame.set_layout(...) requires exactly one frame(...) declaration in the file".to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
    }
}

fn pick_only_world(
    worlds: &BTreeMap<String, crate::model::WorldDecl>,
    world_default: Option<crate::model::WorldDecl>,
) -> Option<crate::model::WorldDecl> {
    if worlds.len() + usize::from(world_default.is_some()) != 1 {
        return None;
    }
    world_default.or_else(|| worlds.values().next().cloned())
}

fn apply_world_mutations(
    worlds: &mut BTreeMap<String, crate::model::WorldDecl>,
    world_default: &mut Option<crate::model::WorldDecl>,
    resources: &[ResourceDecl],
    entities: &[EntityDecl],
    topology: Option<WorldGridDecl>,
    diagnostics: &mut Vec<Diagnostic>,
    target_file: &str,
    world_decl_count: usize,
) {
    let has_mutations = !resources.is_empty() || !entities.is_empty() || topology.is_some();
    if !has_mutations {
        return;
    }
    match world_decl_count {
        0 => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_world_declaration".to_string(),
                message: "world.add_* / world.set_topology(...) requires a world(...) declaration in the same file".to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
        1 => {
            if let Some(world_decl) = world_default.as_mut() {
                apply_world_mutations_to_decl(world_decl, resources, entities, topology);
                return;
            }
            if let Some((_id, world_decl)) = worlds.iter_mut().next() {
                apply_world_mutations_to_decl(world_decl, resources, entities, topology);
            }
        }
        _ => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "ambiguous_world_mutation".to_string(),
                message: "world.add_* / world.set_topology(...) requires exactly one world(...) declaration in the file".to_string(),
                source_path: Some(target_file.to_string()),
            });
        }
    }
}

fn apply_world_mutations_to_decl(
    world_decl: &mut crate::model::WorldDecl,
    resources: &[ResourceDecl],
    entities: &[EntityDecl],
    topology: Option<WorldGridDecl>,
) {
    world_decl.resources.extend(resources.iter().cloned());
    world_decl.entities.extend(entities.iter().cloned());
    if let Some(topology) = topology {
        world_decl.topology = Some(topology);
    }
}

fn load_world_from_file(
    app_root: &Path,
    relative_path: &str,
    world_id: Option<&str>,
) -> Result<crate::model::WorldDecl> {
    let source_path = app_root.join(relative_path);
    let decls = evaluate_mei_file(&source_path)?;
    let mut worlds = Vec::new();
    let mut pending_resources = Vec::new();
    let mut pending_entities = Vec::new();
    let mut pending_topology = None;
    let mut world_topology_set_count = 0usize;
    if let Some(values) = decls.as_array() {
        for value in values {
            match value.get("kind").and_then(Value::as_str) {
                Some("world") => {
                    worlds.push(serde_json::from_value::<crate::model::WorldDecl>(value.clone())?);
                }
                Some("world_add_resource") => {
                    let decl = serde_json::from_value::<WorldAddResourceDecl>(value.clone())?;
                    pending_resources.push(decl.resource);
                }
                Some("world_add_entity") => {
                    let decl = serde_json::from_value::<WorldAddEntityDecl>(value.clone())?;
                    pending_entities.push(decl.entity);
                }
                Some("world_set_topology") => {
                    let decl = serde_json::from_value::<WorldSetTopologyDecl>(value.clone())?;
                    world_topology_set_count += 1;
                    if pending_topology.is_none() {
                        pending_topology = Some(decl.topology);
                    }
                }
                _ => {}
            }
        }
    }
    if world_topology_set_count > 1 {
        return Err(anyhow!(
            "world_file_ref `{relative_path}` declared multiple world.set_topology(...) blocks"
        ));
    }
    if !pending_resources.is_empty() || !pending_entities.is_empty() || pending_topology.is_some() {
        match worlds.len() {
            0 => {
                return Err(anyhow!(
                    "world_file_ref `{relative_path}` used world.add_* / world.set_topology(...) without world(...)"
                ));
            }
            1 => {
                if let Some(world_decl) = worlds.first_mut() {
                    apply_world_mutations_to_decl(
                        world_decl,
                        &pending_resources,
                        &pending_entities,
                        pending_topology,
                    );
                }
            }
            count => {
                return Err(anyhow!(
                    "world_file_ref `{relative_path}` used world.add_* / world.set_topology(...) with {count} world(...) declarations"
                ));
            }
        }
    }
    if let Some(expected_id) = world_id {
        return worlds
            .into_iter()
            .find(|decl| decl.id.as_deref() == Some(expected_id))
            .ok_or_else(|| {
                anyhow!(
                    "world_file_ref `{relative_path}` did not contain world id `{expected_id}`"
                )
            });
    }
    match worlds.len() {
        0 => Err(anyhow!(
            "world_file_ref `{relative_path}` did not contain world(...) declarations"
        )),
        1 => worlds
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("world_file_ref `{relative_path}` did not contain world")),
        count => Err(anyhow!(
            "world_file_ref `{relative_path}` matched {count} world(...) declarations; provide id"
        )),
    }
}

fn load_frame_from_file(
    app_root: &Path,
    relative_path: &str,
    frame_id: Option<&str>,
) -> Result<FrameDecl> {
    let source_path = app_root.join(relative_path);
    let decls = evaluate_mei_file(&source_path)?;
    let mut frames = Vec::new();
    let mut pending_layout: Option<crate::model::LayoutDecl> = None;
    let mut frame_layout_set_count = 0usize;
    if let Some(values) = decls.as_array() {
        for value in values {
            match value.get("kind").and_then(Value::as_str) {
                Some("frame") => {
                    frames.push(serde_json::from_value::<FrameDecl>(value.clone())?);
                }
                Some("frame_set_layout") => {
                    let decl = serde_json::from_value::<FrameSetLayoutDecl>(value.clone())?;
                    frame_layout_set_count += 1;
                    if pending_layout.is_none() {
                        pending_layout = Some(serde_json::from_value::<crate::model::LayoutDecl>(
                            decl.layout,
                        )?);
                    }
                }
                _ => {}
            }
        }
    }
    if frame_layout_set_count > 1 {
        return Err(anyhow!(
            "frame_file_ref `{relative_path}` declared multiple frame.set_layout(...) blocks"
        ));
    }
    if let Some(layout) = pending_layout {
        match frames.len() {
            0 => {
                return Err(anyhow!(
                    "frame_file_ref `{relative_path}` used frame.set_layout(...) without frame(...)"
                ));
            }
            1 => {
                if let Some(frame_decl) = frames.first_mut() {
                    frame_decl.layout = Some(layout);
                }
            }
            count => {
                return Err(anyhow!(
                    "frame_file_ref `{relative_path}` used frame.set_layout(...) with {count} frame(...) declarations"
                ));
            }
        }
    }
    if let Some(expected_id) = frame_id {
        return frames
            .into_iter()
            .find(|decl| decl.id.as_deref() == Some(expected_id))
            .ok_or_else(|| {
                anyhow!(
                    "frame_file_ref `{relative_path}` did not contain frame id `{expected_id}`"
                )
            });
    }
    match frames.len() {
        0 => Err(anyhow!(
            "frame_file_ref `{relative_path}` did not contain frame(...) declarations"
        )),
        1 => frames
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("frame_file_ref `{relative_path}` did not contain frame")),
        count => Err(anyhow!(
            "frame_file_ref `{relative_path}` matched {count} frame(...) declarations; provide id"
        )),
    }
}

fn collect_asset_keys_from_nodes(nodes: &[UiNodeDecl], asset_keys: &mut BTreeSet<String>) {
    for node in nodes {
        match node {
            UiNodeDecl::Panel(panel) => collect_asset_keys_from_nodes(&panel.blocks, asset_keys),
            UiNodeDecl::Block(block) => {
                asset_keys.insert(block.use_key.clone());
            }
        }
    }
}

fn decode_app_decl(path: &Path, raw: &Value) -> (Option<AppDecl>, Vec<Diagnostic>) {
    let mut app_decl = None;
    let mut diagnostics = Vec::new();
    let mut app_decl_count = 0usize;
    if let Some(values) = raw.as_array() {
        for value in values {
            if value.get("kind").and_then(Value::as_str) == Some("app") {
                app_decl_count += 1;
                match serde_json::from_value::<AppDecl>(value.clone()) {
                    Ok(decl) => {
                        if app_decl.is_none() {
                            app_decl = Some(decl);
                        }
                    }
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_app_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(path.to_string_lossy().to_string()),
                    }),
                }
            }
        }
    }
    if app_decl_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_apps".to_string(),
            message: format!(
                "file `{}` declares {app_decl_count} app(...) blocks, expected exactly one",
                path.display()
            ),
            source_path: Some(path.to_string_lossy().to_string()),
        });
    }
    (app_decl, diagnostics)
}

#[cfg(test)]
#[path = "compile/tests.rs"]
mod tests;
