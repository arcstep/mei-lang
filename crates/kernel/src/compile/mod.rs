use std::{collections::BTreeMap, path::Path};

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::{
    eval::evaluate_mei_file,
    model::{CompiledApp, Diagnostic, Severity},
    workspace::{load_component_assets, source_tree},
};

mod analysis;
mod app_decl;
mod decls;
mod entry_payload;
mod load_external;
mod loaders;
mod materialize;
mod mutations;
mod resources;
mod scene;
mod scene_binding;
mod ui_data_policy;

use app_decl::decode_app_decl;
use entry_payload::{compile_scene_payload_for_target, CompiledScenePayload};
use scene::{find_scene_route, resolve_scene_routes};

#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    pub scene: Option<String>,
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
    let mut route_registry =
        resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);

    let asset_map = load_component_assets(source_root)?;
    let mut official_results: BTreeMap<String, CompiledScenePayload> = BTreeMap::new();
    for route in &route_registry.routes {
        let result = compile_scene_payload_for_target(
            app_root,
            &app_decls,
            &asset_map,
            route.target_file.as_str(),
            Some(route),
        );
        official_results.insert(route.scene_id.clone(), result);
    }

    let active_route_meta = if let Some(requested) = options.scene.as_deref() {
        let selected = find_scene_route(&route_registry.routes, requested).cloned();
        if selected.is_none() {
            diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "unknown_scene".to_string(),
                message: format!("scene `{requested}` not found, fallback to default scene"),
                source_path: Some(app_main.to_string_lossy().to_string()),
            });
            route_registry
                .default_scene_id
                .as_deref()
                .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
                .cloned()
                .or_else(|| route_registry.routes.first().cloned())
        } else {
            selected
        }
    } else {
        route_registry
            .default_scene_id
            .as_deref()
            .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
            .cloned()
            .or_else(|| route_registry.routes.first().cloned())
    };

    let selected_target = options
        .preview_target
        .as_deref()
        .filter(|_| options.scene.is_none())
        .map(|value| value.to_string());

    let (active_scene, active_target_file, mut active_payload) = if let Some(target_file) =
        selected_target
    {
        if let Some(scene_route) = route_registry
            .routes
            .iter()
            .find(|route| route.target_file == target_file)
            .cloned()
        {
            let payload = official_results
                .get(&scene_route.scene_id)
                .cloned()
                .unwrap_or_else(|| {
                    compile_scene_payload_for_target(
                        app_root,
                        &app_decls,
                        &asset_map,
                        target_file.as_str(),
                        Some(&scene_route),
                    )
                });
            (Some(scene_route.scene_id), target_file, payload)
        } else {
            let payload = compile_scene_payload_for_target(
                app_root,
                &app_decls,
                &asset_map,
                target_file.as_str(),
                None,
            );
            if target_file == "main.mei" && payload.scene_contract.is_none() {
                let fallback_route = active_route_meta.clone().or_else(|| {
                    route_registry
                        .default_scene_id
                        .as_deref()
                        .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
                        .cloned()
                });
                if let Some(route_meta) = fallback_route {
                    let fallback_payload = official_results
                        .get(&route_meta.scene_id)
                        .cloned()
                        .unwrap_or_else(|| {
                            compile_scene_payload_for_target(
                                app_root,
                                &app_decls,
                                &asset_map,
                                route_meta.target_file.as_str(),
                                Some(&route_meta),
                            )
                        });
                    (Some(route_meta.scene_id), target_file, fallback_payload)
                } else {
                    (None, target_file, payload)
                }
            } else {
                (None, target_file, payload)
            }
        }
    } else if let Some(route_meta) = active_route_meta {
        let payload = official_results
            .get(&route_meta.scene_id)
            .cloned()
            .unwrap_or_else(|| {
                compile_scene_payload_for_target(
                    app_root,
                    &app_decls,
                    &asset_map,
                    route_meta.target_file.as_str(),
                    Some(&route_meta),
                )
            });
        (
            Some(route_meta.scene_id),
            route_meta.target_file,
            payload,
        )
    } else {
        (
            None,
            "main.mei".to_string(),
            compile_scene_payload_for_target(app_root, &app_decls, &asset_map, "main.mei", None),
        )
    };

    diagnostics.append(&mut active_payload.diagnostics);

    if let Some(active_id) = active_scene.as_deref() {
        let default_key = route_registry
            .default_scene_id
            .as_deref()
            .unwrap_or(active_id);
        for route in &mut route_registry.routes {
            route.is_default = route.scene_id == default_key;
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
        scene_routes: route_registry.routes,
        active_scene,
        active_target_file,
        file_tree: source_tree(app_root)?,
        scene_contract: active_payload.scene_contract,
        resources: active_payload.resources,
        component_assets: active_payload.component_assets,
        diagnostics,
    })
}

pub fn evaluate_runtime_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, crate::model::DatasetView>,
    metric_ids: Option<&[String]>,
) -> Result<BTreeMap<String, crate::model::MetricContract>> {
    materialize::evaluate_runtime_metric_defs(metric_defs, base_rows, datasets, metric_ids)
}

#[cfg(test)]
mod tests;
