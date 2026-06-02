use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Instant, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use serde_json::Value;
use walkdir::WalkDir;

use crate::{
    eval::evaluate_mei_file,
    model::{
        CompiledApp, CompiledSceneRoute, ComponentAsset, Diagnostic, LoadedResource, Severity,
        WorldMetricLedgerEntry,
    },
    typed_refs::SceneRegistry,
    workspace::{load_component_assets, source_tree},
};

mod analysis;
mod app_decl;
mod catalog;
mod decl_file_cache;
mod decls;
mod dependency_graph;
mod entry_payload;
mod load_external;
mod loaders;
pub use analysis::dates::coerce_rows_to_schema;
pub use loaders::materialize_xlsx_column_headers;
mod materialize;
mod materialize_cache;
mod mutations;
mod panel_normalize;
mod projection_assembly;
mod resources;
mod scene;
mod scene_binding;
mod scene_payload_cache;
mod shards;
mod ui_data_policy;

use ui_data_policy::validate_imported_catalog_world_refs;

use app_decl::decode_app_decl;
use catalog::{
    build_dataset_catalog_filter, catalog_compile_parallelism, clear_dataset_catalog_index_cache,
    compile_dataset_catalog_resources, dataset_catalog_index_cache_metrics_snapshot,
    merge_resource_catalog, DatasetCatalogFilter,
};
use decl_file_cache::{clear_decl_file_cache, decl_file_cache_metrics_snapshot};
use dependency_graph::{
    clear_dependency_graph_cache, clear_file_content_hash_cache,
    dependency_graph_cache_metrics_snapshot, file_content_hash_cache_metrics_snapshot,
    DependencyGraph,
};
use entry_payload::CompiledScenePayload;
use materialize::{append_world_metrics_dataset_resource, materialize_world_metrics};
use materialize_cache::{clear_materialize_cache, dataset_materialize_cache_metrics_snapshot};
use scene::{find_scene_route, resolve_scene_routes};
use scene_payload_cache::{
    clear_scene_payload_cache, compile_scene_payload_for_target, scene_payload_cache_has_entry,
    scene_payload_cache_metrics_snapshot,
};

/// 将「仅声明在入口 .mei 内、未出现在 app 路由表」的 scene 登记为临时 file_ref 路由，
/// 以便管理态预览与访问态 `/scene/<id>` 能解析到同一入口文件。
///
/// 与 `mei-lang-server` 的 `compile_revision`（目录最新 mtime）配合：磁盘一旦有变更即编译缓存失效，
/// 下一次访问会重新走本逻辑。按 `.mei` 修改时间倒序探测，使 Agent 刚写入的入口优先命中。
fn try_push_discovered_entry_route(
    routes: &mut Vec<CompiledSceneRoute>,
    scene_id: String,
    target_file: String,
    access_export: bool,
) {
    let scene_id = scene_id.trim().to_string();
    let target_file = target_file.trim().to_string();
    if scene_id.is_empty() || target_file.is_empty() {
        return;
    }
    if routes.iter().any(|r| r.target_file == target_file) {
        return;
    }
    if routes.iter().any(|r| r.scene_id == scene_id) {
        return;
    }
    routes.push(CompiledSceneRoute {
        scene_id,
        frame_id: None,
        target_file,
        kind: "file_ref".to_string(),
        title: None,
        is_default: false,
        access_export,
    });
}

fn route_targets_preview(route: &CompiledSceneRoute, preview_target: Option<&str>) -> bool {
    let Some(preview) = preview_target
        .map(str::trim)
        .filter(|target| !target.is_empty())
    else {
        return false;
    };
    route.target_file == preview
}

fn route_matches_preview_scope(
    route: &CompiledSceneRoute,
    preview_target: Option<&str>,
    affected_targets: Option<&std::collections::BTreeSet<String>>,
) -> bool {
    if let Some(targets) = affected_targets {
        if !targets.is_empty() {
            return targets.contains(route.target_file.as_str());
        }
    }
    route_targets_preview(route, preview_target)
}

fn catalog_focus_target<'a>(
    options: &'a CompileOptions,
    active_target_file: Option<&'a str>,
) -> Option<&'a str> {
    options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .or_else(|| {
            options
                .scene
                .as_deref()
                .map(str::trim)
                .filter(|scene| !scene.is_empty())
                .and(active_target_file)
        })
}

fn manage_preview_target(options: &CompileOptions) -> Option<&str> {
    // scene-first：Manage 可同时带 scene 锚与 preview_target（source-focus）；
    // 单文件预览裁剪仍由 preview_target 决定，不得因 scene 已设置而退回全量 compile。
    options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty() && *target != "main.mei")
}

