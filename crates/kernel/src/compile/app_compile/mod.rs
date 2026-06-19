mod active;
mod catalog;
mod finish;

#[path = "../app_compile_revision.rs"]
mod app_compile_revision;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::{
    eval::evaluate_mei_file,
    mei_config::{resolve_app_entry_main, resolve_app_main_path, resolve_app_root},
    model::{CompiledApp, CompiledSceneRoute, Diagnostic, SceneContract, Severity},
    typed_refs::SceneRegistry,
    workspace::load_component_assets,
};

use super::app_decl::decode_app_decl;
use super::catalog::dataset_catalog_index_cache_metrics_snapshot;
use super::decl_file_cache::decl_file_cache_metrics_snapshot;
use super::dependency_graph::{
    dependency_graph_cache_metrics_snapshot, file_content_hash_cache_metrics_snapshot,
    DependencyGraph,
};
use super::discover_routes::{
    inject_discovered_entry_scene_routes, is_dataset_manage_preview, is_manage_preview_only_compile,
    push_app_config_diagnostics, CompileOptions, CompileRevisionPlan,
};
use super::materialize_cache::dataset_materialize_cache_metrics_snapshot;
use super::route_compile::{elapsed_ms, resolve_active_route_meta};
use super::scene::resolve_scene_routes;
use super::scene_payload_cache::scene_payload_cache_metrics_snapshot;
use super::entry_payload::compile_scene_payload_for_target_uncached;

use active::precompile_and_pick_active;
use app_compile_revision::{
    build_compile_revision_plan_from_inputs, scoped_dependency_graph_routes,
};
use catalog::{compile_catalog_and_merge_resources, push_catalog_compile_diagnostics};
use finish::{finish_compiled_app, CompileCacheBefore};

pub use app_compile_revision::{
    compile_revision_plan_from_root_with_options, compile_revision_token_from_root_with_options,
    resolve_default_scene_from_root,
};

pub struct CompileAppArtifacts {
    pub compiled: CompiledApp,
    pub revision_plan: CompileRevisionPlan,
}

pub fn compile_app(source_root: &Path, app_id: &str) -> Result<CompiledApp> {
    compile_app_with_options(source_root, app_id, CompileOptions::default())
}

pub fn compile_app_with_options(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
) -> Result<CompiledApp> {
    Ok(
        compile_app_with_options_and_revision(source_root, app_id, options)?
            .compiled,
    )
}

pub fn compile_app_with_options_and_revision(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
) -> Result<CompileAppArtifacts> {
    let app_root = resolve_app_root(source_root, app_id);
    compile_app_from_root_with_options_and_revision(source_root, &app_root, options)
}

pub fn compile_app_from_root(source_root: &Path, app_root: &Path) -> Result<CompiledApp> {
    compile_app_from_root_with_options(source_root, app_root, CompileOptions::default())
}

pub fn compile_app_from_root_with_options(
    source_root: &Path,
    app_root: &Path,
    options: CompileOptions,
) -> Result<CompiledApp> {
    Ok(
        compile_app_from_root_with_options_and_revision(source_root, app_root, options)?
            .compiled,
    )
}

