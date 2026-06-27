use super::prelude::*;
use super::*;

pub(crate) fn ensure_metric_dataframe_artifact_for_plan(
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    plan: &PlannedDataframeArtifact,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let mcg_revisions = state.active_mcg_bundle_revisions();
    if let Some(current_rev) = current_dataframe_bundle_revision(plan, &mcg_revisions) {
        if crate::graph::metric_bundle_revision_unchanged(
            &state.pre_mcg_bundle_revisions,
            plan.owner_resource_id.as_str(),
            current_rev.as_str(),
        ) && (metric_dataframe_result_artifact_exists(app_root, &plan.shared_artifact_key)
            || metric_dataframe_result_artifact_exists(app_root, &plan.artifact_key))
        {
            coverage.metric_dataframe_artifacts_skipped_bundle_unchanged += 1;
            coverage.metric_dataframe_artifacts_ready += 1;
            return Ok(());
        }
        if let (Some(source_root), Some(stored_app_id)) = (
            state.source_root.as_deref(),
            state.app_id.as_deref(),
        ) {
            let registry = crate::graph::load_mrg_registry(source_root, stored_app_id);
            let scope_key = crate::graph::mrg_eval_scope_key(
                plan.scene_id.as_str(),
                plan.scene_path.as_deref(),
            );
            if crate::graph::mrg_slot_covers_dataframe_eval(
                &registry,
                plan.owner_resource_id.as_str(),
                current_rev.as_str(),
                plan.dependency_revision_key.as_str(),
                scope_key.as_str(),
                plan.shared_artifact_key.as_str(),
            ) && (metric_dataframe_result_artifact_exists(app_root, &plan.shared_artifact_key)
                || metric_dataframe_result_artifact_exists(app_root, &plan.artifact_key))
            {
                state
                    .diagnostics
                    .dataframe_eval_skips
                    .fetch_add(1, Ordering::Relaxed);
                coverage.metric_dataframe_artifacts_skipped_bundle_unchanged += 1;
                coverage.metric_dataframe_artifacts_ready += 1;
                return Ok(());
            }
        }
    }
    let owner_resource = mei_lang_kernel::locate_dataset_resource(
        &outcome.compiled,
        plan.owner_resource_id.as_str(),
    )
    .with_context(|| {
        format!(
            "locate warmup dataset `{}` for dataframe metric `{}`",
            plan.resource_selector_id, plan.dataframe_metric_id
        )
    })?;
    let query_options = widget_dataframe_query_options(plan.page_size);
    if let Some(result) = state.metric_dataframe_shared(&plan.shared_artifact_key) {
        store_metric_dataframe_result_artifact(app_root, &plan.artifact_key, &result)?;
        state.store_metric_dataframe_exact(&plan.artifact_key, &result);
        materialize_metric_dataframe_sibling_aliases(
            app_root,
            outcome,
            &owner_resource,
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        materialize_metric_dataframe_metric_aliases(
            app_root,
            outcome,
            plan.resource_selector_id.as_str(),
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if state.metric_dataframe_exact(&plan.artifact_key).is_some() {
        if let Some(result) = state.metric_dataframe_exact(&plan.artifact_key) {
            materialize_metric_dataframe_sibling_aliases(
                app_root,
                outcome,
                &owner_resource,
                plan.resolved_metric_id.as_str(),
                &query_options,
                plan.defs_for_hydrate.as_ref(),
                &result,
                state,
            )?;
            materialize_metric_dataframe_metric_aliases(
                app_root,
                outcome,
                plan.resource_selector_id.as_str(),
                plan.resolved_metric_id.as_str(),
                &query_options,
                plan.defs_for_hydrate.as_ref(),
                &result,
                state,
            )?;
        }
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if let Some((result, _)) =
        load_metric_dataframe_result_artifact(app_root, &plan.shared_artifact_key)?
    {
        store_metric_dataframe_result_artifact(app_root, &plan.artifact_key, &result)?;
        state.store_metric_dataframe_shared(&plan.shared_artifact_key, &result);
        state.store_metric_dataframe_exact(&plan.artifact_key, &result);
        materialize_metric_dataframe_sibling_aliases(
            app_root,
            outcome,
            &owner_resource,
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        materialize_metric_dataframe_metric_aliases(
            app_root,
            outcome,
            plan.resource_selector_id.as_str(),
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if metric_dataframe_result_artifact_exists(app_root, &plan.shared_artifact_key) {
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if metric_dataframe_result_artifact_exists(app_root, &plan.artifact_key) {
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if let Some((result, _)) = load_metric_dataframe_result_artifact(app_root, &plan.artifact_key)? {
        state.store_metric_dataframe_exact(&plan.artifact_key, &result);
        state.store_metric_dataframe_shared(&plan.shared_artifact_key, &result);
        materialize_metric_dataframe_sibling_aliases(
            app_root,
            outcome,
            &owner_resource,
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        materialize_metric_dataframe_metric_aliases(
            app_root,
            outcome,
            plan.resource_selector_id.as_str(),
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if mode == PrebuildMode::Verify {
        anyhow::bail!(
            "missing metric dataframe artifact for dataset `{}` metric `{}` scope scene=`{}` target=`{}`",
            plan.resource_selector_id,
            plan.dataframe_metric_id,
            plan.scene_id,
            plan.scene_path.as_deref().unwrap_or("")
        );
    }
    let reservation = state
        .metric_dataframe_jobs
        .wait_or_reserve(&plan.shared_artifact_key);
    if let ArtifactReservation::Completed = reservation {
        if let Some(result) = state.metric_dataframe_shared(&plan.shared_artifact_key) {
            store_metric_dataframe_result_artifact(app_root, &plan.artifact_key, &result)?;
            state.store_metric_dataframe_exact(&plan.artifact_key, &result);
            materialize_metric_dataframe_sibling_aliases(
                app_root,
                outcome,
                &owner_resource,
                plan.resolved_metric_id.as_str(),
                &query_options,
                plan.defs_for_hydrate.as_ref(),
                &result,
                state,
            )?;
            materialize_metric_dataframe_metric_aliases(
                app_root,
                outcome,
                plan.resource_selector_id.as_str(),
                plan.resolved_metric_id.as_str(),
                &query_options,
                plan.defs_for_hydrate.as_ref(),
                &result,
                state,
            )?;
            coverage.metric_dataframe_artifacts_ready += 1;
            return Ok(());
        }
    }
    let metric_started = Instant::now();
    let result = query_metric_dataframe(
        &outcome.compiled,
        app_root,
        owner_resource.id.as_str(),
        plan.dataframe_metric_id.as_str(),
        Some(plan.scene_id.as_str()),
        plan.scene_path.as_deref(),
        &outcome.compile_revision,
        query_options.clone(),
        None,
        Vec::new(),
    )
    .with_context(|| {
        format!(
            "build metric dataframe artifact for dataset `{}` metric `{}`",
            plan.resource_selector_id, plan.dataframe_metric_id
        )
    });
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            state.metric_dataframe_jobs.finish(&plan.shared_artifact_key, false);
            if let (Some(source_root), Some(app_id)) =
                (state.source_root.as_deref(), state.app_id.as_deref())
            {
                let bundle_revision = current_dataframe_bundle_revision(plan, &mcg_revisions)
                    .unwrap_or_default();
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
        "[{}] 指标求值 {:.1}s | dataframe | {} | metric={} | scene={} | rows={}",
        app_root.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        metric_started.elapsed().as_secs_f64(),
        short_dataset_id(plan.resource_selector_id.as_str()),
        short_metric_id(plan.dataframe_metric_id.as_str()),
        plan.scene_id,
        result.total
    ));
    state.diagnostics.record_metric_build(
        "dataframe",
        plan.resource_selector_id.as_str(),
        plan.dataframe_metric_id.as_str(),
        plan.scene_id.as_str(),
        metric_started.elapsed().as_millis() as u64,
    );
    let store_result = (|| -> Result<()> {
        store_metric_dataframe_result_artifact(app_root, &plan.shared_artifact_key, &result)?;
        if plan.shared_artifact_key != plan.artifact_key {
            store_metric_dataframe_result_artifact(app_root, &plan.artifact_key, &result)?;
        }
        Ok(())
    })();
    state
        .metric_dataframe_jobs
        .finish(&plan.shared_artifact_key, store_result.is_ok());
    if store_result.is_ok() {
        state.store_metric_dataframe_shared(&plan.shared_artifact_key, &result);
        state.store_metric_dataframe_exact(&plan.artifact_key, &result);
    }
    store_result?;
    materialize_metric_dataframe_sibling_aliases(
        app_root,
        outcome,
        &owner_resource,
        plan.resolved_metric_id.as_str(),
        &query_options,
        plan.defs_for_hydrate.as_ref(),
        &result,
        state,
    )?;
    materialize_metric_dataframe_metric_aliases(
        app_root,
        outcome,
        plan.resource_selector_id.as_str(),
        plan.resolved_metric_id.as_str(),
        &query_options,
        plan.defs_for_hydrate.as_ref(),
        &result,
        state,
    )?;
    coverage.metric_dataframe_artifacts_built += 1;
    if let (Some(source_root), Some(stored_app_id)) = (
        state.source_root.as_deref(),
        state.app_id.as_deref(),
    ) {
        let bundle_revision = current_dataframe_bundle_revision(plan, &mcg_revisions).unwrap_or_default();
        let scope_key = crate::graph::mrg_eval_scope_key(
            plan.scene_id.as_str(),
            plan.scene_path.as_deref(),
        );
        crate::graph::record_prebuild_dataframe_slot(
            source_root,
            stored_app_id,
            plan.logical_node_id.as_str(),
            scope_key.as_str(),
            plan.owner_resource_id.as_str(),
            bundle_revision.as_str(),
            plan.dependency_revision_key.as_str(),
            plan.shared_artifact_key.as_str(),
            "eval-results/results/metric-dataframe/",
            metric_started.elapsed().as_millis() as u64,
        );
    }
    Ok(())
}

pub(crate) fn ensure_metric_dataframe_artifact(
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    resource: &LoadedResource,
    metric_id: &str,
    page_size: usize,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let Some(plan) = plan_dataframe_artifact(app_root, outcome, resource, metric_id, page_size)? else {
        return Ok(());
    };
    ensure_metric_dataframe_artifact_for_plan(app_root, outcome, &plan, mode, coverage, state)
}

pub(crate) fn prebuild_metric_dataframe_shared_key(
    dataset_id: &str,
    metric_id: &str,
    query: &DatasetQueryOptions,
    dependency_revision_key: &str,
) -> String {
    let group = serde_json::to_string(&query.group).unwrap_or_else(|_| "[]".to_string());
    let time_range =
        serde_json::to_string(&query.time_range).unwrap_or_else(|_| "null".to_string());
    format!(
        "prebuild|dataframe|dataset={dataset_id}|metric={metric_id}|dependency={dependency_revision_key}|search={}|filters={}|group={group}|time_range={time_range}",
        query.search.as_deref().unwrap_or(""),
        serde_json::to_string(&query.filters).unwrap_or_else(|_| "{}".to_string()),
    )
}

