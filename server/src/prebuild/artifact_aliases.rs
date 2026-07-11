use super::prelude::*;
use super::*;

pub(crate) fn metric_response_artifact_covers_request(
    artifact: &mei_lang_datasets::LoadedMetricResponseArtifact,
    covered_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> bool {
    if request_all_metrics {
        artifact.complete
    } else {
        covered_metric_ids
            .iter()
            .all(|metric_id| artifact.covered_metric_ids.contains(metric_id))
    }
}

pub(crate) fn materialize_metric_response_sibling_aliases(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    owner_resource: &LoadedResource,
    artifact: &LoadedMetricResponseArtifact,
    query: &DatasetQueryOptions,
    metric_defs: &BTreeMap<String, Value>,
    state: &CoverageState,
) -> Result<()> {
    let owner_dataset = owner_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
    let identity = dataset_metric_identity_key(owner_dataset);
    for resource in &outcome.compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if dataset_metric_identity_key(dataset) != identity {
            continue;
        }
        let (scene_id, scene_path) =
            artifact_scene_context_for_resource(&outcome.compiled, resource.id.as_str());
        let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
            app_root,
            &outcome.compiled,
            resource.id.as_str(),
            metric_defs,
        );
        let mut alias_keys = Vec::new();
        let mut seen_alias_keys = BTreeSet::new();
        let mut push_alias_key = |scene_path: Option<&str>, response_cache_key: String| {
            if seen_alias_keys.insert(response_cache_key.clone()) {
                alias_keys.push((scene_path.map(str::to_string), response_cache_key));
            }
        };
        push_alias_key(
            scene_path.as_deref(),
            metric_response_cache_scope_key(
                app_id,
                scene_id.as_str(),
                scene_path.as_deref(),
                resource.id.as_str(),
                query,
                &outcome.compile_revision,
                &dependency_revision_key,
                &[],
                None,
            ),
        );
        if let Some(path) = scene_path.as_deref() {
            for variant in mei_lang_kernel::app_source_rel_path_lookup_keys(path) {
                push_alias_key(
                    Some(variant.as_str()),
                    metric_response_cache_scope_key(
                        app_id,
                        scene_id.as_str(),
                        Some(variant.as_str()),
                        resource.id.as_str(),
                        query,
                        &outcome.compile_revision,
                        &dependency_revision_key,
                        &[],
                        None,
                    ),
                );
            }
        }
        push_alias_key(
            None,
            mei_lang_datasets::metric_response_prebuild_dataset_key(
                app_id,
                resource.id.as_str(),
                query,
            ),
        );
        push_alias_key(
            None,
            mei_lang_datasets::metric_response_prebuild_shared_key(
                app_id,
                resource.id.as_str(),
                query,
                &dependency_revision_key,
            ),
        );
        for (_alias_scene_path, response_cache_key) in alias_keys {
            if state.metric_response_exact(&response_cache_key).is_some() {
                continue;
            }
            if metric_response_result_artifact_exists(app_root, &response_cache_key) {
                continue;
            }
            materialize_metric_response_alias(app_root, &response_cache_key, artifact)?;
        }
    }
    Ok(())
}