pub fn compile_app_from_root_with_options_and_revision(
    source_root: &Path,
    app_root: &Path,
    options: CompileOptions,
) -> Result<CompileAppArtifacts> {
    let _authoring_guard = super::authoring_eval::install_authoring_eval_context(source_root)?;
    let cache_before = CompileCacheBefore {
        l2_hits: scene_payload_cache_metrics_snapshot().0,
        l2_misses: scene_payload_cache_metrics_snapshot().1,
        l3_hits: dataset_materialize_cache_metrics_snapshot().0,
        l3_misses: dataset_materialize_cache_metrics_snapshot().1,
        catalog_index_hits: dataset_catalog_index_cache_metrics_snapshot().0,
        catalog_index_misses: dataset_catalog_index_cache_metrics_snapshot().1,
        decl_file_hits: decl_file_cache_metrics_snapshot().0,
        decl_file_misses: decl_file_cache_metrics_snapshot().1,
        graph_cache_hits: dependency_graph_cache_metrics_snapshot().0,
        graph_cache_misses: dependency_graph_cache_metrics_snapshot().1,
        content_hash_hits: file_content_hash_cache_metrics_snapshot().0,
        content_hash_misses: file_content_hash_cache_metrics_snapshot().1,
    };
    let app_entry_main = resolve_app_entry_main(app_root);
    let app_main = resolve_app_main_path(app_root);
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let mut route_registry =
        resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);

    let asset_map = load_component_assets(source_root)?;
    let preview_only = is_manage_preview_only_compile(&options, app_entry_main.as_str());
    let scene_registry = SceneRegistry::build_from_routes(&route_registry.routes);
    inject_discovered_entry_scene_routes(
        app_root,
        source_root,
        &app_decls,
        &asset_map,
        &mut route_registry.routes,
        &scene_registry,
        options.preview_target.as_deref(),
        options.scene.as_deref(),
        preview_only,
    );
    let scene_registry = SceneRegistry::build_from_routes(&route_registry.routes);
    let (active_route_meta, unknown_scene_requested) = resolve_active_route_meta(
        &route_registry.routes,
        route_registry.default_scene_id.as_deref(),
        options.scene.as_deref(),
        options.preview_target.as_deref(),
    );
    let dependency_graph_routes =
        scoped_dependency_graph_routes(&route_registry.routes, active_route_meta.as_ref(), &options);
    let dependency_graph_started = Instant::now();
    let dependency_graph =
        DependencyGraph::build_cached(app_root, &app_decls, &dependency_graph_routes);
    let dependency_graph_build_ms = elapsed_ms(dependency_graph_started);
    let preview_affected_targets = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(|target| dependency_graph.dependent_targets_for_file(target));
    if unknown_scene_requested {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "unknown_scene".to_string(),
            message: format!(
                "scene `{}` not found, fallback to default scene",
                options.scene.as_deref().unwrap_or("")
            ),
            source_path: Some(app_main.to_string_lossy().to_string()),
        });
    }
    push_app_config_diagnostics(app_root, &mut diagnostics);

    let mut active = precompile_and_pick_active(
        app_root,
        source_root,
        app_entry_main.as_str(),
        &app_decls,
        &asset_map,
        &scene_registry,
        &dependency_graph,
        &mut route_registry,
        active_route_meta,
        preview_only,
        preview_affected_targets.clone(),
        &options,
    );
    diagnostics.extend(std::mem::take(&mut active.preview_scope_diagnostics));
    active.hydrated_link_targets = hydrate_scene_links(
        app_root,
        &app_decls,
        &asset_map,
        &scene_registry,
        &mut active.active_payload,
        active.active_target_file.as_str(),
    );
    diagnostics.append(&mut active.active_payload.diagnostics);

    let title = app_decl
        .title
        .clone()
        .unwrap_or_else(|| app_decl.id.clone());

    let catalog = compile_catalog_and_merge_resources(
        app_root,
        source_root,
        app_entry_main.as_str(),
        &app_decls,
        &asset_map,
        &scene_registry,
        &dependency_graph,
        active.active_target_file.as_str(),
        &active.active_payload,
        &options,
        &mut diagnostics,
    )?;
    push_catalog_compile_diagnostics(&mut diagnostics, &app_main, &catalog);

    let revision_plan = build_compile_revision_plan_from_inputs(
        source_root,
        app_root,
        app_entry_main.as_str(),
        &app_decls,
        &dependency_graph,
        active.active_target_file.as_str(),
        is_dataset_manage_preview(&options, app_entry_main.as_str()),
        &catalog.catalog_filter,
    );

    let compiled = finish_compiled_app(
        app_root,
        app_decl.id.as_str(),
        title,
        route_registry,
        active,
        catalog,
        &dependency_graph,
        dependency_graph_build_ms,
        preview_affected_targets,
        cache_before,
        &app_main,
        &asset_map,
        &mut diagnostics,
    )?;

    Ok(CompileAppArtifacts {
        compiled,
        revision_plan,
    })
}