/// Manage 态打开 dataset 单文件预览时，只编译目标入口，避免扫全库 dataset 入口。
fn is_dataset_manage_preview(options: &CompileOptions) -> bool {
    manage_preview_target(options)
        .is_some_and(|preview| preview.starts_with("data/") || preview.contains("/datasets/"))
}

/// Manage 态按 `?file=scenes/...` 预览 widget/layout 等：只编译该入口 scene，不编译 home 与其它路由。
fn is_manage_entry_preview(options: &CompileOptions) -> bool {
    manage_preview_target(options).is_some()
}

fn is_manage_preview_only_compile(options: &CompileOptions) -> bool {
    is_dataset_manage_preview(options) || is_manage_entry_preview(options)
}

fn build_world_metric_ledger(
    resources: &[LoadedResource],
    world_metric_values: &[Value],
) -> Result<BTreeMap<String, WorldMetricLedgerEntry>> {
    let mut ledger = BTreeMap::new();
    let mut order = 0usize;
    for resource in resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        for (metric_id, metric) in &dataset.metrics {
            let id = metric_id.trim();
            if id.is_empty() {
                continue;
            }
            order += 1;
            ledger.insert(
                id.to_string(),
                WorldMetricLedgerEntry {
                    id: id.to_string(),
                    owner_resource_id: resource.id.clone(),
                    order,
                    metric: metric.clone(),
                },
            );
        }
    }
    let direct_metrics = materialize_world_metrics(resources, world_metric_values)?;
    for (metric_id, metric) in direct_metrics {
        order += 1;
        ledger.insert(
            metric_id.clone(),
            WorldMetricLedgerEntry {
                id: metric_id,
                owner_resource_id: "__world_metrics__".to_string(),
                order,
                metric,
            },
        );
    }
    Ok(ledger)
}

