use super::prelude::*;
use super::*;

pub(crate) fn ensure_scope_artifacts(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    plan: &ScopeArtifactPlan,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let mrg_registry = if crate::graph::feature::graph_registry_dedup_enabled() {
        state
            .source_root
            .as_deref()
            .zip(state.app_id.as_deref())
            .map(|(source_root, registry_app)| {
                crate::graph::load_mrg_registry(source_root, registry_app)
            })
    } else {
        None
    };
    let _dirty_frontier = mrg_registry
        .as_ref()
        .map(|registry| registry.dirty_slots().len())
        .unwrap_or(0);
    for workset in &plan.metric_worksets {
        if let Some(registry) = mrg_registry.as_ref() {
            if let Some(current_rev) = current_bundle_revision_for_plan(workset) {
                let scope_key = crate::graph::mrg_eval_scope_key(
                    workset.scene_id.as_str(),
                    workset.scene_path.as_deref(),
                );
                let mrg_covers = crate::graph::mrg_slot_covers_eval(
                    registry,
                    workset.owner_resource_id.as_str(),
                    current_rev.as_str(),
                    workset.dependency_revision_key.as_str(),
                    scope_key.as_str(),
                    workset.shared_cache_key.as_str(),
                ) && prebuild_metric_response_index_covers_key(
                    app_root,
                    &workset.shared_cache_key,
                    &workset.covered_metric_ids,
                    workset.request_all_metrics,
                )?;
                if mrg_covers {
                    state
                        .diagnostics
                        .mrg_eval_skips
                        .fetch_add(1, Ordering::Relaxed);
                    coverage.metric_response_artifacts_skipped_bundle_unchanged += 1;
                    coverage.metric_response_artifacts_ready += 1;
                    continue;
                }
            }
        }
        ensure_metric_response_artifact_for_plan(
            app_id,
            app_root,
            outcome,
            workset,
            mode,
            coverage,
            state,
        )?;
    }
    for dataframe in &plan.dataframe_artifacts {
        if let Some(registry) = mrg_registry.as_ref() {
            if let Some(current_rev) = current_dataframe_bundle_revision(dataframe) {
                let scope_key = crate::graph::mrg_eval_scope_key(
                    dataframe.scene_id.as_str(),
                    dataframe.scene_path.as_deref(),
                );
                let mrg_covers = crate::graph::mrg_slot_covers_dataframe_eval(
                    registry,
                    dataframe.owner_resource_id.as_str(),
                    current_rev.as_str(),
                    dataframe.dependency_revision_key.as_str(),
                    scope_key.as_str(),
                    dataframe.shared_artifact_key.as_str(),
                ) && (metric_dataframe_result_artifact_exists(app_root, &dataframe.shared_artifact_key)
                    || metric_dataframe_result_artifact_exists(app_root, &dataframe.artifact_key));
                if mrg_covers {
                    state
                        .diagnostics
                        .dataframe_eval_skips
                        .fetch_add(1, Ordering::Relaxed);
                    coverage.metric_dataframe_artifacts_skipped_bundle_unchanged += 1;
                    coverage.metric_dataframe_artifacts_ready += 1;
                    continue;
                }
            }
        }
        ensure_metric_dataframe_artifact_for_plan(
            app_root,
            outcome,
            dataframe,
            mode,
            coverage,
            state,
        )?;
    }
    Ok(())
}