fn hydrate_scene_links(
    app_root: &Path,
    app_decls: &Value,
    asset_map: &BTreeMap<String, crate::model::ComponentAsset>,
    scene_registry: &SceneRegistry,
    active_payload: &mut super::entry_payload::CompiledScenePayload,
    active_target_file: &str,
) -> BTreeMap<String, (String, super::entry_payload::CompiledScenePayload)> {
    let mut hydrated = BTreeMap::<String, (String, super::entry_payload::CompiledScenePayload)>::new();
    let Some(contract) = active_payload.scene_contract.as_ref() else {
        return hydrated;
    };
    let scene_refs = collect_scene_first_target_refs(&contract.panels);
    if scene_refs.is_empty() {
        return hydrated;
    }
    let mut target_scene_contracts = BTreeMap::<String, SceneContract>::new();
    let mut target_scene_ids_by_file = BTreeMap::<String, Vec<String>>::new();
    for (scene_id, scene_file) in scene_refs {
        let target_file = if scene_file.trim().is_empty() {
            active_target_file.to_string()
        } else {
            scene_file.trim().to_string()
        };
        let route_meta = CompiledSceneRoute {
            scene_id: scene_id.clone(),
            frame_id: None,
            target_file: target_file.clone(),
            kind: "scene_first_board".to_string(),
            title: None,
            is_default: false,
            access_export: true,
        };
        let mut payload = compile_scene_payload_for_target_uncached(
            app_root,
            app_decls,
            asset_map,
            target_file.as_str(),
            Some(&route_meta),
            scene_registry,
        );
        if let Some(contract) = payload.scene_contract.as_ref() {
            let scene_ids = target_scene_ids_by_file.entry(target_file.clone()).or_default();
            if !scene_ids.iter().any(|existing| existing == &scene_id) {
                scene_ids.push(scene_id.clone());
                scene_ids.sort();
            }
            target_scene_contracts.insert(scene_id.clone(), contract.clone());
        }
        active_payload.diagnostics.append(&mut payload.diagnostics);
        hydrated.insert(scene_id, (target_file, payload));
    }
    if let Some(contract) = active_payload.scene_contract.as_mut() {
        crate::compile::projection_assembly::lower_scene_links_in_panels(
            &mut contract.panels,
            &active_payload.resources,
            active_target_file,
            &target_scene_contracts,
            &target_scene_ids_by_file,
            &mut active_payload.diagnostics,
        );
    }
    hydrated
}

fn collect_scene_first_target_refs(
    panels: &[crate::model::PanelDecl],
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for panel in panels {
        collect_scene_first_target_refs_from_value(&panel.props, &mut out);
        collect_scene_first_target_refs_from_nodes(&panel.blocks, &mut out);
    }
    out
}

fn collect_scene_first_target_refs_from_nodes(
    nodes: &[crate::model::UiNodeDecl],
    out: &mut BTreeMap<String, String>,
) {
    for node in nodes {
        match node {
            crate::model::UiNodeDecl::Panel(panel) => {
                collect_scene_first_target_refs_from_value(&panel.props, out);
                collect_scene_first_target_refs_from_nodes(&panel.blocks, out);
            }
            crate::model::UiNodeDecl::Block(block) => {
                collect_scene_first_target_refs_from_value(&block.props, out);
                if let Some(component) = block.component.as_ref() {
                    collect_scene_first_target_refs_from_value(component, out);
                }
                for child in &block.blocks {
                    collect_scene_first_target_refs_from_value(child, out);
                }
            }
            crate::model::UiNodeDecl::PanelRefEmbed(_) => {}
        }
    }
}

fn collect_scene_first_target_refs_from_value(
    value: &Value,
    out: &mut BTreeMap<String, String>,
) {
    match value {
        Value::Object(map) => {
            let is_board_link = map.get("__kind").and_then(Value::as_str) == Some("board_link")
                || map.get("mode").and_then(Value::as_str) == Some("board_link");
            if is_board_link
                && map.get("board").is_none()
                && map.get("tabs").is_none()
                && map.get("projection_slots").is_none()
            {
                if let Some(scene_ref) = map.get("scene").and_then(Value::as_object) {
                    let scene_id = scene_ref
                        .get("scene_id")
                        .or_else(|| scene_ref.get("sceneId"))
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty());
                    if let Some(scene_id) = scene_id {
                        let scene_file = scene_ref
                            .get("scene_file")
                            .or_else(|| scene_ref.get("sceneFile"))
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .unwrap_or("");
                        out.entry(scene_id.to_string())
                            .or_insert_with(|| scene_file.to_string());
                    }
                }
            }
            for child in map.values() {
                collect_scene_first_target_refs_from_value(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_scene_first_target_refs_from_value(child, out);
            }
        }
        _ => {}
    }
}
