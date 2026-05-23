mod merge;
mod scan;

use std::{
    collections::BTreeMap,
    path::Path,
};

use serde_json::Value;

use crate::model::{ComponentAsset, LoadedResource};
use crate::typed_refs::SceneRegistry;

use super::scene_payload_cache::compile_scene_payload_for_target;

pub use scan::{build_dataset_catalog_filter, DatasetCatalogFilter};
use merge::upsert_catalog_dataset_resource;
use scan::{
    build_dataset_id_to_scene_file_map, collect_dataset_catalog_mei_files, dataset_file_matches_filter,
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
) -> Vec<LoadedResource> {
    if !filter.is_active() {
        return Vec::new();
    }

    let mut by_id = BTreeMap::<String, LoadedResource>::new();

    let mut compile_rels: Vec<String> = collect_dataset_catalog_mei_files(app_root);
    for rel in &filter.dataset_paths {
        if rel.ends_with(".mei") && !compile_rels.iter().any(|r| r == rel) {
            compile_rels.push(rel.clone());
        }
    }
    let dataset_scene_files = build_dataset_id_to_scene_file_map(app_root);
    for id in &filter.resource_ids {
        if let Some(rel) = dataset_scene_files.get(id) {
            if !compile_rels.iter().any(|r| r == rel) {
                compile_rels.push(rel.clone());
            }
        }
    }
    if compile_rels.is_empty() {
        return Vec::new();
    }

    for rel in compile_rels {
        if !dataset_file_matches_filter(app_root, rel.as_str(), filter) {
            continue;
        }
        let payload = compile_scene_payload_for_target(
            app_root,
            source_root,
            app_decls,
            asset_map,
            rel.as_str(),
            None,
            &SceneRegistry::new(),
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