pub(crate) fn ensure_request_artifacts_for_compiled(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    dataset_selector: &str,
    metric_ids: &[String],
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let resource = mei_lang_kernel::locate_dataset_resource(&outcome.compiled, dataset_selector)
        .with_context(|| format!("locate warmup dataset `{dataset_selector}`"))?;
    let dataset = resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", resource.id))?;
    if metric_ids.is_empty() {
        let response_metric_ids = response_metric_ids(&outcome.compiled, dataset);
        if !response_metric_ids.is_empty() {
            let metric_groups = group_metric_ids_by_owner(
                &outcome.compiled,
                resource.id.as_str(),
                &response_metric_ids,
            )?;
            for metric_ids in metric_groups.into_values() {
                ensure_metric_response_artifact_for_request(
                    app_id,
                    app_root,
                    outcome,
                    resource.id.as_str(),
                    metric_ids.as_slice(),
                    mode,
                    coverage,
                    state,
                )?;
            }
        } else {
            ensure_metric_response_artifact_for_request(
                app_id,
                app_root,
                outcome,
                resource.id.as_str(),
                metric_ids,
                mode,
                coverage,
                state,
            )?;
        }
    } else {
        ensure_metric_response_artifact_for_request(
            app_id,
            app_root,
            outcome,
            resource.id.as_str(),
            metric_ids,
            mode,
            coverage,
            state,
        )?;
    }
    if is_world_metrics_resource(resource.id.as_str()) {
        let mut dataframe_metrics = requested_dataframe_metric_ids(dataset, metric_ids);
        dataframe_metrics.sort();
        dataframe_metrics.dedup();
        for metric_id in dataframe_metrics {
            for page_size in widget_dataframe_page_sizes() {
                ensure_metric_dataframe_artifact(
                    app_root,
                    outcome,
                    resource,
                    metric_id.as_str(),
                    *page_size,
                    mode,
                    coverage,
                    state,
                )?;
            }
        }
        return Ok(());
    }
    for metric_id in dataframe_metric_ids(dataset) {
        for page_size in widget_dataframe_page_sizes() {
            ensure_metric_dataframe_artifact(
                app_root,
                outcome,
                &resource,
                metric_id.as_str(),
                *page_size,
                mode,
                coverage,
                state,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn is_world_metrics_resource(resource_id: &str) -> bool {
    let resource_id = resource_id.trim();
    resource_id == "__world_metrics__" || resource_id.starts_with("__world_metrics__::")
}

pub(crate) fn compiled_has_world_metrics_runtime_defs(compiled: &CompiledApp) -> bool {
    compiled.resources.iter().any(|resource| {
        resource.dataset.as_ref().is_some_and(|dataset| {
            dataset.has_runtime_metric_defs() && is_world_metrics_resource(resource.id.as_str())
        })
    })
}

pub(crate) fn dataset_can_materialize_metric_artifacts(
    compiled: &CompiledApp,
    dataset_selector: &str,
) -> bool {
    let Ok(resource) = mei_lang_kernel::locate_dataset_resource(compiled, dataset_selector) else {
        return false;
    };
    let Some(dataset) = resource.dataset.as_ref() else {
        return false;
    };
    if dataset.has_runtime_metric_defs() {
        return true;
    }
    compiled_has_world_metrics_runtime_defs(compiled)
}

pub(crate) fn response_metric_ids(
    compiled: &mei_lang_kernel::CompiledApp,
    dataset: &DatasetView,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    ids.extend(
        dataset
            .runtime_analysis_contracts
            .keys()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(str::to_string),
    );
    ids.extend(
        dataset
            .runtime_metric_defs
            .keys()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(str::to_string),
    );
    if ids.is_empty() {
        ids.extend(
            compiled
                .world_metrics
                .keys()
                .map(|id| id.trim())
                .filter(|id| !id.is_empty())
                .map(str::to_string),
        );
    }
    ids.into_iter().collect()
}

pub(crate) fn group_metric_ids_by_owner(
    compiled: &mei_lang_kernel::CompiledApp,
    dataset_id: &str,
    metric_ids: &[String],
) -> Result<BTreeMap<String, Vec<String>>> {
    let mut groups = BTreeMap::<String, Vec<String>>::new();
    for metric_id in metric_ids {
        let (owner, _) = locate_runtime_metric_resource(compiled, dataset_id, metric_id)?;
        groups
            .entry(owner.id.clone())
            .or_default()
            .push(metric_id.clone());
    }
    Ok(groups)
}

pub(crate) fn dataframe_metric_ids(dataset: &DatasetView) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for contract in dataset.runtime_analysis_contracts.values() {
        collect_contract_metric_ids(contract, &mut ids);
    }
    ids.into_iter().collect()
}

pub(crate) fn requested_dataframe_metric_ids(dataset: &DatasetView, metric_ids: &[String]) -> Vec<String> {
    let mut ids = if metric_ids.is_empty() {
        dataframe_metric_ids(dataset)
            .into_iter()
            .collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };
    for metric_id in metric_ids {
        let metric_id = metric_id.trim();
        if metric_id.is_empty() {
            continue;
        }
        if metric_id.ends_with("::__scalar_rowset__") || metric_def_is_dataframe(dataset, metric_id)
        {
            ids.insert(metric_id.to_string());
        }
    }
    ids.into_iter().collect()
}

pub(crate) fn metric_def_is_dataframe(dataset: &DatasetView, metric_id: &str) -> bool {
    dataset
        .runtime_metric_defs
        .get(metric_id)
        .and_then(Value::as_object)
        .and_then(|map| map.get("shape"))
        .and_then(Value::as_str)
        .is_some_and(|shape| shape == "dataframe")
}

pub(crate) fn collect_contract_metric_ids(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let is_metric_key = matches!(
                    key.as_str(),
                    "metric_id"
                        | "table_metric_id"
                        | "detail_table_metric_id"
                        | "drilldown_table_metric_id"
                );
                if is_metric_key {
                    if let Some(metric_id) = child
                        .as_str()
                        .map(str::trim)
                        .filter(|metric_id| !metric_id.is_empty())
                    {
                        out.insert(metric_id.to_string());
                    }
                }
                collect_contract_metric_ids(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_contract_metric_ids(item, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn requested_metric_ids(request: &RuntimeWarmupDatasetRequest) -> Vec<String> {
    let mut metric_ids = request
        .metric_ids
        .iter()
        .map(|metric_id| metric_id.trim())
        .filter(|metric_id| !metric_id.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if let Some(metric_id) = request
        .metric_id
        .as_deref()
        .map(str::trim)
        .filter(|metric_id| !metric_id.is_empty())
    {
        metric_ids.push(metric_id.to_string());
    }
    metric_ids.sort();
    metric_ids.dedup();
    metric_ids
}
