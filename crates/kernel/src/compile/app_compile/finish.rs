use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Instant,
};

use anyhow::Result;
use serde_json::Value;

use crate::compile::entry_payload::CompiledScenePayload;
use crate::model::{
    CompiledApp, ComponentAsset, Diagnostic, LoadedResource, SceneContract, Severity,
    WorldSemanticFileIndex,
};
use crate::workspace::source_tree;

use super::super::catalog::DatasetCatalogFilter;
use super::super::decl_file_cache::decl_file_cache_metrics_snapshot;
use super::super::dependency_graph::DependencyGraph;
use super::super::materialize_cache::dataset_materialize_cache_metrics_snapshot;
use super::super::route_compile::{elapsed_ms, RoutePrecompileStats};
use super::super::scene::SceneRouteRegistry;
use super::super::shards;
use super::super::{
    catalog::dataset_catalog_index_cache_metrics_snapshot,
    dependency_graph::{
        dependency_graph_cache_metrics_snapshot, file_content_hash_cache_metrics_snapshot,
    },
    scene_payload_cache::scene_payload_cache_metrics_snapshot,
};
use super::active::ActiveCompileResult;
use super::catalog::CatalogCompileResult;

pub(super) struct CompileCacheBefore {
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
pub(super) fn finish_compiled_app(
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

    let target_scene_contracts = build_target_scene_contracts(&official_results, &active_payload);
    let target_scene_ids_by_file = route_registry
        .routes
        .iter()
        .fold(BTreeMap::<String, Vec<String>>::new(), |mut acc, route| {
            let scene_ids = acc.entry(route.target_file.clone()).or_default();
            if !scene_ids.iter().any(|scene_id| scene_id == &route.scene_id) {
                scene_ids.push(route.scene_id.clone());
                scene_ids.sort();
            }
            acc
        });
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
        scene_local_nav_by_target,
        scene_bindings_by_id,
        scene_examples_by_id,
        scene_projection_assembly_by_id,
    ) = build_scene_projection_maps(
        &route_registry,
        &official_results,
        active_scene.as_deref(),
        &active_target_file,
        &active_payload,
        &hydrated_link_targets,
    );
    let scene_projection_assembly_ms = elapsed_ms(scene_projection_started);
    let source_tree_started = Instant::now();
    let mut file_tree = source_tree(app_root)?;
    super::super::source_tree_enrich::enrich_source_tree_with_scene_exports(app_root, &mut file_tree);
    let mut world_semantic_by_file = BTreeMap::new();
    super::super::source_tree_world::enrich_source_tree_with_world_capsules(
        app_root,
        &mut file_tree,
        &mut world_semantic_by_file,
    );
    if active_target_file.ends_with(".world.mei") {
        world_semantic_by_file
            .entry(active_target_file.clone())
            .or_insert_with(|| {
                super::super::source_tree_world::build_world_semantic_index(
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
        ensure_world_capsule_preview_components(
            &mut active_payload.component_assets,
            asset_map,
        );
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
        build_board_index: Default::default(),
        build_template_index: Default::default(),
    };
    compiled.build_experience_index = crate::compile::build_experience_index::build_experience_index(
        &compiled.scene_routes,
        &compiled.scene_projection_assembly_by_id,
        &target_scene_contracts,
        &compiled,
    );
    let board = crate::compile::build_board_index(
        &compiled.file_tree,
        &target_scene_contracts,
        &compiled.scene_projection_assembly_by_id,
    );
    compiled.build_board_index = board.index;
    let template = crate::compile::build_template_index(
        &compiled.component_assets,
        &target_scene_contracts,
        &compiled.build_experience_index.node_manifest,
    );
    compiled.build_template_index = template.index;
    compiled.build_experience_index.reachability_snapshot =
        crate::compile::build_experience_index::merge_build_view_tree_roots(
            compiled.build_experience_index.reachability_snapshot.clone(),
            board.tree_root,
            template.tree_root,
        );
    Ok(compiled)
}

const WORLD_CAPSULE_PREVIEW_COMPONENT_KEYS: &[&str] = &["dataset.table"];

fn ensure_world_capsule_preview_components(
    component_assets: &mut Vec<ComponentAsset>,
    asset_map: &BTreeMap<String, ComponentAsset>,
) {
    for key in WORLD_CAPSULE_PREVIEW_COMPONENT_KEYS {
        if component_assets.iter().any(|asset| asset.key == *key) {
            continue;
        }
        if let Some(asset) = asset_map.get(*key) {
            component_assets.push(asset.clone());
        }
    }
}

fn build_target_scene_contracts(
    official_results: &BTreeMap<String, CompiledScenePayload>,
    active_payload: &CompiledScenePayload,
) -> BTreeMap<String, crate::model::SceneContract> {
    let mut out = BTreeMap::new();
    for (scene_id, payload) in official_results {
        if let Some(contract) = payload.scene_contract.as_ref() {
            out.insert(scene_id.clone(), contract.clone());
        }
    }
    if let Some(contract) = active_payload.scene_contract.as_ref() {
        out.insert(contract.scene.id.clone(), contract.clone());
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn push_route_and_graph_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    route_registry: &SceneRouteRegistry,
    precompile_routes: &[crate::model::CompiledSceneRoute],
    route_precompile_stats: &RoutePrecompileStats,
    dependency_graph: &DependencyGraph,
    preview_affected_targets: Option<&BTreeSet<String>>,
    catalog_seed_files: &BTreeSet<String>,
    dataset_manage_preview: bool,
    catalog_filter: &DatasetCatalogFilter,
    active_target_file: &str,
    active_scene: Option<&str>,
    active_payload: &CompiledScenePayload,
    resources: &[LoadedResource],
    cache_before: &CompileCacheBefore,
    app_main_source: &str,
) {
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "route_precompile_stats".to_string(),
        message: format!(
            "routes_total={}, routes_precompile_candidates={}, routes_attempted={}, routes_l2_hits={}, routes_l2_misses={}, routes_recompiled={}, parallelism={}",
            route_registry.routes.len(),
            precompile_routes.len(),
            route_precompile_stats.attempted,
            route_precompile_stats.l2_hits,
            route_precompile_stats.l2_misses,
            route_precompile_stats.l2_misses,
            route_precompile_stats.parallelism
        ),
        source_path: Some(app_main_source.to_string()),
    });

    let active_shard =
        shards::build_scene_payload_shard(active_target_file, active_scene, active_payload);
    let dataset_shard = shards::build_dataset_materialization_shard(
        "__catalog__",
        &resources
            .iter()
            .filter(|resource| resource.dataset.is_some())
            .cloned()
            .collect::<Vec<_>>(),
    );
    let imported_scope_shards = shards::collect_imported_scope_shards(resources);
    let graph_stats = dependency_graph.stats();
    let preview_scope_size = preview_affected_targets.map(BTreeSet::len).unwrap_or(0);
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "dependency_graph_stats".to_string(),
        message: format!(
            "routes={}, unique_files={}, edges={}, max_closure={}, preview_scope={}, catalog_seed_files={}",
            graph_stats.route_roots,
            graph_stats.unique_files,
            graph_stats.edges,
            graph_stats.max_closure,
            preview_scope_size,
            catalog_seed_files.len()
        ),
        source_path: Some(app_main_source.to_string()),
    });
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "catalog_filter_stats".to_string(),
        message: format!(
            "dataset_manage_preview={}, dataset_paths={}, resource_ids={}, metric_ids={}",
            dataset_manage_preview,
            catalog_filter.dataset_paths.len(),
            catalog_filter.resource_ids.len(),
            catalog_filter.metric_ids.len(),
        ),
        source_path: Some(app_main_source.to_string()),
    });
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "compile_shards_stats".to_string(),
        message: format!(
            "scene_shard_target={}, scene_resources={}, scene_assets={}, scene_has_contract={}, scene_id={}, dataset_shard_file={}, dataset_shard_resources={}, imported_scope_shards={}, imported_scope_resources={}, imported_scope_ids={}",
            active_shard.target_file,
            active_shard.resources.len(),
            active_shard.component_assets.len(),
            active_shard.scene_contract.is_some(),
            active_shard.scene_id.as_deref().unwrap_or("-"),
            dataset_shard.dataset_file,
            dataset_shard.resources.len(),
            imported_scope_shards.len(),
            imported_scope_shards
                .iter()
                .map(|shard| shard.resources.len())
                .sum::<usize>(),
            imported_scope_shards
                .iter()
                .map(|shard| shard.import_scope.as_str())
                .collect::<Vec<_>>()
                .join(",")
        ),
        source_path: Some(app_main_source.to_string()),
    });
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "compile_cache_stats".to_string(),
        message: {
            let (l2_hits_after, l2_misses_after) = scene_payload_cache_metrics_snapshot();
            let (l3_hits_after, l3_misses_after) = dataset_materialize_cache_metrics_snapshot();
            let (catalog_index_hits_after, catalog_index_misses_after) =
                dataset_catalog_index_cache_metrics_snapshot();
            let (decl_file_hits_after, decl_file_misses_after) = decl_file_cache_metrics_snapshot();
            let (graph_cache_hits_after, graph_cache_misses_after) =
                dependency_graph_cache_metrics_snapshot();
            let (content_hash_hits_after, content_hash_misses_after) =
                file_content_hash_cache_metrics_snapshot();
            format!(
                "l2_hits_delta={}, l2_misses_delta={}, l3_hits_delta={}, l3_misses_delta={}, catalog_index_hits_delta={}, catalog_index_misses_delta={}, decl_file_hits_delta={}, decl_file_misses_delta={}, graph_cache_hits_delta={}, graph_cache_misses_delta={}, content_hash_hits_delta={}, content_hash_misses_delta={}",
                l2_hits_after.saturating_sub(cache_before.l2_hits),
                l2_misses_after.saturating_sub(cache_before.l2_misses),
                l3_hits_after.saturating_sub(cache_before.l3_hits),
                l3_misses_after.saturating_sub(cache_before.l3_misses),
                catalog_index_hits_after.saturating_sub(cache_before.catalog_index_hits),
                catalog_index_misses_after.saturating_sub(cache_before.catalog_index_misses),
                decl_file_hits_after.saturating_sub(cache_before.decl_file_hits),
                decl_file_misses_after.saturating_sub(cache_before.decl_file_misses),
                graph_cache_hits_after.saturating_sub(cache_before.graph_cache_hits),
                graph_cache_misses_after.saturating_sub(cache_before.graph_cache_misses),
                content_hash_hits_after.saturating_sub(cache_before.content_hash_hits),
                content_hash_misses_after.saturating_sub(cache_before.content_hash_misses),
            )
        },
        source_path: None,
    });
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "compile_optimization_status".to_string(),
        message: format!(
            "dependency_graph=on,preview_scope=on,l2=on,l3=on,catalog_index=on,content_hash=on,graph_cache_delta={},catalog_index_cache_delta={},content_hash_cache_delta={}",
            dependency_graph_cache_metrics_snapshot()
                .0
                .saturating_sub(cache_before.graph_cache_hits)
                + dependency_graph_cache_metrics_snapshot()
                    .1
                    .saturating_sub(cache_before.graph_cache_misses),
            dataset_catalog_index_cache_metrics_snapshot()
                .0
                .saturating_sub(cache_before.catalog_index_hits)
                + dataset_catalog_index_cache_metrics_snapshot()
                    .1
                    .saturating_sub(cache_before.catalog_index_misses),
            file_content_hash_cache_metrics_snapshot()
                .0
                .saturating_sub(cache_before.content_hash_hits)
                + file_content_hash_cache_metrics_snapshot()
                    .1
                    .saturating_sub(cache_before.content_hash_misses),
        ),
        source_path: Some(app_main_source.to_string()),
    });
}

