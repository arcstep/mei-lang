use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::{
    eval::evaluate_mei_file,
    model::{
        AppDecl, CompiledApp, CompiledEntryMeta, ComponentAsset, Diagnostic, FlowDecl, FrameDecl,
        PanelDecl, SceneContract, SceneDecl, Severity, ThemeDecl, UiNodeDecl,
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

use decls::{DatasetViewDecl, LegacyDatasetDecl, LegacyMetricPackDecl};
use materialize::{
    materialize_dataset_views, materialize_legacy_datasets, materialize_metric_packs,
};
use resources::load_resources;
use scene::{find_scene_entry, resolve_scene_entries};

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
    if entry_registry.entries.is_empty() {
        entry_registry.entries.push(CompiledEntryMeta {
            entry_id: "main".to_string(),
            scene_id: "main".to_string(),
            frame_id: None,
            target_file: "main.mei".to_string(),
            kind: "inline".to_string(),
            title: None,
            is_default: true,
        });
        entry_registry.default_entry_id = Some("main".to_string());
    }

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
    let mut frame_default: Option<FrameDecl> = None;
    let mut world_default: Option<crate::model::WorldDecl> = None;
    let mut flow_default: Option<FlowDecl> = None;
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
                    let frame_decl = serde_json::from_value::<FrameDecl>(value.clone())?;
                    if let Some(id) = frame_decl
                        .id
                        .as_deref()
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                    {
                        frames.insert(id.to_string(), frame_decl);
                    } else {
                        frame_default = Some(frame_decl);
                    }
                }
                "scene" => {
                    let scene_decl = serde_json::from_value::<SceneDecl>(value.clone())?;
                    scenes.insert(scene_decl.id.clone(), scene_decl);
                }
                "world" => {
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
                        world_default = Some(world_decl);
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
    let requires_declarative_binding = entry_meta
        .map(|meta| meta.kind == "declarative")
        .unwrap_or(false);
    if requires_declarative_binding && selected_scene.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_scene".to_string(),
            message: "entry file must declare scene(...) for scene-first authoring".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    let selected_frame_id = entry_meta
        .and_then(|meta| meta.frame_id.as_deref())
        .or_else(|| {
            selected_scene
                .as_ref()
                .and_then(|scene| scene.frame.as_deref())
        })
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| id.to_string());
    let frame = if let Some(frame_id) = selected_frame_id {
        let matched = frames.get(frame_id.as_str()).cloned();
        if matched.is_none() && requires_declarative_binding {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "missing_bound_frame".to_string(),
                message: format!("declared frame `{frame_id}` was not found"),
                source_path: Some(target_file.to_string()),
            });
        }
        matched
    } else {
        frame_default.clone().or_else(|| {
            (frames.len() == 1)
                .then(|| frames.values().next().cloned())
                .flatten()
        })
    };
    if requires_declarative_binding && frame.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "missing_frame".to_string(),
            message: "scene entry should declare frame(...) to define UI layout".to_string(),
            source_path: Some(target_file.to_string()),
        });
    }

    let world = selected_scene
        .as_ref()
        .and_then(|scene| scene.world.as_deref())
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .and_then(|id| worlds.get(id).cloned())
        .or_else(|| {
            world_default.clone().or_else(|| {
                (worlds.len() == 1)
                    .then(|| worlds.values().next().cloned())
                    .flatten()
            })
        });
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
    if let Some(values) = raw.as_array() {
        for value in values {
            if value.get("kind").and_then(Value::as_str) == Some("app") {
                match serde_json::from_value::<AppDecl>(value.clone()) {
                    Ok(decl) => app_decl = Some(decl),
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
    (app_decl, diagnostics)
}

#[cfg(test)]
#[path = "compile/tests.rs"]
mod tests;
