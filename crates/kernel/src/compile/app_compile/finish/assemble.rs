use super::{
    build_scene_projection_maps, build_target_scene_contracts,
    ensure_build_tree_entry_scene_assemblies, ensure_world_capsule_preview_components,
    hydrate_board_capsules_from_file_tree, push_route_and_graph_diagnostics,
};

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Instant,
};

use anyhow::Result;

use crate::model::{CompiledApp, ComponentAsset, Diagnostic, Severity, WorldSemanticFileIndex};
use crate::workspace::source_tree;

use super::super::super::dependency_graph::DependencyGraph;
use super::super::super::route_compile::elapsed_ms;
use super::super::super::scene::SceneRouteRegistry;
use super::super::active::ActiveCompileResult;
use super::super::catalog::CatalogCompileResult;

pub(in crate::compile::app_compile) struct CompileCacheBefore {
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l3_hits: u64,
    pub l3_misses: u64,
    pub catalog_index_hits: u64,
    pub catalog_index_misses: u64,
    pub decl_file_hits: u64,
    pub decl_file_misses: u64,
    pub graph_cache_hits: u64,
    pub graph_cache_misses: u64,
    pub content_hash_hits: u64,
    pub content_hash_misses: u64,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::compile::app_compile) fn finish_compiled_app(
    app_root: &Path,
    app_id: &str,
    title: String,
    route_registry: SceneRouteRegistry,
    active: ActiveCompileResult,
    catalog: CatalogCompileResult,
    dependency_graph: &DependencyGraph,
    dependency_graph_build_ms: u64,
    preview_affected_targets: Option<BTreeSet<String>>,
    cache_before: CompileCacheBefore,
    app_main: &Path,
    asset_map: &BTreeMap<String, ComponentAsset>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<CompiledApp> {
    let app_main_source = app_main.to_string_lossy().to_string();
    let ActiveCompileResult {
        official_results,
        precompile_routes,
        route_precompile_stats,
        official_results_all_routes_ms,
        active_scene,
        active_target_file,
        mut active_payload,
        active_payload_pick_or_compile_ms,
        hydrated_link_targets,
        preview_scope_diagnostics: _,
    } = active;
    let CatalogCompileResult {
        resources,
        world_metrics,
        dataset_manage_preview,
        catalog_filter,
        catalog_seed_files,
        catalog_compile_ms,
        resource_merge_ms,
        world_metric_ledger_ms,
        ..
    } = catalog;

    let mut target_scene_contracts =
        build_target_scene_contracts(&official_results, &active_payload);
    let target_scene_ids_by_file = route_registry.routes.iter().fold(
        BTreeMap::<String, Vec<String>>::new(),
        |mut acc, route| {
            let scene_ids = acc.entry(route.target_file.clone()).or_default();
            if !scene_ids.iter().any(|scene_id| scene_id == &route.scene_id) {
                scene_ids.push(route.scene_id.clone());
                scene_ids.sort();
            }
            acc
        },
    );
    if let Some(contract) = active_payload.scene_contract.as_mut() {
        crate::compile::projection_assembly::lower_scene_links_in_panels(
            &mut contract.panels,
            &active_payload.resources,
            &active_target_file,
            &target_scene_contracts,
            &target_scene_ids_by_file,
            diagnostics,
        );
    }

    let world_finalize_started = Instant::now();
    let scene_projection_started = Instant::now();
    let (
        mut scene_local_nav_by_target,
        mut scene_bindings_by_id,
        mut scene_examples_by_id,
        mut scene_projection_assembly_by_id,
    ) = build_scene_projection_maps(
        &route_registry,
        &official_results,
        active_scene.as_deref(),
        &active_target_file,
        &active_payload,
        &hydrated_link_targets,
        diagnostics,
    );
    let scene_projection_assembly_ms = elapsed_ms(scene_projection_started);
    let source_tree_started = Instant::now();
    let mut file_tree = source_tree(app_root)?;
    crate::compile::source_tree_enrich::enrich_source_tree_with_scene_exports(
        app_root,
        &mut file_tree,
    );
    let mut world_semantic_by_file = BTreeMap::new();
    crate::compile::source_tree_world::enrich_source_tree_with_world_capsules(
        app_root,
        &mut file_tree,
        &mut world_semantic_by_file,
    );
    hydrate_board_capsules_from_file_tree(
        app_root,
        app_main,
        asset_map,
        &route_registry,
        &file_tree,
        &mut scene_projection_assembly_by_id,
        &mut scene_bindings_by_id,
        &mut scene_examples_by_id,
        &mut scene_local_nav_by_target,
        &mut target_scene_contracts,
        diagnostics,
    );
    ensure_build_tree_entry_scene_assemblies(
        app_root,
        app_main,
        asset_map,
        dependency_graph,
        &route_registry,
        active_target_file.as_str(),
        &mut scene_projection_assembly_by_id,
        &mut scene_bindings_by_id,
        &mut scene_examples_by_id,
        &mut scene_local_nav_by_target,
        diagnostics,
    );
    if active_target_file.ends_with(".world.mei") {
        world_semantic_by_file
            .entry(active_target_file.clone())
            .or_insert_with(|| {
                crate::compile::source_tree_world::build_world_semantic_index(
                    app_root,
                    active_target_file.as_str(),
                )
                .unwrap_or(WorldSemanticFileIndex {
                    world_id: None,
                    datasets: Vec::new(),
                    metrics: Vec::new(),
                    resource_id: "__world_metrics__".to_string(),
                })
            });
        ensure_world_capsule_preview_components(&mut active_payload.component_assets, asset_map);
    }
    let source_tree_ms = elapsed_ms(source_tree_started);
    let world_finalize_ms = elapsed_ms(world_finalize_started);

    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "compile_stage_timing".to_string(),
        message: format!(
            "dependency_graph_build_ms={}, official_results_all_routes_ms={}, active_payload_pick_or_compile_ms={}, catalog_compile_ms={}, resource_merge_ms={}, world_metric_ledger_ms={}, scene_projection_assembly_ms={}, source_tree_ms={}, world_finalize_ms={}",
            dependency_graph_build_ms,
            official_results_all_routes_ms,
            active_payload_pick_or_compile_ms,
            catalog_compile_ms,
            resource_merge_ms,
            world_metric_ledger_ms,
            scene_projection_assembly_ms,
            source_tree_ms,
            world_finalize_ms
        ),
        source_path: Some(app_main_source.clone()),
    });
    push_route_and_graph_diagnostics(
        diagnostics,
        &route_registry,
        &precompile_routes,
        &route_precompile_stats,
        dependency_graph,
        preview_affected_targets.as_ref(),
        &catalog_seed_files,
        dataset_manage_preview,
        &catalog_filter,
        &active_target_file,
        active_scene.as_deref(),
        &active_payload,
        &resources,
        &cache_before,
        &app_main_source,
    );

    let mut compiled = CompiledApp {
        app_id: app_id.to_string(),
        title,
        app_root: app_root.to_string_lossy().to_string(),
        scene_routes: route_registry.routes,
        active_scene,
        active_target_file,
        file_tree,
        scene_contract: active_payload.scene_contract.take(),
        scene_local_nav_by_target,
        scene_bindings_by_id,
        scene_examples_by_id,
        scene_projection_assembly_by_id,
        resources,
        world_metrics,
        world_semantic_by_file,
        component_assets: std::mem::take(&mut active_payload.component_assets),
        diagnostics: std::mem::take(diagnostics),
        build_experience_index: Default::default(),
        build_t2_page_index: Default::default(),
        build_template_index: Default::default(),
        ui_layout_index: Default::default(),
    };
    compiled.build_experience_index =
        crate::compile::build_experience_index::build_experience_index(
            &compiled.scene_routes,
            &compiled.scene_projection_assembly_by_id,
            &target_scene_contracts,
            &compiled,
        );
    let board = crate::compile::build_t2_page_index(
        &compiled.file_tree,
        &target_scene_contracts,
        &compiled.scene_projection_assembly_by_id,
    );
    compiled.build_t2_page_index = board.index;
    ensure_world_capsule_preview_components(&mut compiled.component_assets, asset_map);
    let workspace_source_root =
        crate::mei_config::resolve_workspace_source_root_from_app_root(app_root);
    let is_catalog_app = crate::mei_config::is_stock_catalog_app(compiled.app_id.as_str());
    let catalog_assets = if is_catalog_app {
        crate::compile::build_template_index::merged_component_catalog(
            workspace_source_root.as_path(),
            &compiled,
        )
    } else {
        Vec::new()
    };
    let template = if is_catalog_app {
        crate::compile::build_template_index(
            catalog_assets.as_slice(),
            &target_scene_contracts,
            &compiled.build_experience_index.node_manifest,
        )
    } else {
        crate::compile::build_template_index::BuildTemplateIndexResult {
            index: Default::default(),
            tree_root: crate::compile::ReachabilityTreeRoot {
                group: "templates".to_string(),
                label: "Components".to_string(),
                default_open: false,
                children: Vec::new(),
            },
        }
    };
    compiled.build_template_index = template.index;
    let ui_layout = crate::compile::build_ui_layout_index::build_ui_layout_index(&compiled);
    for node_id in &ui_layout.duplicate_node_ids {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "ui_scope_duplicate_node_id".to_string(),
            message: format!("duplicate ui-scope node id: {node_id}"),
            source_path: None,
        });
    }
    compiled.ui_layout_index = ui_layout.index;
    let template_files = if is_catalog_app {
        crate::compile::build_template_index::build_stock_template_files_root(
            workspace_source_root.as_path(),
        )
    } else {
        crate::compile::ReachabilityTreeRoot {
            group: "template_files".to_string(),
            label: "Templates".to_string(),
            default_open: false,
            children: Vec::new(),
        }
    };
    let mut reachability_snapshot =
        crate::compile::build_experience_index::merge_build_view_tree_roots(
            compiled
                .build_experience_index
                .reachability_snapshot
                .clone(),
            board.tree_root,
            template.tree_root,
            template_files,
        );
    crate::compile::build_ui_layout_index::merge_ui_structure_root(
        &mut reachability_snapshot,
        ui_layout.tree_root,
    );
    crate::compile::build_experience_index::annotate_stock_preview_availability(
        &mut reachability_snapshot,
        &compiled,
        diagnostics,
    );
    compiled.build_experience_index.reachability_snapshot = reachability_snapshot;
    crate::compile::canonicalize_compiled_app_source_paths(&mut compiled);
    Ok(compiled)
}
