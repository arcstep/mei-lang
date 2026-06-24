use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mei_lang_kernel::{
    load_mei_config_for_app, resolve_app_data_generation, CompiledApp, RuntimeConfig,
};

use crate::metric_hydrate::collect_dataset_ids_from_metric_defs;
use crate::metric_cache_key::lookup_compiled_dataset_view;
use crate::metric_response_cache::{
    metric_response_prebuild_query_tail, metric_response_prebuild_shared_key,
};
use crate::types::DatasetQueryOptions;

pub fn resolve_metric_data_generation(
    app_root: &Path,
    app_id: &str,
    compiled: &CompiledApp,
    owner_dataset_id: &str,
    metric_defs: &BTreeMap<String, serde_json::Value>,
) -> String {
    let runtime = load_mei_config_for_app(app_root, None).runtime;
    resolve_metric_data_generation_with_runtime(
        app_root,
        app_id,
        compiled,
        owner_dataset_id,
        metric_defs,
        &runtime,
    )
}

pub fn resolve_metric_data_generation_with_runtime(
    app_root: &Path,
    app_id: &str,
    compiled: &CompiledApp,
    owner_dataset_id: &str,
    metric_defs: &BTreeMap<String, serde_json::Value>,
    runtime: &RuntimeConfig,
) -> String {
    let mut dataset_ids = collect_dataset_ids_from_metric_defs(metric_defs);
    let owner = owner_dataset_id.trim();
    if !owner.is_empty() {
        dataset_ids.insert(owner.to_string());
    }
    let datasets = dataset_ids
        .iter()
        .filter_map(|dataset_id| lookup_compiled_dataset_view(compiled, dataset_id.as_str()))
        .collect::<Vec<_>>();
    resolve_app_data_generation(app_root, app_id, &datasets, runtime)
}

pub fn canonical_metric_shared_cache_key(
    app_id: &str,
    data_generation: &str,
    owner_dataset_id: &str,
    query: &DatasetQueryOptions,
) -> String {
    format!(
        "idempotent|response|app={app_id}|data_gen={data_generation}|dataset={owner_dataset_id}|{}",
        metric_response_prebuild_query_tail(query)
    )
}

pub fn canonical_metric_idempotency_key(
    app_id: &str,
    data_generation: &str,
    scene_id: &str,
    scene_path: Option<&str>,
    owner_dataset_id: &str,
    metric_ids: &BTreeSet<String>,
    query: &DatasetQueryOptions,
) -> String {
    let mut ids = metric_ids.iter().cloned().collect::<Vec<_>>();
    ids.sort();
    let target = scene_path.unwrap_or("").trim();
    format!(
        "idempotent|app={app_id}|data_gen={data_generation}|scene={scene_id}|target={target}|dataset={owner_dataset_id}|metrics={}|{}",
        serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string()),
        metric_response_prebuild_query_tail(query)
    )
}

pub fn metric_shared_cache_key_with_data_generation(
    app_id: &str,
    data_generation: &str,
    owner_dataset_id: &str,
    query: &DatasetQueryOptions,
    legacy_dependency_revision_key: &str,
) -> String {
    let data_gen = data_generation.trim();
    if data_gen.is_empty() {
        return metric_response_prebuild_shared_key(
            app_id,
            owner_dataset_id,
            query,
            legacy_dependency_revision_key,
        );
    }
    canonical_metric_shared_cache_key(app_id, data_gen, owner_dataset_id, query)
}