pub(crate) fn materialize_metric_dataframe_metric_aliases(
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    resource_id: &str,
    resolved_metric_id: &str,
    query_options: &DatasetQueryOptions,
    metric_defs: &BTreeMap<String, Value>,
    result: &DatasetQueryResult,
    state: &CoverageState,
) -> Result<()> {
    let (scene_id, scene_path) = artifact_scene_context(&outcome.compiled);
    for metric_selector in
        equivalent_dataframe_metric_ids(&outcome.compiled, resource_id, resolved_metric_id)
    {
        let Ok((owner_resource, canonical_metric_id)) = locate_runtime_metric_resource(
            &outcome.compiled,
            resource_id,
            metric_selector.as_str(),
        ) else {
            continue;
        };
        let owner_dataset = owner_resource
            .dataset
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
        let runtime_workset = runtime_metric_workset(
            &owner_resource.id,
            &[canonical_metric_id.clone()],
            owner_dataset,
        );
        let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
            app_root,
            &outcome.compiled,
            &owner_resource.id,
            &runtime_workset.defs_for_hydrate,
        );
        let scope_metric_token = dataframe_scope_metric_token(
            &outcome.compiled,
            owner_resource.id.as_str(),
            metric_selector.as_str(),
        )
        .unwrap_or_else(|| metric_scope_cache_key(std::slice::from_ref(&canonical_metric_id)));
        let response_cache_key = metric_dataframe_result_cache_key(
            app_root,
            Some(scene_id.as_str()),
            scene_path.as_deref(),
            owner_resource.id.as_str(),
            scope_metric_token.as_str(),
            query_options,
            &outcome.compile_revision,
            &dependency_revision_key,
            &[],
        );
        if state.metric_dataframe_exact(&response_cache_key).is_some() {
            continue;
        }
        if metric_dataframe_result_artifact_exists(app_root, &response_cache_key) {
            continue;
        }
        store_metric_dataframe_result_artifact(app_root, &response_cache_key, result)?;
    }
    let _ = metric_defs;
    Ok(())
}

pub(crate) fn materialize_metric_dataframe_sibling_aliases(
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    owner_resource: &LoadedResource,
    resolved_metric_id: &str,
    query_options: &DatasetQueryOptions,
    metric_defs: &BTreeMap<String, Value>,
    result: &DatasetQueryResult,
    state: &CoverageState,
) -> Result<()> {
    let owner_dataset = owner_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
    let identity = dataset_metric_identity_key(owner_dataset);
    let (scene_id, scene_path) = artifact_scene_context(&outcome.compiled);
    for resource in &outcome.compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if dataset_metric_identity_key(dataset) != identity {
            continue;
        }
        let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
            app_root,
            &outcome.compiled,
            resource.id.as_str(),
            metric_defs,
        );
        let scope_metric_token = dataframe_scope_metric_token(
            &outcome.compiled,
            resource.id.as_str(),
            resolved_metric_id,
        )
        .unwrap_or_else(|| {
            metric_scope_cache_key(std::slice::from_ref(&resolved_metric_id.to_string()))
        });
        let response_cache_key = metric_dataframe_result_cache_key(
            app_root,
            Some(scene_id.as_str()),
            scene_path.as_deref(),
            resource.id.as_str(),
            scope_metric_token.as_str(),
            query_options,
            &outcome.compile_revision,
            &dependency_revision_key,
            &[],
        );
        if state.metric_dataframe_exact(&response_cache_key).is_some() {
            continue;
        }
        if load_metric_dataframe_result_artifact(app_root, &response_cache_key)?.is_some() {
            continue;
        }
        store_metric_dataframe_result_artifact(app_root, &response_cache_key, result)?;
        state.store_metric_dataframe_exact(&response_cache_key, result);
    }
    Ok(())
}

pub(crate) fn materialize_metric_response_alias(
    app_root: &Path,
    response_cache_key: &str,
    artifact: &mei_lang_datasets::LoadedMetricResponseArtifact,
) -> Result<()> {
    materialize_metric_response_alias_parts(
        app_root,
        response_cache_key,
        artifact.total_rows,
        &artifact.metrics_map,
        &artifact.covered_metric_ids,
        artifact.complete,
    )
}

pub(crate) fn materialize_metric_response_alias_parts(
    app_root: &Path,
    response_cache_key: &str,
    total_rows: usize,
    metrics_map: &BTreeMap<String, mei_lang_kernel::MetricContract>,
    covered_metric_ids: &BTreeSet<String>,
    complete: bool,
) -> Result<()> {
    store_cached_metric_response(
        response_cache_key.to_string(),
        total_rows,
        metrics_map,
        covered_metric_ids,
        complete,
    );
    store_metric_response_result_artifact(
        app_root,
        response_cache_key,
        total_rows,
        metrics_map,
        covered_metric_ids,
        complete,
    )
}
