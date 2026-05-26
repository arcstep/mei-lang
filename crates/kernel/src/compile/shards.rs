use std::collections::BTreeMap;

use crate::model::{ComponentAsset, LoadedResource, SceneContract};

use super::entry_payload::CompiledScenePayload;

#[derive(Debug, Clone)]
pub(crate) struct ScenePayloadShard {
    pub target_file: String,
    pub scene_id: Option<String>,
    pub scene_contract: Option<SceneContract>,
    pub resources: Vec<LoadedResource>,
    pub component_assets: Vec<ComponentAsset>,
}

#[derive(Debug, Clone)]
pub(crate) struct DatasetMaterializationShard {
    pub dataset_file: String,
    pub resources: Vec<LoadedResource>,
}

#[derive(Debug, Clone)]
pub(crate) struct ImportedScopeShard {
    pub import_scope: String,
    pub resources: Vec<LoadedResource>,
}

pub(crate) fn build_scene_payload_shard(
    target_file: &str,
    scene_id: Option<&str>,
    payload: &CompiledScenePayload,
) -> ScenePayloadShard {
    ScenePayloadShard {
        target_file: target_file.to_string(),
        scene_id: scene_id.map(str::to_string),
        scene_contract: payload.scene_contract.clone(),
        resources: payload.resources.clone(),
        component_assets: payload.component_assets.clone(),
    }
}

pub(crate) fn build_dataset_materialization_shard(
    dataset_file: &str,
    resources: &[LoadedResource],
) -> DatasetMaterializationShard {
    DatasetMaterializationShard {
        dataset_file: dataset_file.to_string(),
        resources: resources.to_vec(),
    }
}

pub(crate) fn collect_imported_scope_shards(
    resources: &[LoadedResource],
) -> Vec<ImportedScopeShard> {
    let mut by_scope = BTreeMap::<String, Vec<LoadedResource>>::new();
    for resource in resources {
        let Some((scope, _local_id)) = resource.id.split_once("::") else {
            continue;
        };
        let scope = scope.trim();
        if !scope.ends_with(".mei") {
            continue;
        }
        by_scope
            .entry(scope.to_string())
            .or_default()
            .push(resource.clone());
    }
    by_scope
        .into_iter()
        .map(|(import_scope, resources)| ImportedScopeShard {
            import_scope,
            resources,
        })
        .collect()
}
