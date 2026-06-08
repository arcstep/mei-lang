use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Instant,
};

use anyhow::Result;
use serde_json::Value;

use crate::model::{Diagnostic, LoadedResource, Severity, WorldMetricLedgerEntry};
use crate::typed_refs::SceneRegistry;

use super::super::catalog::{
    build_dataset_catalog_filter, catalog_compile_parallelism, compile_dataset_catalog_resources,
    merge_resource_catalog, DatasetCatalogFilter,
};
use super::super::dependency_graph::DependencyGraph;
use super::super::discover_routes::{build_world_metric_ledger, catalog_focus_target, CompileOptions};
use super::super::entry_payload::CompiledScenePayload;
use super::super::materialize::append_world_metrics_dataset_resource;
use super::super::route_compile::elapsed_ms;
use super::super::scene_payload_cache::scene_payload_cache_metrics_snapshot;
use super::super::ui_data_policy::validate_imported_catalog_world_refs;

pub(super) struct CatalogCompileResult {
    pub resources: Vec<LoadedResource>,
    pub world_metrics: BTreeMap<String, WorldMetricLedgerEntry>,
    pub dataset_manage_preview: bool,
    pub catalog_filter: DatasetCatalogFilter,
    pub catalog_seed_files: BTreeSet<String>,
    pub catalog_compile_rels: usize,
    pub catalog_parallelism: usize,
    pub catalog_compile_ms: u64,
    pub catalog_l2_hit_delta: u64,
    pub catalog_l2_miss_delta: u64,
    pub resource_merge_ms: u64,
    pub world_metric_ledger_ms: u64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compile_catalog_and_merge_resources(
    app_root: &Path,
    source_root: &Path,
    app_entry_main: &str,
    app_decls: &Value,
    asset_map: &BTreeMap<String, crate::model::ComponentAsset>,
    _scene_registry: &SceneRegistry,
    dependency_graph: &DependencyGraph,
    active_target_file: &str,
    active_payload: &CompiledScenePayload,
    options: &CompileOptions,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<CatalogCompileResult> {
    let dataset_manage_preview = super::super::discover_routes::is_dataset_manage_preview(
        options,
        app_entry_main,
    );
    let catalog_focus = catalog_focus_target(options, Some(active_target_file));
    let catalog_seed_files =
        dependency_graph.catalog_seed_files(app_root, app_decls, catalog_focus);
    let catalog_filter = if dataset_manage_preview {
        DatasetCatalogFilter::default()
    } else {
        build_dataset_catalog_filter(app_root, app_decls, dependency_graph, catalog_focus)
    };
    let mut catalog_compile_rels = 0usize;
    let mut catalog_parallelism = 0usize;
    let mut catalog_compile_ms = 0u64;
    let mut catalog_l2_hit_delta = 0u64;
    let mut catalog_l2_miss_delta = 0u64;
    let dataset_catalog = if dataset_manage_preview {
        Vec::new()
    } else {
        let l2_before_catalog = scene_payload_cache_metrics_snapshot();
        let catalog_started = Instant::now();
        catalog_compile_rels =
            super::super::catalog::resolve_dataset_catalog_compile_rels(app_root, &catalog_filter)
                .len();
        catalog_parallelism = catalog_compile_parallelism(catalog_compile_rels);
        let out = compile_dataset_catalog_resources(
            app_root,
            source_root,
            app_decls,
            asset_map,
            &catalog_filter,
            dependency_graph,
        );
        catalog_compile_ms = elapsed_ms(catalog_started);
        let l2_after_catalog = scene_payload_cache_metrics_snapshot();
        catalog_l2_hit_delta = l2_after_catalog.0.saturating_sub(l2_before_catalog.0);
        catalog_l2_miss_delta = l2_after_catalog.1.saturating_sub(l2_before_catalog.1);
        out
    };
    let resource_merge_started = Instant::now();
    let scene_resources = active_payload.resources.clone();
    let mut resources = merge_resource_catalog(dataset_catalog, scene_resources);
    let resource_merge_ms = elapsed_ms(resource_merge_started);
    let world_metric_ledger_started = Instant::now();
    let direct_world_metrics = active_payload
        .scene_contract
        .as_ref()
        .and_then(|contract| contract.world.as_ref())
        .map(|world| world.metrics.as_slice())
        .unwrap_or(&[]);
    let world_metrics = build_world_metric_ledger(&resources, direct_world_metrics)?;
    append_world_metrics_dataset_resource(&mut resources, &world_metrics, direct_world_metrics);
    let world_metric_ledger_ms = elapsed_ms(world_metric_ledger_started);
    if let Some(contract) = active_payload.scene_contract.as_ref() {
        validate_imported_catalog_world_refs(
            contract,
            &active_payload.resources,
            &resources,
            active_target_file,
            diagnostics,
        );
    }

    Ok(CatalogCompileResult {
        resources,
        world_metrics,
        dataset_manage_preview,
        catalog_filter,
        catalog_seed_files,
        catalog_compile_rels,
        catalog_parallelism,
        catalog_compile_ms,
        catalog_l2_hit_delta,
        catalog_l2_miss_delta,
        resource_merge_ms,
        world_metric_ledger_ms,
    })
}

pub(super) fn push_catalog_compile_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    app_main: &std::path::Path,
    catalog: &CatalogCompileResult,
) {
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "catalog_compile_stats".to_string(),
        message: format!(
            "dataset_manage_preview={}, compile_rels={}, parallelism={}, l2_hits_delta={}, l2_misses_delta={}, catalog_compile_ms={}",
            catalog.dataset_manage_preview,
            catalog.catalog_compile_rels,
            catalog.catalog_parallelism,
            catalog.catalog_l2_hit_delta,
            catalog.catalog_l2_miss_delta,
            catalog.catalog_compile_ms
        ),
        source_path: Some(app_main.to_string_lossy().to_string()),
    });
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "catalog_parallelism_eval".to_string(),
        message: if catalog.dataset_manage_preview {
            "decision=skip_preview_scope".to_string()
        } else if catalog.catalog_compile_rels >= 8 && catalog.catalog_compile_ms >= 120 {
            format!(
                "decision=candidate, reason=high_catalog_cost, compile_rels={}, parallelism={}, catalog_compile_ms={}",
                catalog.catalog_compile_rels, catalog.catalog_parallelism, catalog.catalog_compile_ms
            )
        } else {
            format!(
                "decision=defer, reason=low_catalog_cost, compile_rels={}, parallelism={}, catalog_compile_ms={}",
                catalog.catalog_compile_rels, catalog.catalog_parallelism, catalog.catalog_compile_ms
            )
        },
        source_path: Some(app_main.to_string_lossy().to_string()),
    });
}