fn insert_scene_projection_assembly_entry(
    scene_projection_assembly_by_id: &mut BTreeMap<String, Value>,
    scene_bindings_by_id: &mut BTreeMap<String, Value>,
    scene_examples_by_id: &mut BTreeMap<String, Value>,
    scene_local_nav_by_target: &mut BTreeMap<String, Value>,
    scene_id: &str,
    target_file: &str,
    kind: Option<&str>,
    title: Option<&str>,
    contract: &SceneContract,
    resources: &[LoadedResource],
) {
    let mut assembly = serde_json::Map::new();
    assembly.insert(
        "scene_id".to_string(),
        Value::String(scene_id.to_string()),
    );
    assembly.insert(
        "target_file".to_string(),
        Value::String(target_file.to_string()),
    );
    if let Some(kind) = kind.map(str::trim).filter(|value| !value.is_empty()) {
        assembly.insert("kind".to_string(), Value::String(kind.to_string()));
    }
    if let Some(title) = title.map(str::trim).filter(|value| !value.is_empty()) {
        assembly.insert("title".to_string(), Value::String(title.to_string()));
    }
    if !contract.scene.bindings.is_null() {
        scene_bindings_by_id.insert(scene_id.to_string(), contract.scene.bindings.clone());
        assembly.insert("bindings".to_string(), contract.scene.bindings.clone());
    }
    if !contract.scene.examples.is_null() {
        scene_examples_by_id.insert(scene_id.to_string(), contract.scene.examples.clone());
        assembly.insert("examples".to_string(), contract.scene.examples.clone());
    }
    if !contract.scene.local_nav.is_null() {
        scene_local_nav_by_target.insert(target_file.to_string(), contract.scene.local_nav.clone());
        assembly.insert("local_nav".to_string(), contract.scene.local_nav.clone());
    }
    if !contract.scene.params.is_null() {
        assembly.insert("params".to_string(), contract.scene.params.clone());
    }
    if let Some(frame) = contract.frame.as_ref() {
        assembly.insert(
            "frame".to_string(),
            serde_json::to_value(frame).unwrap_or(Value::Null),
        );
    }
    if !contract.panels.is_empty() {
        assembly.insert(
            "panels".to_string(),
            serde_json::to_value(&contract.panels).unwrap_or(Value::Null),
        );
    }
    if let Some(shell_contract) =
        crate::compile::projection_assembly::scene_shell_contract_from_scene_contract(contract)
    {
        assembly.insert("shell_contract".to_string(), Value::Object(shell_contract));
    }
    crate::compile::projection_assembly::enrich_scene_projection_assembly_preview(
        &mut assembly,
        contract,
        resources,
        target_file,
    );
    scene_projection_assembly_by_id.insert(scene_id.to_string(), Value::Object(assembly));
}

