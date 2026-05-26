mod merge;
mod scan;

use std::{collections::BTreeMap, path::Path};

use serde_json::Value;

use crate::model::{ComponentAsset, LoadedResource};
use crate::typed_refs::SceneRegistry;

use super::dependency_graph::DependencyGraph;
use super::scene_payload_cache::compile_scene_payload_for_target;

use merge::upsert_catalog_dataset_resource;
#[cfg(test)]
pub(crate) use scan::clear_dataset_catalog_index_cache_for_tests;
use scan::resolve_dataset_catalog_compile_rels;
pub use scan::{build_dataset_catalog_filter, DatasetCatalogFilter};
pub(crate) use scan::{
    dataset_catalog_index_cache_metrics_snapshot, extract_from_dataset_tokens,
    extract_metric_ref_tokens,
};

/// 收集 dataset 声明 `.mei`（`data/dataset/**` 或 `scenes/**`），供驾驶舱 panel 等跨入口 `metric_ref` 解析。
///
/// **硬约束**：仅当 `filter.is_active()` 且路径命中过滤器时才物化；绝不因 `filter == None` 或空过滤器而扫全库。
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

    let mut by_id = BTreeMap::<String, LoadedResource>::new();

    let compile_rels = resolve_dataset_catalog_compile_rels(app_root, filter);
    if compile_rels.is_empty() {
        return Vec::new();
    }

    for rel in compile_rels {
        let dependency_fingerprint =
            dependency_graph.dependency_fingerprint_for_target(app_root, app_decls, rel.as_str());
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
        let mut dataset_resources = Vec::new();
        for resource in payload.resources {
            if resource.dataset.is_some() {
                dataset_resources.push(resource);
            }
        }
        for resource in dataset_resources {
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
