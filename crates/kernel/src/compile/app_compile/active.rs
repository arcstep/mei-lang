use std::{collections::BTreeMap, path::Path, time::Instant};

use serde_json::Value;

use crate::model::{CompiledSceneRoute, ComponentAsset, Diagnostic};
use crate::typed_refs::SceneRegistry;

use super::super::dependency_graph::DependencyGraph;
use super::super::discover_routes::{manage_preview_precompile_routes, CompileOptions};
use super::super::entry_payload::CompiledScenePayload;
use super::super::route_compile::{elapsed_ms, precompile_route_payloads, RoutePrecompileStats};
use super::super::scene::find_scene_route;
use super::super::scene_payload_cache::compile_scene_payload_for_target;

pub(super) struct ActiveCompileResult {
    pub official_results: BTreeMap<String, CompiledScenePayload>,
    pub precompile_routes: Vec<CompiledSceneRoute>,
    pub route_precompile_stats: RoutePrecompileStats,
    pub official_results_all_routes_ms: u64,
    pub active_scene: Option<String>,
    pub active_target_file: String,
    pub active_payload: CompiledScenePayload,
    pub active_payload_pick_or_compile_ms: u64,
    /// Board / scene-first targets compiled while hydrating popup links on the active scene.
    pub hydrated_link_targets: BTreeMap<String, (String, CompiledScenePayload)>,
    pub preview_scope_diagnostics: Vec<Diagnostic>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn precompile_and_pick_active(
    app_root: &Path,
    source_root: &Path,
    app_entry_main: &str,
    app_decls: &Value,
    asset_map: &BTreeMap<String, ComponentAsset>,
    scene_registry: &SceneRegistry,
    dependency_graph: &DependencyGraph,
    route_registry: &mut super::super::scene::SceneRouteRegistry,
    active_route_meta: Option<CompiledSceneRoute>,
    preview_only: bool,
    preview_affected_targets: Option<std::collections::BTreeSet<String>>,
    options: &CompileOptions,
) -> ActiveCompileResult {
    let mut official_results: BTreeMap<String, CompiledScenePayload> = BTreeMap::new();
    let mut precompile_routes = Vec::<CompiledSceneRoute>::new();
    let mut preview_scope_diagnostics = Vec::<Diagnostic>::new();
    if preview_only {
        match manage_preview_precompile_routes(
            options,
            &route_registry.routes,
            preview_affected_targets.as_ref(),
        ) {
            Ok(routes) => precompile_routes = routes,
            Err(diag) => preview_scope_diagnostics.push(diag),
        }
    } else {
        let mut route_by_target = BTreeMap::<String, CompiledSceneRoute>::new();
        let explicit_scene_scope = options
            .scene
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
            || options
                .preview_target
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some();
        if let Some(route) = active_route_meta.as_ref() {
            route_by_target.insert(route.target_file.clone(), route.clone());
        }
        if !explicit_scene_scope {
            if let Some(default_route) = route_registry
                .default_scene_id
                .as_deref()
                .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
            {
                route_by_target.insert(default_route.target_file.clone(), default_route.clone());
            }
        }
        if let Some(preview_route) = options
            .preview_target
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .and_then(|target| {
                route_registry
                    .routes
                    .iter()
                    .find(|route| route.target_file == target)
            })
        {
            route_by_target.insert(preview_route.target_file.clone(), preview_route.clone());
        }
        precompile_routes = route_by_target.into_values().collect();
    }
    let official_results_started = Instant::now();
    let route_precompile_stats = precompile_route_payloads(
        app_root,
        source_root,
        app_decls,
        asset_map,
        scene_registry,
        dependency_graph,
        &precompile_routes,
        &mut official_results,
    );
    let official_results_all_routes_ms = elapsed_ms(official_results_started);

    let selected_target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(|value| value.to_string());
    let selected_scene_hint = options
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty());
    let active_payload_pick_started = Instant::now();
    let (active_scene, active_target_file, active_payload) = if let Some(target_file) =
        selected_target
    {
        if let Some(scene_route) = route_registry
            .routes
            .iter()
            .find(|route| {
                route.target_file == target_file
                    && selected_scene_hint
                        .map(|scene_id| route.scene_id == scene_id)
                        .unwrap_or(true)
            })
            .or_else(|| {
                active_route_meta
                    .as_ref()
                    .filter(|route| route.target_file == target_file)
            })
            .or_else(|| {
                route_registry
                    .routes
                    .iter()
                    .find(|route| route.target_file == target_file)
            })
            .cloned()
        {
            let payload = official_results
                .get(&scene_route.scene_id)
                .cloned()
                .unwrap_or_else(|| {
                    let dependency_fingerprint = dependency_graph
                        .dependency_fingerprint_for_target(
                            app_root,
                            app_decls,
                            target_file.as_str(),
                        );
                    compile_scene_payload_for_target(
                        app_root,
                        source_root,
                        app_decls,
                        asset_map,
                        target_file.as_str(),
                        Some(&scene_route),
                        scene_registry,
                        dependency_fingerprint.as_deref(),
                    )
                });
            (Some(scene_route.scene_id), target_file, payload)
        } else {
            let dependency_fingerprint = dependency_graph.dependency_fingerprint_for_target(
                app_root,
                app_decls,
                target_file.as_str(),
            );
            let payload = compile_scene_payload_for_target(
                app_root,
                source_root,
                app_decls,
                asset_map,
                target_file.as_str(),
                None,
                scene_registry,
                dependency_fingerprint.as_deref(),
            );
            if target_file == app_entry_main && payload.scene_contract.is_none() {
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
                            let dependency_fingerprint = dependency_graph
                                .dependency_fingerprint_for_target(
                                    app_root,
                                    app_decls,
                                    route_meta.target_file.as_str(),
                                );
                            compile_scene_payload_for_target(
                                app_root,
                                source_root,
                                app_decls,
                                asset_map,
                                route_meta.target_file.as_str(),
                                Some(&route_meta),
                                scene_registry,
                                dependency_fingerprint.as_deref(),
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
                let dependency_fingerprint = dependency_graph.dependency_fingerprint_for_target(
                    app_root,
                    app_decls,
                    route_meta.target_file.as_str(),
                );
                compile_scene_payload_for_target(
                    app_root,
                    source_root,
                    app_decls,
                    asset_map,
                    route_meta.target_file.as_str(),
                    Some(&route_meta),
                    scene_registry,
                    dependency_fingerprint.as_deref(),
                )
            });
        (Some(route_meta.scene_id), route_meta.target_file, payload)
    } else {
        let dependency_fingerprint =
            dependency_graph.dependency_fingerprint_for_target(app_root, app_decls, app_entry_main);
        (
            None,
            app_entry_main.to_string(),
            compile_scene_payload_for_target(
                app_root,
                source_root,
                app_decls,
                asset_map,
                app_entry_main,
                None,
                scene_registry,
                dependency_fingerprint.as_deref(),
            ),
        )
    };
    let active_payload_pick_or_compile_ms = elapsed_ms(active_payload_pick_started);

    if let Some(active_id) = active_scene.as_deref() {
        let default_key = route_registry
            .default_scene_id
            .as_deref()
            .unwrap_or(active_id);
        for route in &mut route_registry.routes {
            route.is_default = route.scene_id == default_key;
        }
    }

    ActiveCompileResult {
        official_results,
        precompile_routes,
        route_precompile_stats,
        official_results_all_routes_ms,
        active_scene,
        active_target_file,
        active_payload,
        active_payload_pick_or_compile_ms,
        hydrated_link_targets: BTreeMap::new(),
        preview_scope_diagnostics,
    }
}