fn inject_discovered_entry_scene_routes(
    app_root: &Path,
    source_root: &Path,
    app_decls: &Value,
    asset_map: &BTreeMap<String, ComponentAsset>,
    routes: &mut Vec<CompiledSceneRoute>,
    scene_registry: &SceneRegistry,
    preview_target: Option<&str>,
    scene_selector: Option<&str>,
    preview_only: bool,
) {
    if let Some(preview) = preview_target.map(str::trim).filter(|s| !s.is_empty()) {
        if preview.ends_with(".mei") && !routes.iter().any(|r| r.target_file == preview) {
            let payload = compile_scene_payload_for_target(
                app_root,
                source_root,
                app_decls,
                asset_map,
                preview,
                None,
                scene_registry,
                None,
            );
            if let Some(contract) = payload.scene_contract.as_ref() {
                let sid = contract.scene.id.trim().to_string();
                if !sid.is_empty() {
                    try_push_discovered_entry_route(
                        routes,
                        sid,
                        preview.to_string(),
                        contract.scene.access_export,
                    );
                }
            }
        }
    }

    if preview_only {
        return;
    }

    let Some(requested) = scene_selector.map(str::trim).filter(|s| !s.is_empty()) else {
        return;
    };
    if find_scene_route(routes, requested).is_some() {
        return;
    }

    fn file_modified_ms(path: &Path) -> u128 {
        std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u128)
            .unwrap_or(0)
    }

    const MAX_MEI_PROBES: usize = 400;
    let mut mei_files: Vec<(String, u128)> = Vec::new();
    for entry in WalkDir::new(app_root).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        if e.depth() > 0 {
            if matches!(name.as_ref(), ".git" | "node_modules" | "target" | ".mei") {
                return false;
            }
        }
        true
    }) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("mei") {
            continue;
        }
        let Ok(rel) = path.strip_prefix(app_root) else {
            continue;
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if routes.iter().any(|r| r.target_file == rel_str) {
            continue;
        }
        let full: PathBuf = app_root.join(rel);
        mei_files.push((rel_str, file_modified_ms(&full)));
    }
    mei_files.sort_by(|a, b| b.1.cmp(&a.1));

    let mut probed = 0usize;
    for (rel_str, _) in mei_files {
        if probed >= MAX_MEI_PROBES {
            break;
        }
        probed += 1;
        let payload = compile_scene_payload_for_target(
            app_root,
            source_root,
            app_decls,
            asset_map,
            rel_str.as_str(),
            None,
            scene_registry,
            None,
        );
        let Some(contract) = payload.scene_contract.as_ref() else {
            continue;
        };
        if contract.scene.id.trim() != requested {
            continue;
        }
        try_push_discovered_entry_route(
            routes,
            contract.scene.id.trim().to_string(),
            rel_str,
            contract.scene.access_export,
        );
        break;
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    pub scene: Option<String>,
    pub preview_target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileWatchedFile {
    pub rel_path: String,
    pub modified_ms: u128,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct CompileRevisionPlan {
    pub token: String,
    pub watched_files: Vec<CompileWatchedFile>,
    pub components_revision: u128,
}

#[derive(Debug, Default, Clone, Copy)]
struct RoutePrecompileStats {
    attempted: usize,
    l2_hits: usize,
    l2_misses: usize,
    parallelism: usize,
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

fn route_compile_parallelism(max_jobs: usize) -> usize {
    if max_jobs == 0 {
        return 0;
    }
    let default_workers = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1)
        .clamp(1, 8);
    let configured = std::env::var("MEI_ROUTE_COMPILE_PARALLELISM")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_workers);
    configured.clamp(1, max_jobs)
}

fn resolve_active_route_meta(
    routes: &[CompiledSceneRoute],
    default_scene_id: Option<&str>,
    scene_selector: Option<&str>,
    preview_target: Option<&str>,
) -> (Option<CompiledSceneRoute>, bool) {
    if let Some(requested) = scene_selector {
        let requested = requested.trim();
        if requested.is_empty() {
            let selected = default_scene_id
                .and_then(|scene_id| find_scene_route(routes, scene_id))
                .cloned()
                .or_else(|| routes.first().cloned());
            return (selected, false);
        }
        let selected = find_scene_route(routes, requested).cloned();
        if selected.is_some() {
            return (selected, false);
        }
        let preview_route = preview_target
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .and_then(|target| routes.iter().find(|route| route.target_file == target))
            .cloned();
        let fallback = preview_route
            .clone()
            .or_else(|| {
                default_scene_id
                    .and_then(|scene_id| find_scene_route(routes, scene_id))
                    .cloned()
            })
            .or_else(|| routes.first().cloned());
        return (fallback, preview_route.is_none());
    }
    (
        default_scene_id
            .and_then(|scene_id| find_scene_route(routes, scene_id))
            .cloned()
            .or_else(|| routes.first().cloned()),
        false,
    )
}

fn precompile_route_payloads(
    app_root: &Path,
    source_root: &Path,
    app_decls: &Value,
    asset_map: &BTreeMap<String, ComponentAsset>,
    scene_registry: &SceneRegistry,
    dependency_graph: &DependencyGraph,
    routes: &[CompiledSceneRoute],
    official_results: &mut BTreeMap<String, CompiledScenePayload>,
) -> RoutePrecompileStats {
    if routes.is_empty() {
        return RoutePrecompileStats::default();
    }
    let parallelism = route_compile_parallelism(routes.len());
    if parallelism <= 1 || routes.len() <= 1 {
        let mut stats = RoutePrecompileStats {
            attempted: 0,
            l2_hits: 0,
            l2_misses: 0,
            parallelism: 1,
        };
        for route in routes {
            let dependency_fingerprint = dependency_graph.dependency_fingerprint_for_target(
                app_root,
                app_decls,
                route.target_file.as_str(),
            );
            let cache_hit = scene_payload_cache_has_entry(
                app_root,
                source_root,
                route.target_file.as_str(),
                dependency_fingerprint.as_deref(),
            );
            let payload = compile_scene_payload_for_target(
                app_root,
                source_root,
                app_decls,
                asset_map,
                route.target_file.as_str(),
                Some(route),
                scene_registry,
                dependency_fingerprint.as_deref(),
            );
            official_results.insert(route.scene_id.clone(), payload);
            stats.attempted += 1;
            if cache_hit {
                stats.l2_hits += 1;
            } else {
                stats.l2_misses += 1;
            }
        }
        return stats;
    }

    let queue = Arc::new(Mutex::new(VecDeque::from(routes.to_vec())));
    let output: Arc<Mutex<Vec<(CompiledSceneRoute, CompiledScenePayload, bool)>>> =
        Arc::new(Mutex::new(Vec::with_capacity(routes.len())));
    std::thread::scope(|scope| {
        for _ in 0..parallelism {
            let queue = Arc::clone(&queue);
            let output = Arc::clone(&output);
            scope.spawn(move || loop {
                let route = match queue.lock() {
                    Ok(mut guard) => guard.pop_front(),
                    Err(_) => None,
                };
                let Some(route) = route else {
                    break;
                };
                let dependency_fingerprint = dependency_graph.dependency_fingerprint_for_target(
                    app_root,
                    app_decls,
                    route.target_file.as_str(),
                );
                let cache_hit = scene_payload_cache_has_entry(
                    app_root,
                    source_root,
                    route.target_file.as_str(),
                    dependency_fingerprint.as_deref(),
                );
                let payload = compile_scene_payload_for_target(
                    app_root,
                    source_root,
                    app_decls,
                    asset_map,
                    route.target_file.as_str(),
                    Some(&route),
                    scene_registry,
                    dependency_fingerprint.as_deref(),
                );
                if let Ok(mut guard) = output.lock() {
                    guard.push((route, payload, cache_hit));
                }
            });
        }
    });

    let mut rows = output.lock().map(|guard| guard.clone()).unwrap_or_default();
    let route_order = routes
        .iter()
        .enumerate()
        .map(|(index, route)| ((route.scene_id.clone(), route.target_file.clone()), index))
        .collect::<BTreeMap<_, _>>();
    rows.sort_by_key(|(route, _, _)| {
        route_order
            .get(&(route.scene_id.clone(), route.target_file.clone()))
            .copied()
            .unwrap_or(usize::MAX)
    });
    let mut stats = RoutePrecompileStats {
        attempted: rows.len(),
        l2_hits: 0,
        l2_misses: 0,
        parallelism,
    };
    for (route, payload, cache_hit) in rows {
        official_results.insert(route.scene_id.clone(), payload);
        if cache_hit {
            stats.l2_hits += 1;
        } else {
            stats.l2_misses += 1;
        }
    }
    stats
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

pub fn resolve_default_scene_from_root(app_root: &Path) -> Result<Option<String>> {
    let app_main = app_root.join("main.mei");
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let route_registry = resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);
    Ok(route_registry
        .default_scene_id
        .or_else(|| {
            route_registry
                .routes
                .first()
                .map(|route| route.scene_id.clone())
        })
        .map(|scene_id| scene_id.trim().to_string())
        .filter(|scene_id| !scene_id.is_empty()))
}

pub fn compile_revision_plan_from_root_with_options(
    source_root: &Path,
    app_root: &Path,
    options: &CompileOptions,
) -> Result<CompileRevisionPlan> {
    let app_main = app_root.join("main.mei");
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let mut route_registry =
        resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);
    let asset_map = load_component_assets(source_root)?;
    let preview_only = is_manage_preview_only_compile(options);
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
    let dependency_graph =
        DependencyGraph::build_cached(app_root, &app_decls, &route_registry.routes);

    let active_route_meta = if let Some(requested) = options.scene.as_deref() {
        let selected = find_scene_route(&route_registry.routes, requested).cloned();
        if selected.is_none() {
            let preview_route = options
                .preview_target
                .as_deref()
                .map(str::trim)
                .filter(|target| !target.is_empty())
                .and_then(|target| {
                    route_registry
                        .routes
                        .iter()
                        .find(|route| route.target_file == target)
                        .cloned()
                });
            preview_route.or_else(|| {
                route_registry
                    .default_scene_id
                    .as_deref()
                    .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
                    .cloned()
                    .or_else(|| route_registry.routes.first().cloned())
            })
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
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(|value| value.to_string());
    let primary_target = selected_target
        .or_else(|| {
            active_route_meta
                .as_ref()
                .map(|route| route.target_file.clone())
        })
        .unwrap_or_else(|| "main.mei".to_string());

    let dataset_manage_preview = is_dataset_manage_preview(options);
    let catalog_focus = catalog_focus_target(options, Some(primary_target.as_str()));
    let catalog_filter = if dataset_manage_preview {
        DatasetCatalogFilter::default()
    } else {
        build_dataset_catalog_filter(app_root, &app_decls, &dependency_graph, catalog_focus)
    };

    let mut token_parts = BTreeMap::<String, String>::new();
    let mut watched_paths = BTreeSet::<String>::new();
    watched_paths.insert("main.mei".to_string());
    if let Some(main_token) =
        dependency_graph.dependency_fingerprint_for_target(app_root, &app_decls, "main.mei")
    {
        token_parts.insert("main".to_string(), main_token);
        watched_paths.extend(dependency_graph.closure_for_target(app_root, &app_decls, "main.mei"));
    }
    if let Some(primary_token) =
        dependency_graph.dependency_fingerprint_for_target(app_root, &app_decls, &primary_target)
    {
        token_parts.insert(format!("target:{primary_target}"), primary_token);
        watched_paths.extend(dependency_graph.closure_for_target(
            app_root,
            &app_decls,
            &primary_target,
        ));
    }
    if !dataset_manage_preview {
        for rel in catalog::resolve_dataset_catalog_compile_rels(app_root, &catalog_filter) {
            if let Some(token) =
                dependency_graph.dependency_fingerprint_for_target(app_root, &app_decls, &rel)
            {
                token_parts.insert(format!("catalog:{rel}"), token);
                watched_paths
                    .extend(dependency_graph.closure_for_target(app_root, &app_decls, &rel));
            }
        }
    }

    let components_revision = scene_payload_cache::components_revision(source_root);
    token_parts.insert("components".to_string(), components_revision.to_string());
    let watched_files = watched_paths
        .into_iter()
        .map(|rel_path| {
            let path = app_root.join(&rel_path);
            let metadata = std::fs::metadata(&path).ok();
            CompileWatchedFile {
                rel_path,
                modified_ms: scene_payload_cache::file_mtime_ms(&path),
                size_bytes: metadata.map(|meta| meta.len()).unwrap_or(0),
            }
        })
        .collect();
    Ok(CompileRevisionPlan {
        token: token_parts.into_values().collect::<Vec<_>>().join("||"),
        watched_files,
        components_revision,
    })
}

pub fn compile_revision_token_from_root_with_options(
    source_root: &Path,
    app_root: &Path,
    options: &CompileOptions,
) -> Result<String> {
    Ok(compile_revision_plan_from_root_with_options(source_root, app_root, options)?.token)
}

pub fn compile_app_from_root_with_options(
    source_root: &Path,
    app_root: &Path,
    options: CompileOptions,
) -> Result<CompiledApp> {
    let (l2_hits_before, l2_misses_before) = scene_payload_cache_metrics_snapshot();
    let (l3_hits_before, l3_misses_before) = dataset_materialize_cache_metrics_snapshot();
    let (catalog_index_hits_before, catalog_index_misses_before) =
        dataset_catalog_index_cache_metrics_snapshot();
    let (decl_file_hits_before, decl_file_misses_before) = decl_file_cache_metrics_snapshot();
    let (graph_cache_hits_before, graph_cache_misses_before) =
        dependency_graph_cache_metrics_snapshot();
    let (content_hash_hits_before, content_hash_misses_before) =
        file_content_hash_cache_metrics_snapshot();
    let app_main = app_root.join("main.mei");
    let app_decls = evaluate_mei_file(&app_main)?;
    let (app_decl, mut diagnostics) = decode_app_decl(&app_main, &app_decls);
    let app_decl =
        app_decl.ok_or_else(|| anyhow!("{} missing app(...) declaration", app_main.display()))?;
    let mut route_registry =
        resolve_scene_routes(&app_main, &app_decl, &app_decls, &mut diagnostics);

    let asset_map = load_component_assets(source_root)?;
    let preview_only = is_manage_preview_only_compile(&options);
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
    let mut official_results: BTreeMap<String, CompiledScenePayload> = BTreeMap::new();
    let mut precompile_routes = Vec::<CompiledSceneRoute>::new();
    if preview_only {
        for route in &route_registry.routes {
            if route_matches_preview_scope(
                route,
                options.preview_target.as_deref(),
                preview_affected_targets.as_ref(),
            ) {
                precompile_routes.push(route.clone());
            }
        }
    } else {
        let mut route_by_target = BTreeMap::<String, CompiledSceneRoute>::new();
        if let Some(route) = active_route_meta.as_ref() {
            route_by_target.insert(route.target_file.clone(), route.clone());
        }
        if let Some(default_route) = route_registry
            .default_scene_id
            .as_deref()
            .and_then(|scene_id| find_scene_route(&route_registry.routes, scene_id))
        {
            route_by_target.insert(default_route.target_file.clone(), default_route.clone());
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
        &app_decls,
        &asset_map,
        &scene_registry,
        &dependency_graph,
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
    let active_payload_pick_started = Instant::now();
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
                    let dependency_fingerprint = dependency_graph
                        .dependency_fingerprint_for_target(
                            app_root,
                            &app_decls,
                            target_file.as_str(),
                        );
                    compile_scene_payload_for_target(
                        app_root,
                        source_root,
                        &app_decls,
                        &asset_map,
                        target_file.as_str(),
                        Some(&scene_route),
                        &scene_registry,
                        dependency_fingerprint.as_deref(),
                    )
                });
            (Some(scene_route.scene_id), target_file, payload)
        } else {
            let dependency_fingerprint = dependency_graph.dependency_fingerprint_for_target(
                app_root,
                &app_decls,
                target_file.as_str(),
            );
            let payload = compile_scene_payload_for_target(
                app_root,
                source_root,
                &app_decls,
                &asset_map,
                target_file.as_str(),
                None,
                &scene_registry,
                dependency_fingerprint.as_deref(),
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
                            let dependency_fingerprint = dependency_graph
                                .dependency_fingerprint_for_target(
                                    app_root,
                                    &app_decls,
                                    route_meta.target_file.as_str(),
                                );
                            compile_scene_payload_for_target(
                                app_root,
                                source_root,
                                &app_decls,
                                &asset_map,
                                route_meta.target_file.as_str(),
                                Some(&route_meta),
                                &scene_registry,
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
                    &app_decls,
                    route_meta.target_file.as_str(),
                );
                compile_scene_payload_for_target(
                    app_root,
                    source_root,
                    &app_decls,
                    &asset_map,
                    route_meta.target_file.as_str(),
                    Some(&route_meta),
                    &scene_registry,
                    dependency_fingerprint.as_deref(),
                )
            });
        (Some(route_meta.scene_id), route_meta.target_file, payload)
    } else {
        let dependency_fingerprint =
            dependency_graph.dependency_fingerprint_for_target(app_root, &app_decls, "main.mei");
        (
            None,
            "main.mei".to_string(),
            compile_scene_payload_for_target(
                app_root,
                source_root,
                &app_decls,
                &asset_map,
                "main.mei",
                None,
                &scene_registry,
                dependency_fingerprint.as_deref(),
            ),
        )
    };
    let active_payload_pick_or_compile_ms = elapsed_ms(active_payload_pick_started);

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

    let dataset_manage_preview = is_dataset_manage_preview(&options);
    let catalog_focus = catalog_focus_target(&options, Some(active_target_file.as_str()));
    let catalog_seed_files =
        dependency_graph.catalog_seed_files(app_root, &app_decls, catalog_focus);
    let catalog_filter = if dataset_manage_preview {
        DatasetCatalogFilter::default()
    } else {
        build_dataset_catalog_filter(app_root, &app_decls, &dependency_graph, catalog_focus)
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
            catalog::resolve_dataset_catalog_compile_rels(app_root, &catalog_filter).len();
        catalog_parallelism = catalog_compile_parallelism(catalog_compile_rels);
        let out = compile_dataset_catalog_resources(
            app_root,
            source_root,
            &app_decls,
            &asset_map,
            &catalog_filter,
            &dependency_graph,
        );
        catalog_compile_ms = elapsed_ms(catalog_started);
        let l2_after_catalog = scene_payload_cache_metrics_snapshot();
        catalog_l2_hit_delta = l2_after_catalog.0.saturating_sub(l2_before_catalog.0);
        catalog_l2_miss_delta = l2_after_catalog.1.saturating_sub(l2_before_catalog.1);
        out
    };
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "catalog_compile_stats".to_string(),
        message: format!(
            "dataset_manage_preview={}, compile_rels={}, parallelism={}, l2_hits_delta={}, l2_misses_delta={}, catalog_compile_ms={}",
            dataset_manage_preview,
            catalog_compile_rels,
            catalog_parallelism,
            catalog_l2_hit_delta,
            catalog_l2_miss_delta,
            catalog_compile_ms
        ),
        source_path: Some(app_main.to_string_lossy().to_string()),
    });
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "catalog_parallelism_eval".to_string(),
        message: if dataset_manage_preview {
            "decision=skip_preview_scope".to_string()
        } else if catalog_compile_rels >= 8 && catalog_compile_ms >= 120 {
            format!(
                "decision=candidate, reason=high_catalog_cost, compile_rels={}, parallelism={}, catalog_compile_ms={}",
                catalog_compile_rels, catalog_parallelism, catalog_compile_ms
            )
        } else {
            format!(
                "decision=defer, reason=low_catalog_cost, compile_rels={}, parallelism={}, catalog_compile_ms={}",
                catalog_compile_rels, catalog_parallelism, catalog_compile_ms
            )
        },
        source_path: Some(app_main.to_string_lossy().to_string()),
    });
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
            active_target_file.as_str(),
            &mut diagnostics,
        );
    }

    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "compile_stage_timing".to_string(),
        message: format!(
            "dependency_graph_build_ms={}, official_results_all_routes_ms={}, active_payload_pick_or_compile_ms={}, catalog_compile_ms={}, resource_merge_ms={}, world_metric_ledger_ms={}",
            dependency_graph_build_ms,
            official_results_all_routes_ms,
            active_payload_pick_or_compile_ms,
            catalog_compile_ms,
            resource_merge_ms,
            world_metric_ledger_ms
        ),
        source_path: Some(app_main.to_string_lossy().to_string()),
    });
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
        source_path: Some(app_main.to_string_lossy().to_string()),
    });

    let active_shard = shards::build_scene_payload_shard(
        active_target_file.as_str(),
        active_scene.as_deref(),
        &active_payload,
    );
    let dataset_shard = shards::build_dataset_materialization_shard(
        "__catalog__",
        &resources
            .iter()
            .filter(|resource| resource.dataset.is_some())
            .cloned()
            .collect::<Vec<_>>(),
    );
    let imported_scope_shards = shards::collect_imported_scope_shards(&resources);
    let graph_stats = dependency_graph.stats();
    let preview_scope_size = preview_affected_targets
        .as_ref()
        .map(std::collections::BTreeSet::len)
        .unwrap_or(0);
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
        source_path: Some(app_main.to_string_lossy().to_string()),
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
        source_path: Some(app_main.to_string_lossy().to_string()),
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
        source_path: Some(app_main.to_string_lossy().to_string()),
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
                l2_hits_after.saturating_sub(l2_hits_before),
                l2_misses_after.saturating_sub(l2_misses_before),
                l3_hits_after.saturating_sub(l3_hits_before),
                l3_misses_after.saturating_sub(l3_misses_before),
                catalog_index_hits_after.saturating_sub(catalog_index_hits_before),
                catalog_index_misses_after.saturating_sub(catalog_index_misses_before),
                decl_file_hits_after.saturating_sub(decl_file_hits_before),
                decl_file_misses_after.saturating_sub(decl_file_misses_before),
                graph_cache_hits_after.saturating_sub(graph_cache_hits_before),
                graph_cache_misses_after.saturating_sub(graph_cache_misses_before),
                content_hash_hits_after.saturating_sub(content_hash_hits_before),
                content_hash_misses_after.saturating_sub(content_hash_misses_before),
            )
        },
        source_path: Some(app_main.to_string_lossy().to_string()),
    });
    diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "compile_optimization_status".to_string(),
        message: format!(
            "dependency_graph=on,preview_scope=on,l2=on,l3=on,catalog_index=on,content_hash=on,graph_cache_delta={},catalog_index_cache_delta={},content_hash_cache_delta={}",
            dependency_graph_cache_metrics_snapshot()
                .0
                .saturating_sub(graph_cache_hits_before)
                + dependency_graph_cache_metrics_snapshot()
                    .1
                    .saturating_sub(graph_cache_misses_before),
            dataset_catalog_index_cache_metrics_snapshot()
                .0
                .saturating_sub(catalog_index_hits_before)
                + dataset_catalog_index_cache_metrics_snapshot()
                    .1
                    .saturating_sub(catalog_index_misses_before),
            file_content_hash_cache_metrics_snapshot()
                .0
                .saturating_sub(content_hash_hits_before)
                + file_content_hash_cache_metrics_snapshot()
                    .1
                    .saturating_sub(content_hash_misses_before),
        ),
        source_path: Some(app_main.to_string_lossy().to_string()),
    });

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
        let mut assembly = serde_json::Map::new();
        assembly.insert(
            "scene_id".to_string(),
            Value::String(route.scene_id.clone()),
        );
        assembly.insert(
            "target_file".to_string(),
            Value::String(route.target_file.clone()),
        );
        assembly.insert("kind".to_string(), Value::String(route.kind.clone()));
        if let Some(title) = route
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            assembly.insert("title".to_string(), Value::String(title.to_string()));
        }
        if !contract.scene.bindings.is_null() {
            scene_bindings_by_id.insert(route.scene_id.clone(), contract.scene.bindings.clone());
            assembly.insert("bindings".to_string(), contract.scene.bindings.clone());
        }
        if !contract.scene.examples.is_null() {
            scene_examples_by_id.insert(route.scene_id.clone(), contract.scene.examples.clone());
            assembly.insert("examples".to_string(), contract.scene.examples.clone());
        }
        if !contract.scene.local_nav.is_null() {
            scene_local_nav_by_target
                .insert(route.target_file.clone(), contract.scene.local_nav.clone());
            assembly.insert("local_nav".to_string(), contract.scene.local_nav.clone());
        }
        scene_projection_assembly_by_id.insert(route.scene_id.clone(), Value::Object(assembly));
    }
    if let Some(contract) = active_payload.scene_contract.as_ref() {
        if let Some(active_scene_id) = active_scene.as_deref() {
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
                        Value::String(active_target_file.clone()),
                    );
                    Value::Object(assembly)
                });
            if let Some(assembly_map) = assembly_entry.as_object_mut() {
                assembly_map.insert(
                    "target_file".to_string(),
                    Value::String(active_target_file.clone()),
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
            scene_local_nav_by_target
                .insert(active_target_file.clone(), contract.scene.local_nav.clone());
        }
    }

    Ok(CompiledApp {
        app_id: app_decl.id.clone(),
        title,
        app_root: app_root.to_string_lossy().to_string(),
        scene_routes: route_registry.routes,
        active_scene,
        active_target_file,
        file_tree: source_tree(app_root)?,
        scene_contract: active_payload.scene_contract,
        scene_local_nav_by_target,
        scene_bindings_by_id,
        scene_examples_by_id,
        scene_projection_assembly_by_id,
        resources,
        world_metrics,
        component_assets: active_payload.component_assets,
        diagnostics,
    })
}

