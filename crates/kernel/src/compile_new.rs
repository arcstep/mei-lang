use std::{
    collections::BTreeSet,
    path::Path,
};

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::{
    eval::evaluate_mei_file,
    model::{
        AppDecl, CompiledApp, ComponentAsset, Diagnostic, FlowDecl, FrameDecl, PanelDecl,
        SceneContract, SceneDecl, Severity, ThemeDecl, UiNodeDecl,
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
use scene::resolve_scene_source;

pub fn compile_app(source_root: &Path, app_id: &str) -> Result<CompiledApp> {
    let app_root = source_root.join(app_id);
    compile_app_from_root(source_root, &app_root)
}

pub fn compile_app_from_root(source_root: &Path, app_root: &Path) -> Result<CompiledApp> {
    let app_main = app_root.join("main.mei");
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let (entry_target, entry_decls) =
        resolve_scene_source(app_root, &app_main, &app_decl, &app_decls, &mut diagnostics)?;

    let mut frame: Option<FrameDecl> = None;
    let mut scene: Option<SceneDecl> = None;
    let mut themes: Vec<ThemeDecl> = Vec::new();
    let mut world: Option<crate::model::WorldDecl> = None;
    let mut flow: Option<FlowDecl> = None;
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
                        source_path: Some(entry_target.clone()),
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
                        source_path: Some(entry_target.clone()),
                    }),
                }
                continue;
            }
            let Some(kind) = value.get("kind").and_then(Value::as_str) else {
                continue;
            };
            match kind {
                "frame" => frame = Some(serde_json::from_value(value.clone())?),
                "scene" => scene = Some(serde_json::from_value(value.clone())?),
                "world" => world = Some(serde_json::from_value(value.clone())?),
                "flow" => flow = Some(serde_json::from_value(value.clone())?),
                "theme" => themes.push(serde_json::from_value(value.clone())?),
                "panel" => panels.push(serde_json::from_value(value.clone())?),
                "dataset_view" => match serde_json::from_value::<DatasetViewDecl>(value.clone()) {
                    Ok(decl) => dataset_views.push(decl),
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_dataset_view_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(entry_target.clone()),
                    }),
                },
                "app" | "app_scene_ref" => {}
                _ => diagnostics.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "unknown_decl".to_string(),
                    message: format!("unknown declaration kind `{kind}`"),
                    source_path: Some(entry_target.clone()),
                }),
            }
        }
    }

    let asset_map = load_component_assets(source_root)?;
    let mut asset_keys = BTreeSet::new();
    for panel in &panels {
        collect_asset_keys_from_nodes(&panel.blocks, &mut asset_keys);
    }
    let component_assets = asset_keys
        .into_iter()
        .filter_map(|key| asset_map.get(&key).cloned())
        .collect::<Vec<ComponentAsset>>();

    if scene.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "missing_scene".to_string(),
            message: "entry file must declare scene(...) for scene-first authoring".to_string(),
            source_path: Some(entry_target.clone()),
        });
    }

    if frame.is_none() {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "missing_frame".to_string(),
            message: "scene entry should declare frame(...) to define UI layout".to_string(),
            source_path: Some(entry_target.clone()),
        });
    }

    let title = app_decl
        .title
        .clone()
        .unwrap_or_else(|| app_decl.id.clone());

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

    let scene_contract = scene.map(|scene_decl| SceneContract {
        scene: scene_decl,
        themes,
        world,
        flow,
        frame: frame.clone(),
        panels,
    });

    Ok(CompiledApp {
        app_id: app_decl.id.clone(),
        title,
        app_root: app_root.to_string_lossy().to_string(),
        entry_target,
        file_tree: source_tree(app_root)?,
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
