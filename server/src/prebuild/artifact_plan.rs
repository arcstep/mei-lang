use super::prelude::*;
use super::*;

pub(crate) fn logical_dataframe_artifact_id(
    owner_resource_id: &str,
    metric_id: &str,
    page_size: usize,
) -> String {
    format!("dataframe|owner={owner_resource_id}|metric={metric_id}|page_size={page_size}")
}

pub(crate) fn plan_metric_workset(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    dataset_selector: &str,
    metric_ids: &[String],
) -> Result<PlannedMetricWorkset> {
    let request_all_metrics = metric_ids.is_empty();
    let access_plan =
        plan_access_metric_eval_for_ids(&outcome.compiled, dataset_selector, metric_ids)
            .with_context(|| {
                format!(
                    "plan metric response artifact for dataset `{dataset_selector}` metrics [{}]",
                    summarize_metric_ids(metric_ids)
                )
            })?;
    let runtime_workset = runtime_metric_workset(
        &access_plan.owner.id,
        &access_plan.request_metric_ids,
        access_plan.owner_dataset,
    );
    let covered_metric_ids = runtime_workset
        .eval_metric_ids
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let defs_for_hydrate = Arc::new(runtime_workset.defs_for_hydrate);
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root,
        &outcome.compiled,
        &access_plan.owner.id,
        defs_for_hydrate.as_ref(),
    );
    let query_state = empty_query_state();
    let query = collect_all_query_options(&query_state);
    let (scene_id, scene_path) =
        artifact_scene_context_for_resource(&outcome.compiled, access_plan.owner.id.as_str());
    let scope_id = scope_identity_key(scene_id.as_str(), scene_path.as_deref());
    let logical_node_id = logical_metric_workset_id(
        app_id,
        access_plan.owner.id.as_str(),
        &access_plan.request_metric_ids,
    );
    let materialization_key = materialization_identity(
        logical_node_id.as_str(),
        scope_id.as_str(),
        dependency_revision_key.as_str(),
        outcome.compile_revision.as_str(),
    );
    let response_cache_key = metric_response_cache_scope_key(
        app_id,
        scene_id.as_str(),
        scene_path.as_deref(),
        &access_plan.owner.id,
        &query,
        &outcome.compile_revision,
        &dependency_revision_key,
        &[],
        None,
    );
    let shared_cache_key = metric_response_prebuild_shared_key(
        app_id,
        &access_plan.owner.id,
        &query,
        &dependency_revision_key,
    );
    Ok(PlannedMetricWorkset {
        logical_node_id,
        scope_id,
        materialization_key,
        dataset_selector: dataset_selector.to_string(),
        owner_resource_id: access_plan.owner.id.clone(),
        requested_metric_ids: access_plan.request_metric_ids,
        request_all_metrics,
        scene_id,
        scene_path,
        dependency_revision_key,
        response_cache_key,
        shared_cache_key,
        covered_metric_ids,
        defs_for_hydrate,
    })
}

