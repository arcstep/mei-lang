use super::prelude::*;
use super::*;

pub(crate) fn collect_request_artifact_plans(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    dataset_selector: &str,
    metric_ids: &[String],
    metric_worksets: &mut BTreeMap<String, PlannedMetricWorkset>,
    dataframe_tasks: &mut BTreeMap<String, PlannedDataframeArtifact>,
) -> Result<()> {
    let resource = mei_lang_kernel::locate_dataset_resource(&outcome.compiled, dataset_selector)
        .with_context(|| format!("locate warmup dataset `{dataset_selector}`"))?;
    let dataset = resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", resource.id))?;
    if metric_ids.is_empty() {
        let response_ids = response_metric_ids(&outcome.compiled, dataset);
        if !response_ids.is_empty() {
            let metric_groups =
                group_metric_ids_by_owner(&outcome.compiled, resource.id.as_str(), &response_ids)?;
            for metric_ids in metric_groups.into_values() {
                let plan = plan_metric_workset(
                    app_id,
                    app_root,
                    outcome,
                    resource.id.as_str(),
                    metric_ids.as_slice(),
                )?;
                metric_worksets
                    .entry(plan.materialization_key.clone())
                    .or_insert(plan);
            }
        } else {
            let plan = plan_metric_workset(app_id, app_root, outcome, resource.id.as_str(), &[])?;
            metric_worksets
                .entry(plan.materialization_key.clone())
                .or_insert(plan);
        }
    } else {
        let metric_groups =
            group_metric_ids_by_owner(&outcome.compiled, resource.id.as_str(), metric_ids)?;
        for metric_ids in metric_groups.into_values() {
            let plan = plan_metric_workset(
                app_id,
                app_root,
                outcome,
                resource.id.as_str(),
                metric_ids.as_slice(),
            )?;
            metric_worksets
                .entry(plan.materialization_key.clone())
                .or_insert(plan);
        }
    }

    if is_world_metrics_resource(resource.id.as_str()) {
        let mut requested = requested_dataframe_metric_ids(dataset, metric_ids);
        requested.sort();
        requested.dedup();
        for metric_id in requested {
            for page_size in widget_dataframe_page_sizes() {
                if let Some(plan) = plan_dataframe_artifact(
                    app_root,
                    outcome,
                    &resource,
                    metric_id.as_str(),
                    *page_size,
                )? {
                    dataframe_tasks
                        .entry(plan.materialization_key.clone())
                        .or_insert(plan);
                }
            }
        }
        return Ok(());
    }

    for metric_id in dataframe_metric_ids(dataset) {
        for page_size in widget_dataframe_page_sizes() {
            if let Some(plan) = plan_dataframe_artifact(
                app_root,
                outcome,
                &resource,
                metric_id.as_str(),
                *page_size,
            )? {
                dataframe_tasks
                    .entry(plan.materialization_key.clone())
                    .or_insert(plan);
            }
        }
    }
    Ok(())
}

fn is_world_metrics_warmup_dataset(dataset_id: &str) -> bool {
    dataset_id.trim().starts_with("__world_metrics__")
}

fn is_home_route_scope(scope: &CompileScope) -> bool {
    let canonical = scope.canonicalized();
    if canonical.requested_scene_id.as_deref() != Some("home") {
        return false;
    }
    match canonical
        .requested_target_file
        .as_deref()
        .map(mei_lang_kernel::canonical_app_source_rel_path)
        .filter(|target| !target.is_empty())
    {
        None => true,
        Some(target) => target.ends_with("home.mei"),
    }
}

fn home_embedded_warmup_request_matches_scope(
    request: &AggregatedWarmupRequest,
    scope: &CompileScope,
) -> bool {
    if request.scope.key() == scope.key() {
        return true;
    }
    if !is_home_route_scope(scope) {
        return false;
    }
    let request_scope = request.scope.canonicalized();
    if request_scope.requested_scene_id.as_deref() != Some("home") {
        return false;
    }
    if is_world_metrics_warmup_dataset(request.dataset_id.as_str()) {
        return false;
    }
    // 仅放宽「scene=home 且 target 由 namespaced dataset 推导、无独立 focus」的嵌入 capsule，
    // 例如 typical_cases；带 focus 的条目（如 06 实时预警 warning_list）仍走各自 scope。
    let inferred_target = warmup_dataset_selector_target_file(request.dataset_id.as_str());
    let Some(inferred_target) = inferred_target else {
        return false;
    };
    let request_target = request_scope
        .requested_target_file
        .as_deref()
        .map(mei_lang_kernel::canonical_app_source_rel_path)
        .filter(|target| !target.is_empty());
    let Some(request_target) = request_target else {
        return false;
    };
    let inferred_target =
        mei_lang_kernel::canonical_app_source_rel_path(inferred_target.as_str());
    request_target == inferred_target && !request_target.ends_with("home.mei")
}

