mod active;
mod catalog;
mod finish;

#[path = "../app_compile_revision.rs"]
mod app_compile_revision;

use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};

use crate::{
    eval::evaluate_mei_file,
    mei_config::{resolve_app_entry_main, resolve_app_main_path, resolve_app_root},
    model::{CompiledApp, Diagnostic, Severity},
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
    inject_discovered_entry_scene_routes, is_manage_preview_only_compile,
    push_app_config_diagnostics, CompileOptions,
};
use super::materialize_cache::dataset_materialize_cache_metrics_snapshot;
use super::route_compile::{elapsed_ms, resolve_active_route_meta};
use super::scene::resolve_scene_routes;
use super::scene_payload_cache::scene_payload_cache_metrics_snapshot;

use active::precompile_and_pick_active;
use catalog::{compile_catalog_and_merge_resources, push_catalog_compile_diagnostics};
use finish::{finish_compiled_app, CompileCacheBefore};

pub use app_compile_revision::{
    compile_revision_plan_from_root_with_options, compile_revision_token_from_root_with_options,
    resolve_default_scene_from_root,
};

pub fn compile_app(source_root: &Path, app_id: &str) -> Result<CompiledApp> {
    compile_app_with_options(source_root, app_id, CompileOptions::default())
}

pub fn compile_app_with_options(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
) -> Result<CompiledApp> {
    let app_root = resolve_app_root(source_root, app_id);
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
    let dependency_graph_started = Instant::now();
    let dependency_graph =
        DependencyGraph::build_cached(app_root, &app_decls, &route_registry.routes);
    let dependency_graph_build_ms = elapsed_ms(dependency_graph_started);
    let preview_affected_targets = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(|target| dependency_graph.dependent_targets_for_file(target));
    let (active_route_meta, unknown_scene_requested) = resolve_active_route_meta(
        &route_registry.routes,
        route_registry.default_scene_id.as_deref(),
        options.scene.as_deref(),
        options.preview_target.as_deref(),
    );
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

    finish_compiled_app(
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
        &mut diagnostics,
    )
}