pub(crate) fn plan_dataframe_artifact(
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    resource: &LoadedResource,
    metric_id: &str,
    page_size: usize,
) -> Result<Option<PlannedDataframeArtifact>> {
    let Ok((owner_resource, resolved_metric_id)) =
        locate_runtime_metric_resource(&outcome.compiled, resource.id.as_str(), metric_id)
    else {
        return Ok(None);
    };
    let owner_dataset = owner_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
    let dataframe_metric_id =
        prebuild_dataframe_metric_selector(&owner_dataset.runtime_metric_defs, &resolved_metric_id);
    let runtime_workset = runtime_metric_workset(
        &owner_resource.id,
        std::slice::from_ref(&dataframe_metric_id),
        owner_dataset,
    );
    let defs_for_hydrate = Arc::new(runtime_workset.defs_for_hydrate);
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root,
        &outcome.compiled,
        &owner_resource.id,
        if owner_dataset.runtime_metric_defs.is_empty() {
            defs_for_hydrate.as_ref()
        } else {
            &owner_dataset.runtime_metric_defs
        },
    );
    let query_options = widget_dataframe_query_options(page_size);
    let (scene_id, scene_path) =
        artifact_scene_context_for_resource(&outcome.compiled, owner_resource.id.as_str());
    let scope_metric_token = dataframe_scope_metric_token(
        &outcome.compiled,
        resource.id.as_str(),
        dataframe_metric_id.as_str(),
    )
    .unwrap_or_else(|| metric_scope_cache_key(std::slice::from_ref(&dataframe_metric_id)));
    let artifact_key = metric_dataframe_result_cache_key(
        app_root,
        Some(scene_id.as_str()),
        scene_path.as_deref(),
        owner_resource.id.as_str(),
        scope_metric_token.as_str(),
        &query_options,
        &outcome.compile_revision,
        &dependency_revision_key,
        &[],
    );
    let shared_artifact_key = prebuild_metric_dataframe_shared_key(
        owner_resource.id.as_str(),
        dataframe_metric_id.as_str(),
        &query_options,
        &dependency_revision_key,
    );
    let scope_id = scope_identity_key(scene_id.as_str(), scene_path.as_deref());
    let logical_node_id = logical_dataframe_artifact_id(
        owner_resource.id.as_str(),
        dataframe_metric_id.as_str(),
        page_size,
    );
    let materialization_key = materialization_identity(
        logical_node_id.as_str(),
        scope_id.as_str(),
        dependency_revision_key.as_str(),
        outcome.compile_revision.as_str(),
    );
    Ok(Some(PlannedDataframeArtifact {
        logical_node_id,
        scope_id,
        materialization_key,
        artifact_key,
        shared_artifact_key,
        owner_resource_id: owner_resource.id.clone(),
        resource_selector_id: resource.id.clone(),
        dataframe_metric_id,
        resolved_metric_id,
        page_size,
        scene_id,
        scene_path,
        dependency_revision_key,
        scope_metric_token,
        defs_for_hydrate,
    }))
}

/// Prebuild materializes one full rowset per metric; runtime paginates at query time.
pub(crate) const PREBUILD_DATAFRAME_CANONICAL_PAGE_SIZE: usize = 1000;

pub(crate) fn prebuild_dataframe_page_sizes() -> &'static [usize] {
    static PAGE_SIZES: OnceLock<Vec<usize>> = OnceLock::new();
    PAGE_SIZES.get_or_init(|| {
        let mut sizes = std::env::var("MEI_PREBUILD_DATAFRAME_PAGE_SIZES")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .filter_map(|item| item.trim().parse::<usize>().ok())
                    .collect::<Vec<_>>()
            })
            .filter(|sizes| !sizes.is_empty())
            .unwrap_or_else(|| vec![PREBUILD_DATAFRAME_CANONICAL_PAGE_SIZE]);
        sizes.sort_unstable();
        sizes.dedup();
        sizes
    })
}

pub(crate) fn prebuild_dataframe_query_options(page_size: usize) -> DatasetQueryOptions {
    DatasetQueryOptions {
        page: 1,
        page_size,
        collect_all: page_size >= PREBUILD_DATAFRAME_CANONICAL_PAGE_SIZE,
        ..Default::default()
    }
}

pub(crate) fn widget_dataframe_page_sizes() -> &'static [usize] {
    prebuild_dataframe_page_sizes()
}

pub(crate) fn widget_dataframe_query_options(page_size: usize) -> DatasetQueryOptions {
    prebuild_dataframe_query_options(page_size)
}

pub(crate) fn equivalent_dataframe_metric_ids(
    compiled: &mei_lang_kernel::CompiledApp,
    resource_id: &str,
    resolved_metric_id: &str,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    ids.insert(resolved_metric_id.trim().to_string());
    let Ok((owner, resolved)) =
        locate_runtime_metric_resource(compiled, resource_id, resolved_metric_id)
    else {
        return ids.into_iter().collect();
    };
    let Some(dataset) = owner.dataset.as_ref() else {
        return ids.into_iter().collect();
    };
    for def_key in dataset.runtime_metric_defs.keys() {
        if let Ok((_, candidate)) = locate_runtime_metric_resource(compiled, resource_id, def_key) {
            if candidate == resolved {
                ids.insert(def_key.trim().to_string());
            }
        }
    }
    ids.into_iter().collect()
}

pub(crate) fn dataset_metric_identity_key(dataset: &DatasetView) -> String {
    let mut metric_keys = dataset
        .runtime_metric_defs
        .keys()
        .map(|metric_id| metric_id.as_str())
        .collect::<Vec<_>>();
    metric_keys.sort_unstable();
    let source_path = dataset.source.path.trim().replace('\\', "/");
    format!("{}|{}", source_path, metric_keys.join(","))
}