fn collect_home_embedded_warmup_artifact_plans(
    app_id: &str,
    app_root: &Path,
    scope: &CompileScope,
    planning_outcome: &SharedCompileOutcome,
    all_warmup_requests: &[AggregatedWarmupRequest],
    metric_worksets: &mut BTreeMap<String, PlannedMetricWorkset>,
    dataframe_tasks: &mut BTreeMap<String, PlannedDataframeArtifact>,
) -> Result<()> {
    for request in all_warmup_requests {
        if !home_embedded_warmup_request_matches_scope(request, scope) {
            continue;
        }
        if is_world_metrics_warmup_dataset(request.dataset_id.as_str()) {
            continue;
        }
        if mei_lang_kernel::locate_dataset_resource(
            &planning_outcome.compiled,
            request.dataset_id.as_str(),
        )
        .is_err()
        {
            tracing::debug!(
                app_id = %app_id,
                dataset_id = %request.dataset_id,
                scope = %scope.key(),
                "skip home embedded warmup artifact plan: dataset not locateable after MCG hydrate"
            );
            continue;
        }
        collect_request_artifact_plans(
            app_id,
            app_root,
            planning_outcome,
            request.dataset_id.as_str(),
            request.metric_ids.as_slice(),
            metric_worksets,
            dataframe_tasks,
        )?;
    }
    Ok(())
}

pub(crate) fn build_scope_artifact_plan(
    source_root: &Path,
    app_id: &str,
    app_root: &Path,
    scope: &CompileScope,
    outcome: &SharedCompileOutcome,
    requests: &[&AggregatedWarmupRequest],
    scope_profile: PrebuildScopeProfile,
    all_warmup_requests: &[AggregatedWarmupRequest],
) -> Result<ScopeArtifactPlan> {
    let mut metric_worksets = BTreeMap::<String, PlannedMetricWorkset>::new();
    let mut dataframe_tasks = BTreeMap::<String, PlannedDataframeArtifact>::new();
    for request in requests {
        collect_request_artifact_plans(
            app_id,
            app_root,
            outcome,
            request.dataset_id.as_str(),
            request.metric_ids.as_slice(),
            &mut metric_worksets,
            &mut dataframe_tasks,
        )?;
    }
    if scope.key() == CompileScope::default_scope().key()
        || outcome
            .compiled
            .active_target_file
            .contains("home.mei")
    {
        let mut planning_compiled = (*outcome.compiled).clone();
        let _ = crate::graph::hydrate_compiled_for_prebuild_eval(
            source_root,
            app_id,
            &mut planning_compiled,
            &[],
            &[],
        );
        let planning_outcome = SharedCompileOutcome {
            compiled: Arc::new(planning_compiled),
            ..outcome.clone()
        };
        if scope_profile == PrebuildScopeProfile::HotOnly {
            let mut owners = BTreeMap::<String, Vec<String>>::new();
            for request in all_warmup_requests {
                let owner = request.dataset_id.trim();
                if owner.is_empty() || !is_world_metrics_warmup_dataset(owner) {
                    continue;
                }
                owners
                    .entry(owner.to_string())
                    .or_default()
                    .extend(request.metric_ids.iter().cloned());
            }
            for (owner, mut metric_ids) in owners {
                metric_ids.sort_unstable();
                metric_ids.dedup();
                if mei_lang_kernel::locate_dataset_resource(
                    &planning_outcome.compiled,
                    owner.as_str(),
                )
                .is_err()
                {
                    tracing::debug!(
                        app_id = %app_id,
                        owner = %owner,
                        "skip hot-only world_metrics artifact plan: owner not locateable after MCG hydrate"
                    );
                    continue;
                }
                collect_request_artifact_plans(
                    app_id,
                    app_root,
                    &planning_outcome,
                    owner.as_str(),
                    metric_ids.as_slice(),
                    &mut metric_worksets,
                    &mut dataframe_tasks,
                )?;
            }
            collect_home_embedded_warmup_artifact_plans(
                app_id,
                app_root,
                scope,
                &planning_outcome,
                all_warmup_requests,
                &mut metric_worksets,
                &mut dataframe_tasks,
            )?;
        } else {
            let mut owners = crate::graph::discover_world_metrics_owner_ids(
                source_root,
                app_id,
                &planning_outcome.compiled,
            );
            if owners.is_empty()
                && compiled_has_world_metrics_runtime_defs(&planning_outcome.compiled)
            {
                owners.insert("__world_metrics__".to_string());
            }
            for owner in owners {
                if mei_lang_kernel::locate_dataset_resource(
                    &planning_outcome.compiled,
                    owner.as_str(),
                )
                .is_err()
                {
                    tracing::debug!(
                        app_id = %app_id,
                        owner = %owner,
                        "skip home embedded world_metrics artifact plan: owner not locateable after MCG hydrate"
                    );
                    continue;
                }
                collect_request_artifact_plans(
                    app_id,
                    app_root,
                    &planning_outcome,
                    owner.as_str(),
                    &[],
                    &mut metric_worksets,
                    &mut dataframe_tasks,
                )?;
            }
            collect_home_embedded_warmup_artifact_plans(
                app_id,
                app_root,
                scope,
                &planning_outcome,
                all_warmup_requests,
                &mut metric_worksets,
                &mut dataframe_tasks,
            )?;
        }
    }
    Ok(ScopeArtifactPlan {
        metric_worksets: metric_worksets.into_values().collect(),
        dataframe_artifacts: dataframe_tasks.into_values().collect(),
    })
}

