use super::prelude::*;
use super::*;

fn mrg_slot_and_artifact_ready(
    registry: &crate::graph::mrg::registry::MrgRegistry,
    app_root: &Path,
    plan: &PlannedMetricWorkset,
    bundle_revision: &str,
) -> bool {
    let scope_key =
        crate::graph::mrg_eval_scope_key(plan.scene_id.as_str(), plan.scene_path.as_deref());
    let canonical = slot_cache_key_for_plan(plan, bundle_revision);
    let mrg_covers = crate::graph::mrg_slot_covers_eval(
        registry,
        plan.owner_resource_id.as_str(),
        bundle_revision,
        plan.dependency_revision_key.as_str(),
        scope_key.as_str(),
        canonical.as_str(),
    );
    mrg_covers
        && (metric_response_result_artifact_exists(app_root, canonical.as_str())
            || metric_response_result_artifact_exists(app_root, plan.shared_cache_key.as_str())
            || metric_response_result_artifact_exists(app_root, plan.response_cache_key.as_str()))
}

fn active_mcg_bundle_revisions(state: &CoverageState) -> BTreeMap<String, String> {
    state.active_mcg_bundle_revisions()
}

pub(crate) fn ensure_metric_response_artifact_for_plan(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    plan: &PlannedMetricWorkset,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let mcg_revisions = active_mcg_bundle_revisions(state);
    if let Some(current_rev) = current_bundle_revision_for_plan(plan, &mcg_revisions) {
        if crate::graph::metric_bundle_revision_unchanged(
            &state.pre_mcg_bundle_revisions,
            plan.owner_resource_id.as_str(),
            current_rev.as_str(),
        ) {
            if let (Some(source_root), Some(stored_app_id)) =
                (state.source_root.as_deref(), state.app_id.as_deref())
            {
                let registry = crate::graph::load_mrg_registry(source_root, stored_app_id);
                if mrg_slot_and_artifact_ready(&registry, app_root, plan, current_rev.as_str()) {
                    promote_prebuild_metric_response_slot(
                        state.source_root.as_deref(),
                        state.app_id.as_deref(),
                        plan,
                        current_rev.as_str(),
                    );
                    coverage.metric_response_artifacts_skipped_bundle_unchanged += 1;
                    coverage.metric_response_artifacts_ready += 1;
                    return Ok(());
                }
            }
        }
        if let (Some(source_root), Some(stored_app_id)) =
            (state.source_root.as_deref(), state.app_id.as_deref())
        {
            let registry = crate::graph::load_mrg_registry(source_root, stored_app_id);
            if mrg_slot_and_artifact_ready(&registry, app_root, plan, current_rev.as_str()) {
                promote_prebuild_metric_response_slot(
                    state.source_root.as_deref(),
                    state.app_id.as_deref(),
                    plan,
                    current_rev.as_str(),
                );
                state
                    .diagnostics
                    .mrg_eval_skips
                    .fetch_add(1, Ordering::Relaxed);
                coverage.metric_response_artifacts_skipped_bundle_unchanged += 1;
                coverage.metric_response_artifacts_ready += 1;
                return Ok(());
            }
        }
    }
    let owner_resource = mei_lang_kernel::locate_dataset_resource(
        &outcome.compiled,
        plan.owner_resource_id.as_str(),
    )
    .with_context(|| format!("locate warmup dataset `{}`", plan.dataset_selector))?;
    let owner_dataset = owner_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
    if !state
        .hydrated_owners
        .lock()
        .expect("prebuild dataset pool lock")
        .insert(plan.owner_resource_id.clone())
    {
        state
            .diagnostics
            .mrg_eval_skips
            .fetch_add(1, Ordering::Relaxed);
    }
    let query_state = empty_query_state();
    let query = collect_all_query_options(&query_state);
    if let Some(artifact) = state.metric_response_exact(&plan.response_cache_key) {
        let artifact_covers_request = metric_response_artifact_covers_request(
            &artifact,
            &plan.covered_metric_ids,
            plan.request_all_metrics,
        );
        if artifact_covers_request {
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                &owner_resource,
                &artifact,
                &query,
                plan.defs_for_hydrate.as_ref(),
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
    }
    if let Some(artifact) = state.metric_response_shared(&plan.shared_cache_key) {
        let artifact_covers_request = metric_response_artifact_covers_request(
            &artifact,
            &plan.covered_metric_ids,
            plan.request_all_metrics,
        );
        if artifact_covers_request {
            materialize_metric_response_alias(app_root, &plan.response_cache_key, &artifact)?;
            state.store_metric_response_exact(&plan.response_cache_key, &artifact);
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                &owner_resource,
                &artifact,
                &query,
                plan.defs_for_hydrate.as_ref(),
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
    }
    if metric_response_result_artifact_exists(app_root, plan.shared_cache_key.as_str())
        || metric_response_result_artifact_exists(app_root, plan.response_cache_key.as_str())
    {
        coverage.metric_response_artifacts_ready += 1;
        return Ok(());
    }
    if let Some((artifact, _)) =
        load_metric_response_result_artifact(app_root, &plan.response_cache_key)?
    {
        let artifact_covers_request = metric_response_artifact_covers_request(
            &artifact,
            &plan.covered_metric_ids,
            plan.request_all_metrics,
        );
        if artifact_covers_request {
            state.store_metric_response_exact(&plan.response_cache_key, &artifact);
            state.store_metric_response_shared(&plan.shared_cache_key, &artifact);
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                &owner_resource,
                &artifact,
                &query,
                plan.defs_for_hydrate.as_ref(),
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
        if mode == PrebuildMode::Verify {
            anyhow::bail!(
                "metric response artifact for dataset `{}` scope scene=`{}` target=`{}` does not cover all declared metrics",
                plan.dataset_selector,
                plan.scene_id,
                plan.scene_path.as_deref().unwrap_or("")
            );
        }
    } else if mode == PrebuildMode::Verify {
        anyhow::bail!(
            "missing metric response artifact for dataset `{}` scope scene=`{}` target=`{}`",
            plan.dataset_selector,
            plan.scene_id,
            plan.scene_path.as_deref().unwrap_or("")
        );
    }
    if let Some((artifact, _)) =
        load_metric_response_result_artifact(app_root, &plan.shared_cache_key)?
    {
        let artifact_covers_request = metric_response_artifact_covers_request(
            &artifact,
            &plan.covered_metric_ids,
            plan.request_all_metrics,
        );
        if artifact_covers_request {
            materialize_metric_response_alias(app_root, &plan.response_cache_key, &artifact)?;
            state.store_metric_response_shared(&plan.shared_cache_key, &artifact);
            state.store_metric_response_exact(&plan.response_cache_key, &artifact);
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                &owner_resource,
                &artifact,
                &query,
                plan.defs_for_hydrate.as_ref(),
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
    }
    let reservation = state
        .metric_response_jobs
        .wait_or_reserve(&plan.shared_cache_key);
    if let ArtifactReservation::Completed = reservation {
        if let Some(artifact) = state.metric_response_shared(&plan.shared_cache_key) {
            let artifact_covers_request = metric_response_artifact_covers_request(
                &artifact,
                &plan.covered_metric_ids,
                plan.request_all_metrics,
            );
            if artifact_covers_request {
                materialize_metric_response_alias(app_root, &plan.response_cache_key, &artifact)?;
                state.store_metric_response_exact(&plan.response_cache_key, &artifact);
                materialize_metric_response_sibling_aliases(
                    app_id,
                    app_root,
                    outcome,
                    &owner_resource,
                    &artifact,
                    &query,
                    plan.defs_for_hydrate.as_ref(),
                    state,
                )?;
                coverage.metric_response_artifacts_ready += 1;
                return Ok(());
            }
        }
    }
    let metric_started = Instant::now();
    let primary_resource =
        mei_lang_kernel::locate_dataset_resource(&outcome.compiled, plan.dataset_selector.as_str())
            .with_context(|| format!("locate warmup dataset `{}`", plan.dataset_selector))?;
    let primary_dataset = primary_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", primary_resource.id))?;
    let access_plan = AccessMetricEvalPlan {
        primary: primary_resource,
        primary_dataset,
        owner: owner_resource,
        owner_dataset,
        request_metric_ids: plan.requested_metric_ids.clone(),
    };
    let eval_outcome = evaluate_runtime_metrics_from_plan(
        &outcome.compiled,
        app_root,
        &access_plan,
        plan.scene_id.as_str(),
        plan.scene_path.as_deref(),
        &query_state,
        &[],
        RuntimeMetricEvalMode::WithDag,
        plan.request_all_metrics,
    )
    .with_context(|| {
        format!(
            "build metric response artifact for dataset `{}`",
            plan.dataset_selector
        )
    });
    let eval_outcome = match eval_outcome {
        Ok(eval_outcome) => eval_outcome,
        Err(error) => {
            state
                .metric_response_jobs
                .finish(&plan.shared_cache_key, false);
            if let Some(source_root) = state.source_root.as_deref() {
                let bundle_revision =
                    current_bundle_revision_for_plan(plan, &mcg_revisions).unwrap_or_default();
                let scope_key = crate::graph::mrg_eval_scope_key(
                    plan.scene_id.as_str(),
                    plan.scene_path.as_deref(),
                );
                crate::graph::record_prebuild_slot_failed(
                    source_root,
                    app_id,
                    plan.logical_node_id.as_str(),
                    scope_key.as_str(),
                    plan.owner_resource_id.as_str(),
                    bundle_revision.as_str(),
                    plan.dependency_revision_key.as_str(),
                    error.to_string().as_str(),
                );
            }
            return Err(error);
        }
    };
    prebuild_emit_progress_detail(format!(
        "[{app_id}] 指标求值 {:.1}s | response | dataset={} | scene={} | rows={}",
        metric_started.elapsed().as_secs_f64(),
        short_dataset_id(plan.dataset_selector.as_str()),
        plan.scene_id,
        eval_outcome.total_rows
    ));
    state.diagnostics.record_metric_build(
        "response",
        plan.dataset_selector.as_str(),
        "(bundle)",
        plan.scene_id.as_str(),
        metric_started.elapsed().as_millis() as u64,
    );
    let declared_metric_ids = owner_dataset
        .runtime_metric_defs
        .keys()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let complete = plan.request_all_metrics
        && !declared_metric_ids.is_empty()
        && declared_metric_ids
            .iter()
            .all(|metric_id| plan.covered_metric_ids.contains(metric_id));
    let built_artifact = LoadedMetricResponseArtifact {
        total_rows: eval_outcome.total_rows,
        metrics_map: eval_outcome.metrics_map.clone(),
        covered_metric_ids: plan.covered_metric_ids.clone(),
        complete,
    };
    let store_result = (|| -> Result<()> {
        store_cached_metric_response(
            plan.shared_cache_key.clone(),
            eval_outcome.total_rows,
            &eval_outcome.metrics_map,
            &plan.covered_metric_ids,
            complete,
        );
        store_metric_response_result_artifact(
            app_root,
            &plan.shared_cache_key,
            eval_outcome.total_rows,
            &eval_outcome.metrics_map,
            &plan.covered_metric_ids,
            complete,
        )?;
        let dataset_key = mei_lang_datasets::metric_response_prebuild_dataset_key(
            app_id,
            plan.owner_resource_id.as_str(),
            &query,
        );
        materialize_metric_response_alias(app_root, dataset_key.as_str(), &built_artifact)?;
        materialize_metric_response_alias_parts(
            app_root,
            &plan.response_cache_key,
            eval_outcome.total_rows,
            &eval_outcome.metrics_map,
            &plan.covered_metric_ids,
            complete,
        )?;
        Ok(())
    })();
    state
        .metric_response_jobs
        .finish(&plan.shared_cache_key, store_result.is_ok());
    if store_result.is_ok() {
        state.store_metric_response_shared(&plan.shared_cache_key, &built_artifact);
        state.store_metric_response_exact(&plan.response_cache_key, &built_artifact);
    }
    store_result?;
    materialize_metric_response_sibling_aliases(
        app_id,
        app_root,
        outcome,
        &owner_resource,
        &built_artifact,
        &query,
        plan.defs_for_hydrate.as_ref(),
        state,
    )?;
    coverage.metric_response_artifacts_built += 1;
    if let Some(source_root) = state.source_root.as_deref() {
        let bundle_revision =
            current_bundle_revision_for_plan(plan, &mcg_revisions).unwrap_or_default();
        let scope_key =
            crate::graph::mrg_eval_scope_key(plan.scene_id.as_str(), plan.scene_path.as_deref());
        let _ = crate::graph::mrg::eval_nodes::persist_workset_node(
            source_root,
            app_id,
            plan.logical_node_id.as_str(),
            plan.owner_resource_id.as_str(),
            plan.covered_metric_ids
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .as_slice(),
        );
        let _ = crate::graph::mrg::eval_nodes::persist_eval_plan_node(
            source_root,
            app_id,
            plan.owner_resource_id.as_str(),
            bundle_revision.as_str(),
            &serde_json::json!({
                "worksetId": plan.logical_node_id,
                "responseCacheKey": plan.shared_cache_key,
                "scopeKey": scope_key,
            }),
        );
        crate::graph::record_prebuild_slot(
            source_root,
            app_id,
            plan.logical_node_id.as_str(),
            scope_key.as_str(),
            plan.owner_resource_id.as_str(),
            bundle_revision.as_str(),
            plan.dependency_revision_key.as_str(),
            plan.shared_cache_key.as_str(),
            "eval-results/results/metric-response/",
            metric_started.elapsed().as_millis() as u64,
        );
    }
    Ok(())
}

pub(crate) fn ensure_metric_response_artifact_for_request(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    dataset_selector: &str,
    metric_ids: &[String],
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let plan = plan_metric_workset(app_id, app_root, outcome, dataset_selector, metric_ids)?;
    ensure_metric_response_artifact_for_plan(
        app_id, app_root, outcome, &plan, mode, coverage, state,
    )
}

pub(crate) fn dataframe_scope_metric_token(
    compiled: &mei_lang_kernel::CompiledApp,
    resource_id: &str,
    metric_selector: &str,
) -> Option<String> {
    let (_, resolved_metric_id) =
        locate_runtime_metric_resource(compiled, resource_id, metric_selector).ok()?;
    Some(metric_scope_cache_key(std::slice::from_ref(
        &resolved_metric_id,
    )))
}

pub(crate) fn prebuild_dataframe_metric_selector(
    metric_defs: &BTreeMap<String, Value>,
    resolved_metric_id: &str,
) -> String {
    let resolved_metric_id = resolved_metric_id.trim();
    if resolved_metric_id.is_empty() || resolved_metric_id.ends_with("::__scalar_rowset__") {
        return resolved_metric_id.to_string();
    }
    let scalar_rowset_id = format!("{resolved_metric_id}::__scalar_rowset__");
    if metric_defs.contains_key(&scalar_rowset_id) {
        return scalar_rowset_id;
    }
    let shape = metric_defs
        .get(resolved_metric_id)
        .and_then(Value::as_object)
        .and_then(|map| map.get("shape"))
        .and_then(Value::as_str);
    if matches!(shape, Some("scalar") | Some("scalar_map")) {
        return scalar_rowset_id;
    }
    resolved_metric_id.to_string()
}