pub use materialize_cache::cached_load_xlsx_table_snapshot;
pub use materialize_cache::dataset_materialize_cache_epoch;
pub use materialize_cache::try_get_cached_xlsx_table_snapshot;
pub use materialize_cache::TableSnapshot;
pub use materialize_cache::TableSnapshotKey;
pub use panel_normalize::panel_resolved_has_head;
pub use scene_payload_cache::scene_payload_cache_epoch;

pub fn clear_runtime_compile_caches() {
    clear_materialize_cache();
    clear_scene_payload_cache();
    clear_dataset_catalog_index_cache();
    clear_decl_file_cache();
    clear_dependency_graph_cache();
    clear_file_content_hash_cache();
    clear_runtime_eval_node_cache();
}

pub fn clear_runtime_eval_node_cache() -> usize {
    analysis::eval_context::clear_eval_node_cache()
}

#[cfg(test)]
pub(crate) use materialize_cache::{
    clear_materialize_cache_for_tests, legacy_rows_cache_len_for_tests,
};
#[cfg(test)]
pub(crate) use scene_payload_cache::{
    clear_scene_payload_cache_for_tests, scene_payload_cache_len_for_tests,
};

pub fn evaluate_runtime_metric_defs(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, crate::model::DatasetView>,
    metric_ids: Option<&[String]>,
) -> Result<BTreeMap<String, crate::model::MetricContract>> {
    materialize::evaluate_runtime_metric_defs(metric_defs, base_rows, datasets, metric_ids)
}