pub(crate) fn build_plan_node_stats(
    manifest_plan: &PrebuildManifestPlan,
    canonical_identity_count: usize,
    scope_plans: &[ScopeArtifactPlan],
) -> PrebuildPlanNodeStatsReport {
    let mut metric_workset_nodes = BTreeSet::new();
    let mut response_nodes = BTreeSet::new();
    let mut dataframe_nodes = BTreeSet::new();
    let mut warmup_scope_nodes = BTreeSet::new();
    let mut logical_workset_nodes = BTreeSet::new();
    let mut logical_dataframe_nodes = BTreeSet::new();
    let mut scope_ids = BTreeSet::new();
    let mut dependency_keys = BTreeSet::new();
    for request in &manifest_plan.warmup_requests {
        warmup_scope_nodes.insert(request.scope.key());
    }
    for plan in scope_plans {
        for workset in &plan.metric_worksets {
            logical_workset_nodes.insert(workset.logical_node_id.clone());
            scope_ids.insert(workset.scope_id.clone());
            dependency_keys.insert(workset.dependency_revision_key.clone());
            metric_workset_nodes.insert(workset.materialization_key.clone());
            response_nodes.insert(workset.response_cache_key.clone());
        }
        for dataframe in &plan.dataframe_artifacts {
            logical_dataframe_nodes.insert(dataframe.logical_node_id.clone());
            scope_ids.insert(dataframe.scope_id.clone());
            dependency_keys.insert(dataframe.dependency_revision_key.clone());
            let _ = dataframe.scope_metric_token.as_str();
            dataframe_nodes.insert(dataframe.artifact_key.clone());
        }
    }
    let _ = (logical_workset_nodes.len(), logical_dataframe_nodes.len(), scope_ids.len(), dependency_keys.len());
    let canonical_prebuild_nodes = canonical_identity_count + metric_workset_nodes.len();
    let budget = PrebuildNodeBudgetReport {
        canonical_node_limit: CANONICAL_PREBUILD_NODE_BUDGET,
        startup_wall_ms_limit: STARTUP_PREBUILD_WALL_MS_BUDGET_MS,
        over_canonical_node_limit: canonical_prebuild_nodes > CANONICAL_PREBUILD_NODE_BUDGET,
    };
    let planned_total_nodes = canonical_prebuild_nodes
        + manifest_plan.warmup_requests.len()
        + dataframe_nodes.len();
    PrebuildPlanNodeStatsReport {
        manifest_compile_scope_nodes: 1
            + manifest_plan.hot_scopes.len()
            + manifest_plan.deferred_scopes.len(),
        hot_compile_scope_nodes: manifest_plan.hot_scopes.len(),
        deferred_compile_scope_nodes: manifest_plan.deferred_scopes.len(),
        planned_warmup_request_nodes: manifest_plan.warmup_requests.len(),
        planned_warmup_scope_nodes: warmup_scope_nodes.len(),
        planned_metric_workset_nodes: metric_workset_nodes.len(),
        planned_response_artifact_nodes: response_nodes.len(),
        planned_dataframe_artifact_nodes: dataframe_nodes.len(),
        planned_total_nodes,
        canonical_prebuild_nodes,
        budget,
    }
}

pub(crate) fn current_bundle_revision_for_plan(
    plan: &PlannedMetricWorkset,
    mcg_revisions: &BTreeMap<String, String>,
) -> Option<String> {
    crate::graph::resolve_metric_bundle_revision(
        mcg_revisions,
        plan.owner_resource_id.as_str(),
        plan.defs_for_hydrate.as_ref(),
    )
}

