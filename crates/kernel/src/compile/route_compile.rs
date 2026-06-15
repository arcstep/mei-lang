use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::Value;

use crate::model::{CompiledSceneRoute, ComponentAsset};
use crate::typed_refs::SceneRegistry;

use super::authoring_eval::{
    install_shared_authoring_guard, shared_authoring_helpers_for_compile,
};
use super::dependency_graph::DependencyGraph;
use super::entry_payload::CompiledScenePayload;
use super::scene::find_scene_route;
use super::scene_payload_cache::{compile_scene_payload_for_target, scene_payload_cache_has_entry};

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct RoutePrecompileStats {
    pub(super) attempted: usize,
    pub(super) l2_hits: usize,
    pub(super) l2_misses: usize,
    pub(super) parallelism: usize,
}

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

pub(super) fn route_compile_parallelism(max_jobs: usize) -> usize {
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

pub(super) fn resolve_active_route_meta(
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

pub(super) fn precompile_route_payloads(
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
    let authoring_helpers = shared_authoring_helpers_for_compile(source_root);
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
                Some(route.scene_id.as_str()),
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
            let authoring_helpers = Arc::clone(&authoring_helpers);
            scope.spawn(move || {
                let _authoring_guard =
                    install_shared_authoring_guard(authoring_helpers.as_ref());
                loop {
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
                    Some(route.scene_id.as_str()),
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