/// Popup drilldown only needs shell + preview slots, not full frame/panels in HTML context.
fn insert_hydrated_link_projection_assembly_entry(
    scene_projection_assembly_by_id: &mut BTreeMap<String, Value>,
    scene_bindings_by_id: &mut BTreeMap<String, Value>,
    scene_examples_by_id: &mut BTreeMap<String, Value>,
    scene_local_nav_by_target: &mut BTreeMap<String, Value>,
    scene_id: &str,
    target_file: &str,
    contract: &SceneContract,
    resources: &[LoadedResource],
) {
    if !contract.scene.bindings.is_null() {
        scene_bindings_by_id.insert(scene_id.to_string(), contract.scene.bindings.clone());
    }
    if !contract.scene.examples.is_null() {
        scene_examples_by_id.insert(scene_id.to_string(), contract.scene.examples.clone());
    }
    if !contract.scene.local_nav.is_null() {
        scene_local_nav_by_target.insert(target_file.to_string(), contract.scene.local_nav.clone());
    }
    let mut assembly = serde_json::Map::new();
    assembly.insert(
        "scene_id".to_string(),
        Value::String(scene_id.to_string()),
    );
    assembly.insert(
        "target_file".to_string(),
        Value::String(target_file.to_string()),
    );
    assembly.insert(
        "kind".to_string(),
        Value::String("scene_first_board".to_string()),
    );
    if !contract.scene.local_nav.is_null() {
        assembly.insert("local_nav".to_string(), contract.scene.local_nav.clone());
    }
    if let Some(shell_contract) =
        crate::compile::projection_assembly::scene_shell_contract_from_scene_contract(contract)
    {
        assembly.insert("shell_contract".to_string(), Value::Object(shell_contract));
    }
    crate::compile::projection_assembly::enrich_scene_projection_assembly_preview(
        &mut assembly,
        contract,
        resources,
        target_file,
    );
    scene_projection_assembly_by_id.insert(scene_id.to_string(), Value::Object(assembly));
}

fn build_scene_projection_maps(
    route_registry: &SceneRouteRegistry,
    official_results: &BTreeMap<String, CompiledScenePayload>,
    active_scene: Option<&str>,
    active_target_file: &str,
    active_payload: &CompiledScenePayload,
    hydrated_link_targets: &BTreeMap<String, (String, CompiledScenePayload)>,
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
                if let Some(shell_contract) = crate::compile::projection_assembly::scene_shell_contract_from_scene_contract(contract) {
                    assembly_map.insert("shell_contract".to_string(), Value::Object(shell_contract));
                }
                crate::compile::projection_assembly::enrich_scene_projection_assembly_preview(
                    assembly_map,
                    contract,
                    &active_payload.resources,
                    active_target_file,
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