pub(crate) fn current_dataframe_bundle_revision(
    plan: &PlannedDataframeArtifact,
    mcg_revisions: &BTreeMap<String, String>,
) -> Option<String> {
    crate::graph::resolve_metric_bundle_revision(
        mcg_revisions,
        plan.owner_resource_id.as_str(),
        plan.defs_for_hydrate.as_ref(),
    )
}

pub(crate) fn slot_cache_key_for_plan(
    plan: &PlannedMetricWorkset,
    bundle_revision: &str,
) -> String {
    crate::graph::canonical_slot_cache_key_for_workset(
        plan.owner_resource_id.as_str(),
        plan.scene_id.as_str(),
        plan.scene_path.as_deref(),
        bundle_revision,
        plan.dependency_revision_key.as_str(),
    )
}

pub(crate) fn promote_prebuild_metric_response_slot(
    source_root: Option<&Path>,
    app_id: Option<&str>,
    plan: &PlannedMetricWorkset,
    bundle_revision: &str,
) {
    let (Some(source_root), Some(app_id)) = (source_root, app_id) else {
        return;
    };
    let scope_key =
        crate::graph::mrg_eval_scope_key(plan.scene_id.as_str(), plan.scene_path.as_deref());
    let workset_id = format!(
        "workset|app={app_id}|owner={}|metrics={}",
        plan.owner_resource_id,
        if plan.request_all_metrics {
            "*".to_string()
        } else {
            plan.requested_metric_ids.join(",")
        }
    );
    if let Err(error) = crate::graph::mrg::slots::record_mrg_slot_after_eval(
        source_root,
        app_id,
        workset_id.as_str(),
        scope_key.as_str(),
        plan.owner_resource_id.as_str(),
        bundle_revision,
        plan.dependency_revision_key.as_str(),
        slot_cache_key_for_plan(plan, bundle_revision).as_str(),
        "eval-results/results/metric-response/",
        0,
        true,
    ) {
        tracing::warn!(
            app_id = %app_id,
            owner = %plan.owner_resource_id,
            error = %error,
            "failed to promote MRG slot from existing metric response artifact"
        );
    }
}

#[cfg(test)]
mod home_embedded_warmup_tests {
    use super::*;

    #[test]
    fn home_route_scope_matches_embedded_capsule_warmup_without_exact_target() {
        let home_scope = CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: Some("scenes/home.mei".to_string()),
        };
        let embedded_request = AggregatedWarmupRequest {
            scope: CompileScope {
                requested_scene_id: Some("home".to_string()),
                requested_target_file: Some("scenes/09-监督典型案例.mei".to_string()),
            },
            dataset_id: "scenes/09-监督典型案例.mei::typical_cases".to_string(),
            priority: WarmupRequestPriority::Critical,
            metric_ids: vec![
                "case_count".to_string(),
                "case_highlights".to_string(),
            ],
        };
        assert!(home_embedded_warmup_request_matches_scope(
            &embedded_request,
            &home_scope
        ));
    }

    #[test]
    fn home_route_scope_does_not_match_focused_warning_list_warmup() {
        let home_scope = CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: Some("scenes/home.mei".to_string()),
        };
        let warning_list_request = AggregatedWarmupRequest {
            scope: CompileScope {
                requested_scene_id: Some("home".to_string()),
                requested_target_file: Some("scenes/06-实时预警.mei".to_string()),
            },
            dataset_id: "warning_list".to_string(),
            priority: WarmupRequestPriority::Critical,
            metric_ids: vec!["warnings_realtime_cockpit_table".to_string()],
        };
        assert!(!home_embedded_warmup_request_matches_scope(
            &warning_list_request,
            &home_scope
        ));
    }

    #[test]
    fn non_home_scope_does_not_match_embedded_capsule_warmup() {
        let board_scope = CompileScope {
            requested_scene_id: Some("typical_cases_detail_board".to_string()),
            requested_target_file: Some("scenes/09-监督典型案例.board.mei".to_string()),
        };
        let embedded_request = AggregatedWarmupRequest {
            scope: CompileScope {
                requested_scene_id: Some("home".to_string()),
                requested_target_file: Some("scenes/09-监督典型案例.mei".to_string()),
            },
            dataset_id: "scenes/09-监督典型案例.mei::typical_cases".to_string(),
            priority: WarmupRequestPriority::Critical,
            metric_ids: vec!["case_highlights".to_string()],
        };
        assert!(!home_embedded_warmup_request_matches_scope(
            &embedded_request,
            &board_scope
        ));
    }
}

