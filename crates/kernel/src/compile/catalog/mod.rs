mod merge;
mod scan;

use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
    sync::{Arc, Mutex},
};

use serde_json::Value;

use crate::model::{ComponentAsset, LoadedResource};
use crate::typed_refs::SceneRegistry;

use super::dependency_graph::DependencyGraph;
use super::authoring_eval::{
    install_shared_authoring_guard, shared_authoring_helpers_for_compile,
};
use super::scene_payload_cache::compile_scene_payload_for_target;

use merge::upsert_catalog_dataset_resource;
pub(crate) use scan::clear_dataset_catalog_index_cache;
#[cfg(test)]
pub(crate) use scan::clear_dataset_catalog_index_cache_for_tests;
pub(super) use scan::resolve_dataset_catalog_compile_rels;
pub use scan::{build_dataset_catalog_filter, DatasetCatalogFilter};
pub(crate) use scan::{
    dataset_catalog_index_cache_metrics_snapshot, extract_from_dataset_tokens,
    extract_metric_ref_tokens,
};

pub(super) fn catalog_compile_parallelism(max_jobs: usize) -> usize {
    if max_jobs == 0 {
        return 0;
    }
    let default_workers = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1)
        .clamp(1, 8);
    let configured = std::env::var("MEI_CATALOG_COMPILE_PARALLELISM")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default_workers);
    configured.clamp(1, max_jobs)
}

/// 收集 dataset 声明 `.mei`（`data/dataset/**` 或 `scenes/**`），供驾驶舱 panel 等跨入口 `metric_ref` 解析。
///
/// **硬约束**：仅当 `filter.is_active()` 且路径命中过滤器时才物化；绝不因 `filter == None` 或空过滤器而扫全库。
#[cfg(test)]
pub(super) fn compile_dataset_catalog_resources(
    app_root: &Path,
    source_root: &Path,
    app_decls: &Value,
    asset_map: &BTreeMap<String, ComponentAsset>,
    filter: &DatasetCatalogFilter,
    dependency_graph: &DependencyGraph,
) -> Vec<LoadedResource> {
    if !filter.is_active() {
        return Vec::new();
    }

    let compile_rels = resolve_dataset_catalog_compile_rels(app_root, filter);
    compile_dataset_catalog_resources_for_rels(
        app_root,
        source_root,
        app_decls,
        asset_map,
        dependency_graph,
        compile_rels,
    )
}

pub(super) fn compile_dataset_catalog_resources_for_rels(
    app_root: &Path,
    source_root: &Path,
    app_decls: &Value,
    asset_map: &BTreeMap<String, ComponentAsset>,
    dependency_graph: &DependencyGraph,
    compile_rels: Vec<String>,
) -> Vec<LoadedResource> {
    if compile_rels.is_empty() {
        return Vec::new();
    }
    let authoring_helpers = shared_authoring_helpers_for_compile(source_root);
    let parallelism = catalog_compile_parallelism(compile_rels.len());

    let mut compiled = Vec::<(String, Vec<LoadedResource>)>::new();
    if parallelism <= 1 || compile_rels.len() <= 1 {
        for rel in compile_rels {
            let dependency_fingerprint = dependency_graph.dependency_fingerprint_for_target(
                app_root,
                app_decls,
                rel.as_str(),
            );
            let payload = compile_scene_payload_for_target(
                app_root,
                source_root,
                app_decls,
                asset_map,
                rel.as_str(),
                None,
                &SceneRegistry::new(),
                dependency_fingerprint.as_deref(),
            );
            let dataset_resources = payload
                .resources
                .into_iter()
                .filter(|resource| resource.dataset.is_some())
                .collect::<Vec<_>>();
            compiled.push((rel, dataset_resources));
        }
    } else {
        let queue = Arc::new(Mutex::new(VecDeque::from(compile_rels)));
        let output = Arc::new(Mutex::new(Vec::<(String, Vec<LoadedResource>)>::new()));
        std::thread::scope(|scope| {
            for _ in 0..parallelism {
                let queue = Arc::clone(&queue);
                let output = Arc::clone(&output);
                let authoring_helpers = Arc::clone(&authoring_helpers);
                scope.spawn(move || {
                    let _authoring_guard =
                        install_shared_authoring_guard(authoring_helpers.as_ref());
                    loop {
                    let rel = match queue.lock() {
                        Ok(mut guard) => guard.pop_front(),
                        Err(_) => None,
                    };
                    let Some(rel) = rel else { break };
                    let dependency_fingerprint = dependency_graph
                        .dependency_fingerprint_for_target(app_root, app_decls, rel.as_str());
                    let payload = compile_scene_payload_for_target(
                        app_root,
                        source_root,
                        app_decls,
                        asset_map,
                        rel.as_str(),
                        None,
                        &SceneRegistry::new(),
                        dependency_fingerprint.as_deref(),
                    );
                    let dataset_resources = payload
                        .resources
                        .into_iter()
                        .filter(|resource| resource.dataset.is_some())
                        .collect::<Vec<_>>();
                    if let Ok(mut guard) = output.lock() {
                        guard.push((rel, dataset_resources));
                    }
                    }
                });
            }
        });
        compiled = output.lock().map(|guard| guard.clone()).unwrap_or_default();
    }

    compiled.sort_by(|left, right| left.0.cmp(&right.0));
    let mut by_id = BTreeMap::<String, LoadedResource>::new();
    for (_rel, resources) in compiled {
        for resource in resources {
            upsert_catalog_dataset_resource(&mut by_id, resource);
        }
    }

    by_id.into_values().collect()
}

pub(super) fn merge_resource_catalog(
    catalog: Vec<LoadedResource>,
    scene_resources: Vec<LoadedResource>,
) -> Vec<LoadedResource> {
    let mut by_id = BTreeMap::<String, LoadedResource>::new();
    for resource in catalog {
        upsert_catalog_dataset_resource(&mut by_id, resource);
    }
    for resource in scene_resources {
        upsert_catalog_dataset_resource(&mut by_id, resource);
    }
    by_id.into_values().collect()
}

#[cfg(test)]
mod tests;
