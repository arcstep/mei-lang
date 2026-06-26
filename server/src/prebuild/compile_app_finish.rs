use super::prelude::*;
use super::*;

pub(crate) struct PrebuildAppAfterCompile {
    pub app_started: Instant,
    pub app_root: PathBuf,
    pub components_root: PathBuf,
    pub diagnostics: Arc<PrebuildDiagnostics>,
    pub manifest_plan: PrebuildManifestPlan,
    pub warmup_requests: Vec<AggregatedWarmupRequest>,
    pub max_parallelism: usize,
    pub pre_mcg_bundle_revisions: BTreeMap<String, String>,
    pub initial_scope_count: usize,
    pub compile_scopes_ms: u64,
    pub compile_reports: Vec<PrebuildScopeReport>,
    pub prepared_outcomes: Vec<PreparedCompileOutcome>,
    pub compile_session: Arc<Mutex<PrebuildCompileSession>>,
    pub warnings: Vec<PrebuildWarningReport>,
}

pub(crate) fn finish_run_prebuild_for_app(
    source_root: &Path,
    app: &RuntimeWarmupApp,
    mode: PrebuildMode,
    ctx: PrebuildAppAfterCompile,
) -> Result<PrebuildAppReport> {
    let PrebuildAppAfterCompile {
        app_started,
        app_root,
        components_root,
        diagnostics,
        manifest_plan,
        warmup_requests,
        max_parallelism,
        pre_mcg_bundle_revisions,
        initial_scope_count,
        compile_scopes_ms,
        compile_reports,
        prepared_outcomes,
        compile_session,
        mut warnings,
    } = ctx;

    let required_xlsx_sources = collect_required_xlsx_sources(
        app,
        unique_prepared_outcomes_for_artifacts(&prepared_outcomes)
            .iter()
            .map(|prepared| prepared.outcome.compiled.as_ref()),
    );
    let snapshot_started = Instant::now();
    let data_snapshots = match mode {
        PrebuildMode::Build => Some(publish_required_data_snapshots(
            source_root,
            app.app_id.as_str(),
            required_xlsx_sources.iter().cloned().collect(),
        )?),
        PrebuildMode::Verify => None,
    };
    let data_snapshots_ms = snapshot_started.elapsed().as_millis() as u64;
    verify_required_xlsx_sources(app_root.as_path(), &required_xlsx_sources)?;
    let mut coverage = PrebuildCoverageReport::default();
    coverage.dataset_import_artifacts_planned = required_xlsx_sources.len();
    coverage.dataset_import_artifacts_ready = required_xlsx_sources.len();
    let _ = mei_lang_kernel::clear_runtime_eval_node_cache();
    let coverage_state = CoverageState {
        diagnostics: Arc::clone(&diagnostics),
        pre_mcg_bundle_revisions,
        source_root: Some(source_root.to_path_buf()),
        app_id: Some(app.app_id.clone()),
        ..CoverageState::default()
    };
    let artifact_outcomes = unique_prepared_outcomes_for_artifacts(&prepared_outcomes);
    let canonical_identity_count = artifact_outcomes.len();
    let session_entries_before_clear = if let Ok(session) = compile_session.lock() {
        (
            session.by_scope_key.len(),
            session.by_compile_cache_key.len(),
            session.by_identity.len(),
        )
    } else {
        (0, 0, 0)
    };
    let session_entries_after_clear = if let Ok(mut session) = compile_session.lock() {
        session.clear_runtime_maps();
        (
            session.by_scope_key.len(),
            session.by_compile_cache_key.len(),
            session.by_identity.len(),
        )
    } else {
        (0, 0, 0)
    };
    drop(compile_session);
    drop(prepared_outcomes);
    if std::env::var("MEI_PREBUILD_EVICTION")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .is_none_or(|value| !matches!(value.as_str(), "0" | "false" | "no" | "off"))
        && crate::graph::feature::graph_registry_dedup_enabled()
    {
        let _ = toolchain::clear_compile_cache_for_app(source_root, app.app_id.as_str());
    }
    let artifact_outcomes_for_warmup = artifact_outcomes.clone();
    let mut scope_artifact_plans = Vec::with_capacity(artifact_outcomes_for_warmup.len());
    for prepared in &artifact_outcomes_for_warmup {
        let matching_requests = matching_warmup_requests_for_outcome(&warmup_requests, &prepared.outcome);
        scope_artifact_plans.push(build_scope_artifact_plan(
            app.app_id.as_str(),
            app_root.as_path(),
            &prepared.scope,
            &prepared.outcome,
            matching_requests.as_slice(),
        )?);
    }
    let plan_nodes = build_plan_node_stats(
        &manifest_plan,
        canonical_identity_count,
        scope_artifact_plans.as_slice(),
    );
    coverage.compile_artifacts_planned = initial_scope_count;
    coverage.compile_artifacts_ready = compile_reports.len();
    coverage.metric_response_artifacts_planned = plan_nodes.planned_response_artifact_nodes;
    coverage.metric_dataframe_artifacts_planned = plan_nodes.planned_dataframe_artifact_nodes;
    let scope_artifacts_started = Instant::now();
    prebuild_emit_progress(&format!(
        "[{}] ── 2/3 生成 metric 产物 ── {} 个编译结果待处理（response + dataframe 落盘）",
        app.app_id,
        artifact_outcomes.len()
    ));
    let artifact_total = artifact_outcomes.len();
    let artifacts_started = Arc::new(Instant::now());
    let scope_results = run_limited_parallel_ordered_with_hook(
        artifact_outcomes
            .into_iter()
            .zip(scope_artifact_plans.clone())
            .collect(),
        max_parallelism,
        |(prepared, scope_plan)| {
            let mut local_coverage = PrebuildCoverageReport::default();
            let started = Instant::now();
            let result = ensure_scope_artifacts(
                app.app_id.as_str(),
                app_root.as_path(),
                &prepared.outcome,
                &scope_plan,
                mode,
                &mut local_coverage,
                &coverage_state,
            );
            (
                prepared.scope.clone(),
                result,
                local_coverage,
                started.elapsed(),
            )
        },
        {
            let app_id = app.app_id.clone();
            let done = Arc::new(AtomicUsize::new(0));
            let artifacts_started = Arc::clone(&artifacts_started);
            move |index,
                  (scope, result, local_coverage, wall_time): &(
                CompileScope,
                Result<()>,
                PrebuildCoverageReport,
                std::time::Duration,
            )| {
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                let scene = scope.requested_scene_id.clone().unwrap_or_default();
                let target = scope.requested_target_file.clone().unwrap_or_default();
                let file = format_scope_file(scene.as_str(), target.as_str(), None);
                let built_df = local_coverage.metric_dataframe_artifacts_built;
                let built_resp = local_coverage.metric_response_artifacts_built;
                if built_df > 0 || built_resp > 0 {
                    prebuild_emit_progress(format!(
                        "[{app_id}] 指标产物 {:.1}s | {n}/{artifact_total} | scene={scene} | file={file} | +{built_df} dataframe +{built_resp} response",
                        wall_time.as_secs_f64()
                    ));
                } else if result.is_err() {
                    prebuild_emit_progress(format!(
                        "[{app_id}] 指标产物失败 | {n}/{artifact_total} | scene={scene} | file={file}"
                    ));
                } else if n % 20 == 0 || n == artifact_total {
                    prebuild_emit_progress(format!(
                        "[{app_id}] 指标产物进度 {n}/{artifact_total} | 已用 {:.0}s（多数命中磁盘缓存）",
                        artifacts_started.elapsed().as_secs_f64()
                    ));
                }
                let _ = index;
            }
        },
    );
    for (scope, result, local_coverage, _wall_time) in scope_results {
        if let Err(error) = result {
            if mode == PrebuildMode::Verify {
                return Err(error);
            }
            warnings.push(build_prebuild_warning(
                "scope_artifacts",
                scope.requested_scene_id.as_deref(),
                scope.requested_target_file.as_deref(),
                None,
                None,
                None,
                None,
                error.to_string(),
            ));
        } else {
            merge_coverage(&mut coverage, &local_coverage);
        }
    }
    let scope_artifacts_ms = scope_artifacts_started.elapsed().as_millis() as u64;
    prebuild_emit_progress(&format!(
        "[{}] ── 2/3 产物完成 {:.1}s | response={} dataframe={} | 新建 dataframe {} 个 ──",
        app.app_id,
        scope_artifacts_ms as f64 / 1000.0,
        coverage.metric_response_artifacts_ready,
        coverage.metric_dataframe_artifacts_ready,
        coverage.metric_dataframe_artifacts_built
    ));
    coverage_state.clear();
    let _ = mei_lang_datasets::clear_all_metric_caches();
    let _ = mei_lang_kernel::clear_runtime_eval_node_cache();
    let warmup_reuse_hits = warmup_requests
        .iter()
        .filter(|request| {
            artifact_outcomes_for_warmup
                .iter()
                .any(|prepared| warmup_request_matches_outcome(request, &prepared.outcome))
        })
        .count();
    let warmup_requests_to_run = warmup_requests
        .iter()
        .filter(|request| {
            !artifact_outcomes_for_warmup
                .iter()
                .any(|prepared| warmup_request_matches_outcome(request, &prepared.outcome))
        })
        .collect::<Vec<_>>();
    let mut critical_warmup_requests = Vec::new();
    let mut deferred_warmup_requests = Vec::new();
    let critical_warmup_cache_hit_count = warmup_requests
        .iter()
        .filter(|request| {
            request.priority == WarmupRequestPriority::Critical
                && artifact_outcomes_for_warmup
                    .iter()
                    .any(|prepared| warmup_request_matches_outcome(request, &prepared.outcome))
        })
        .count();
    let deferred_warmup_cache_hit_count = warmup_requests
        .iter()
        .filter(|request| {
            request.priority == WarmupRequestPriority::Deferred
                && artifact_outcomes_for_warmup
                    .iter()
                    .any(|prepared| warmup_request_matches_outcome(request, &prepared.outcome))
        })
        .count();
    for request in warmup_requests_to_run {
        match request.priority {
            WarmupRequestPriority::Critical => critical_warmup_requests.push(request),
            WarmupRequestPriority::Deferred => deferred_warmup_requests.push(request),
        }
    }
    let critical_warmup_request_count = critical_warmup_requests.len();
    let deferred_warmup_request_count = deferred_warmup_requests.len();
    let critical_warmup_total_count =
        critical_warmup_request_count + critical_warmup_cache_hit_count;
    let deferred_warmup_total_count =
        deferred_warmup_request_count + deferred_warmup_cache_hit_count;
    let mut critical_warmup_ok = true;
    let mut deferred_warmup_ok = true;
    let run_and_merge_warmup = |label: &str,
                                requests: &[&AggregatedWarmupRequest],
                                ok_flag: &mut bool,
                                warnings: &mut Vec<PrebuildWarningReport>,
                                coverage: &mut PrebuildCoverageReport|
     -> Result<u64> {
        if requests.is_empty() {
            return Ok(0);
        }
        prebuild_emit_progress(&format!(
            "[{}] ── 3/3 warmup {label} ── {} requests ──",
            app.app_id,
            requests.len()
        ));
        let started = Instant::now();
        let results = run_warmup_request_batch(
            source_root,
            app.app_id.as_str(),
            app_root.as_path(),
            mode,
            components_root.as_path(),
            &coverage_state,
            requests,
            max_parallelism,
        );
        for (scope, dataset_results, local_coverage) in results {
            let scope = CompileScope {
                requested_scene_id: scope.requested_scene_id.clone(),
                requested_target_file: scope.requested_target_file.clone(),
            };
            for (dataset_id, result) in dataset_results {
                if let Err(error) = result {
                    *ok_flag = false;
                    if mode == PrebuildMode::Verify {
                        return Err(error);
                    }
                    warnings.push(build_prebuild_warning(
                        &format!("warmup_{label}"),
                        scope.requested_scene_id.as_deref(),
                        scope.requested_target_file.as_deref(),
                        Some(dataset_id.as_str()),
                        None,
                        None,
                        None,
                        error.to_string(),
                    ));
                }
            }
            merge_coverage(coverage, &local_coverage);
        }
        Ok(started.elapsed().as_millis() as u64)
    };
    let critical_warmup_requests_ms = run_and_merge_warmup(
        "critical",
        critical_warmup_requests.as_slice(),
        &mut critical_warmup_ok,
        &mut warnings,
        &mut coverage,
    )?;
    let deferred_warmup_requests_ms = run_and_merge_warmup(
        "deferred",
        deferred_warmup_requests.as_slice(),
        &mut deferred_warmup_ok,
        &mut warnings,
        &mut coverage,
    )?;
    coverage_state.clear();
    let _ = mei_lang_datasets::clear_all_metric_caches();
    let _ = mei_lang_kernel::clear_runtime_eval_node_cache();
    finalize_coverage_report(&mut coverage);
    if mode == PrebuildMode::Verify && coverage.total_missing_artifacts > 0 {
        anyhow::bail!(
            "prebuild coverage verify failed: missing artifacts total={} compile={} dataset_import={} metric_response={} metric_dataframe={}",
            coverage.total_missing_artifacts,
            coverage.compile_artifacts_missing,
            coverage.dataset_import_artifacts_missing,
            coverage.metric_response_artifacts_missing,
            coverage.metric_dataframe_artifacts_missing
        );
    }
    let warmup_requests_ms = critical_warmup_requests_ms + deferred_warmup_requests_ms;
    if let Err(error) =
        mei_lang_datasets::preload_prebuild_metric_response_index(app_root.as_path())
    {
        warnings.push(build_prebuild_warning(
            "post_prebuild",
            None,
            None,
            None,
            None,
            None,
            None,
            format!("metric response index preload failed: {error}"),
        ));
    }
    let diagnostics_report = build_prebuild_diagnostics_report(
        app_root.as_path(),
        compile_reports.as_slice(),
        diagnostics.as_ref(),
        plan_nodes.clone(),
        canonical_identity_count,
        session_entries_before_clear,
        session_entries_after_clear,
        warmup_reuse_hits,
        critical_warmup_total_count,
        critical_warmup_request_count,
        critical_warmup_cache_hit_count,
        critical_warmup_requests_ms,
        critical_warmup_ok,
        deferred_warmup_total_count,
        deferred_warmup_request_count,
        deferred_warmup_cache_hit_count,
        deferred_warmup_requests_ms,
        deferred_warmup_ok,
    );
    emit_prebuild_optimization_report(
        app.app_id.as_str(),
        app_root.as_path(),
        compile_reports.as_slice(),
        &coverage,
        diagnostics.as_ref(),
        &plan_nodes,
        compile_scopes_ms,
        scope_artifacts_ms,
        max_parallelism,
        warnings.len(),
        canonical_identity_count,
        session_entries_before_clear,
        session_entries_after_clear,
        warmup_reuse_hits,
    );
    let summary = crate::diagnostics::LastBuildSummary::from_prebuild_diagnostics(
        app.app_id.as_str(),
        &diagnostics_report,
    );
    if let Err(error) =
        crate::diagnostics::persist_last_build_summary(app_root.as_path(), &summary)
    {
        tracing::warn!(
            %error,
            app_id = %app.app_id,
            "failed to persist last build summary"
        );
    }
    Ok(PrebuildAppReport {
        app_id: app.app_id.clone(),
        compile_scopes: compile_reports,
        coverage,
        timings: PrebuildTimingReport {
            total_wall_ms: app_started.elapsed().as_millis() as u64,
            compile_scopes_ms,
            data_snapshots_ms,
            scope_artifacts_ms,
            warmup_requests_ms,
            critical_warmup_requests_ms,
            deferred_warmup_requests_ms,
            critical_warmup_request_count,
            deferred_warmup_request_count,
            max_parallelism,
        },
        data_snapshots,
        diagnostics: diagnostics_report,
        warnings,
    })
}