pub fn evaluate_runtime_metric_defs_with_scope(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, crate::model::DatasetView>,
    metric_ids: Option<&[String]>,
    scope: &analysis::eval_context::RuntimeMetricEvalScope,
) -> Result<BTreeMap<String, crate::model::MetricContract>> {
    materialize::evaluate_runtime_metric_defs_with_scope(
        metric_defs,
        base_rows,
        datasets,
        metric_ids,
        scope,
    )
}

pub fn evaluate_runtime_metric_defs_with_scope_and_dag(
    metric_defs: &BTreeMap<String, Value>,
    base_rows: &[Value],
    datasets: &BTreeMap<String, crate::model::DatasetView>,
    metric_ids: Option<&[String]>,
    scope: &analysis::eval_context::RuntimeMetricEvalScope,
) -> Result<(
    BTreeMap<String, crate::model::MetricContract>,
    materialize::RuntimeMetricEvalReport,
)> {
    materialize::evaluate_runtime_metric_defs_with_scope_and_dag(
        metric_defs,
        base_rows,
        datasets,
        metric_ids,
        scope,
    )
}

pub fn build_runtime_analysis_graph(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> crate::model::AnalysisGraph {
    materialize::build_analysis_graph(metric_defs, root_dataset_id)
}

pub fn build_runtime_analysis_contracts(
    metric_defs: &BTreeMap<String, Value>,
    root_dataset_id: &str,
) -> BTreeMap<String, Value> {
    materialize::build_analysis_contracts(metric_defs, root_dataset_id)
}

pub fn runtime_analysis_closure_metric_ids(
    graph: &crate::model::AnalysisGraph,
    focus_ids: &[String],
) -> Vec<String> {
    materialize::analysis_closure_metric_ids(graph, focus_ids)
}

pub use analysis::eval_context::{
    runtime_eval_node_cache_enabled, RequestDagMetrics, RuntimeMetricEvalScope,
};
pub use materialize::{
    capsule_path_from_namespaced_resource_id,
    imported_capsule_path_from_world_metrics_resource_id, local_dataset_id_from_namespaced_token,
    resolve_runtime_metric_def_key, EvalPlan, EvalPlanEdge, EvalPlanEdgeKind, EvalPlanNode,
    EvalPlanNodeKind, EvalPlanScope, RuntimeMetricEvalReport,
};

#[cfg(test)]
mod tests;
