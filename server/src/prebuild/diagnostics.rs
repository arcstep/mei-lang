use super::prelude::*;
use super::*;

fn phase_rss_snapshot(atom: &AtomicUsize) -> Option<u64> {
    let value = atom.load(Ordering::Relaxed) as u64;
    if value > 0 {
        Some(value)
    } else {
        None
    }
}

pub(crate) fn build_prebuild_diagnostics_report(
    app_root: &Path,
    reports: &[PrebuildScopeReport],
    diagnostics: &PrebuildDiagnostics,
    plan_nodes: PrebuildPlanNodeStatsReport,
    canonical_identity_count: usize,
    session_entries_before_clear: (usize, usize, usize, usize),
    session_entries_after_clear: (usize, usize, usize, usize),
    warmup_reuse_hits: usize,
    critical_warmup_total_count: usize,
    critical_warmup_executed_count: usize,
    critical_warmup_cache_hit_count: usize,
    critical_warmup_ms: u64,
    critical_warmup_ok: bool,
    deferred_warmup_total_count: usize,
    deferred_warmup_executed_count: usize,
    deferred_warmup_cache_hit_count: usize,
    deferred_warmup_ms: u64,
    deferred_warmup_ok: bool,
) -> PrebuildDiagnosticsReport {
    let total_scope_checks = reports.len();
    let assemble_only_count = reports.iter().filter(|report| report.assemble_only).count();
    let real_compile_count = reports
        .iter()
        .filter(|report| !report.cache_hit && !report.assemble_only)
        .count();
    let cache_hit_count = reports.iter().filter(|report| report.cache_hit).count();
    let cache_probe_ms: u64 = reports
        .iter()
        .filter(|report| report.cache_hit)
        .map(|report| {
            report
                .cache_lookup_ms
                .saturating_add(report.artifact_load_ms)
        })
        .sum();
    let compile_miss_ms: u64 = reports
        .iter()
        .filter(|report| !report.cache_hit)
        .map(|report| report.compile_ms)
        .sum();
    let unique_compile_result_count = reports
        .iter()
        .map(compile_active_identity)
        .collect::<BTreeSet<_>>()
        .len();
    let redundant_scope_checks = total_scope_checks.saturating_sub(unique_compile_result_count);
    let expansion_ratio = if unique_compile_result_count > 0 {
        total_scope_checks as f64 / unique_compile_result_count as f64
    } else {
        1.0
    };
    let preload_reuse_hits = diagnostics
        .compile_preload_reuse_hits
        .load(Ordering::Relaxed);
    let postload_identity_collapses = diagnostics
        .compile_postload_identity_collapses
        .load(Ordering::Relaxed);
    let compile_index_hits = diagnostics.compile_index_hits.load(Ordering::Relaxed);
    let compile_index_misses = diagnostics.compile_index_misses.load(Ordering::Relaxed);
    let compile_index_stale_entries = diagnostics
        .compile_index_stale_entries
        .load(Ordering::Relaxed);
    let compile_fallback_loads = diagnostics.compile_fallback_loads.load(Ordering::Relaxed);
    let manifest_probes = diagnostics.compile_manifest_probes.load(Ordering::Relaxed);
    let manifest_stale_skips = diagnostics
        .compile_manifest_stale_skips
        .load(Ordering::Relaxed);
    let artifact_loads_avoided = diagnostics
        .compile_artifact_loads_avoided
        .load(Ordering::Relaxed);
    let mrg_eval_skips = diagnostics.mrg_eval_skips.load(Ordering::Relaxed);
    let dataframe_eval_skips = diagnostics.dataframe_eval_skips.load(Ordering::Relaxed);
    let target_overlay_reuse_hits = diagnostics
        .compile_target_overlay_reuse_hits
        .load(Ordering::Relaxed);
    let mcg_assemble_only_count = diagnostics
        .mcg_assemble_only_count
        .load(Ordering::Relaxed);
    let session_peak_identity_entries = diagnostics
        .session_peak_identity_entries
        .load(Ordering::Relaxed);
    let hydrate_reuse_hits = diagnostics.hydrate_reuse_hits.load(Ordering::Relaxed);
    let eval_root = mei_lang_kernel::resolve_app_var_root(app_root).join("eval-results");
    let response_dir = eval_root.join("results").join("metric-response");
    let dataframe_dir = eval_root.join("results").join("metric-dataframe");
    let current_rss_bytes = current_process_rss_bytes();
    let peak_rss_bytes = diagnostics.peak_rss_bytes.load(Ordering::Relaxed) as u64;
    let orchestrator_peak_rss_bytes = peak_rss_bytes;
    let worker_peak_rss_bytes = diagnostics.worker_peak_rss_bytes.load(Ordering::Relaxed) as u64;
    let empty_binary_baseline_bytes = {
        let baseline = diagnostics.empty_binary_baseline_bytes.load(Ordering::Relaxed) as u64;
        if baseline > 0 {
            Some(baseline)
        } else {
            None
        }
    };
    let rss_after_compile_bytes = phase_rss_snapshot(&diagnostics.rss_after_compile_bytes);
    let rss_after_artifacts_bytes = phase_rss_snapshot(&diagnostics.rss_after_artifacts_bytes);
    let rss_after_warmup_bytes = phase_rss_snapshot(&diagnostics.rss_after_warmup_bytes);
    let graph_working_set_peak_bytes = {
        let baseline = empty_binary_baseline_bytes.unwrap_or(0);
        let orchestrator_delta = orchestrator_peak_rss_bytes.saturating_sub(baseline);
        if worker_peak_rss_bytes > 0 {
            Some(worker_peak_rss_bytes)
        } else if orchestrator_delta > 0 {
            Some(orchestrator_delta)
        } else {
            None
        }
    };
    let content_store_root = app_root.join("build").join("active").join("store");
    let var_active_root = mei_lang_kernel::resolve_app_var_root(app_root).join("active");
    let logical_disk_bytes = {
        let content_bytes = dir_size_summary(content_store_root.as_path()).bytes;
        let var_bytes = dir_size_summary(var_active_root.as_path()).bytes;
        let total = content_bytes.saturating_add(var_bytes);
        if total > 0 {
            Some(total)
        } else {
            None
        }
    };

    let mut slow_scopes = reports
        .iter()
        .filter(|report| !report.cache_hit && report.compile_ms > 0)
        .map(|report| PrebuildSlowScopeDiagnostic {
            scene_id: report
                .requested_scene_id
                .clone()
                .or(report.active_scene_id.clone()),
            target_file: report
                .requested_target_file
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| report.active_target_file.clone()),
            compile_ms: report.compile_ms,
        })
        .collect::<Vec<_>>();
    slow_scopes.sort_by_key(|entry| std::cmp::Reverse(entry.compile_ms));
    slow_scopes.truncate(8);

    let mut slow_metrics = diagnostics
        .metric_builds
        .lock()
        .expect("lock prebuild diagnostics")
        .iter()
        .map(|entry| PrebuildSlowMetricDiagnostic {
            kind: entry.kind.to_string(),
            dataset: entry.dataset.clone(),
            metric: entry.metric.clone(),
            scene: entry.scene.clone(),
            ms: entry.ms,
        })
        .collect::<Vec<_>>();
    slow_metrics.sort_by_key(|entry| std::cmp::Reverse(entry.ms));
    slow_metrics.truncate(8);

    PrebuildDiagnosticsReport {
        total_scope_checks,
        real_compile_count,
        assemble_only_count,
        cache_hit_count,
        unique_compile_result_count,
        canonical_identity_count,
        redundant_scope_checks,
        expansion_ratio,
        cache_probe_ms,
        compile_miss_ms,
        current_rss_bytes,
        peak_rss_bytes,
        orchestrator_peak_rss_bytes,
        worker_peak_rss_bytes,
        empty_binary_baseline_bytes,
        rss_after_compile_bytes,
        rss_after_artifacts_bytes,
        rss_after_warmup_bytes,
        graph_working_set_peak_bytes,
        logical_disk_bytes,
        session_peak_identity_entries,
        hydrate_reuse_hits,
        eval_artifacts_disk: PrebuildEvalArtifactDiskReport {
            total: disk_usage_report(dir_size_summary(eval_root.as_path())),
            metric_response: disk_usage_report(dir_size_summary(response_dir.as_path())),
            metric_dataframe: disk_usage_report(dir_size_summary(dataframe_dir.as_path())),
        },
        compile_index: PrebuildCompileIndexStatsReport {
            preload_reuse_hits,
            postload_identity_collapses,
            hits: compile_index_hits,
            misses: compile_index_misses,
            stale_entries: compile_index_stale_entries,
            fallback_loads: compile_fallback_loads,
            manifest_probes,
            manifest_stale_skips,
            artifact_loads_avoided,
            mrg_eval_skips,
            dataframe_eval_skips,
            target_overlay_reuse_hits,
            mcg_assemble_only_count,
        },
        session_before_clear: PrebuildSessionEntryStatsReport {
            scope_entries: session_entries_before_clear.0,
            cache_entries: session_entries_before_clear.1,
            identity_entries: session_entries_before_clear.2,
            target_entries: session_entries_before_clear.3,
        },
        session_after_clear: PrebuildSessionEntryStatsReport {
            scope_entries: session_entries_after_clear.0,
            cache_entries: session_entries_after_clear.1,
            identity_entries: session_entries_after_clear.2,
            target_entries: session_entries_after_clear.3,
        },
        warmup_reuse_hits,
        plan_nodes,
        critical_warmup: PrebuildWarmupDiagnosticReport {
            total_request_count: critical_warmup_total_count,
            executed_request_count: critical_warmup_executed_count,
            cache_hit_count: critical_warmup_cache_hit_count,
            total_ms: critical_warmup_ms,
            ok: critical_warmup_ok,
        },
        deferred_warmup: PrebuildWarmupDiagnosticReport {
            total_request_count: deferred_warmup_total_count,
            executed_request_count: deferred_warmup_executed_count,
            cache_hit_count: deferred_warmup_cache_hit_count,
            total_ms: deferred_warmup_ms,
            ok: deferred_warmup_ok,
        },
        slow_scopes,
        slow_metrics,
        fingerprint_skip: false,
        inputs_fingerprint: None,
        plan_source: None,
        dirty_slot_count: None,
    }
}

pub(crate) fn aggregate_prebuild_diagnostics(apps: &[PrebuildAppReport]) -> PrebuildDiagnosticsReport {
    let mut aggregate = PrebuildDiagnosticsReport::default();
    let mut slow_scopes = Vec::new();
    let mut slow_metrics = Vec::new();
    for app in apps {
        let diagnostics = &app.diagnostics;
        aggregate.total_scope_checks += diagnostics.total_scope_checks;
        aggregate.real_compile_count += diagnostics.real_compile_count;
        aggregate.cache_hit_count += diagnostics.cache_hit_count;
        aggregate.unique_compile_result_count += diagnostics.unique_compile_result_count;
        aggregate.canonical_identity_count += diagnostics.canonical_identity_count;
        aggregate.redundant_scope_checks += diagnostics.redundant_scope_checks;
        aggregate.cache_probe_ms = aggregate
            .cache_probe_ms
            .saturating_add(diagnostics.cache_probe_ms);
        aggregate.compile_miss_ms = aggregate
            .compile_miss_ms
            .saturating_add(diagnostics.compile_miss_ms);
        aggregate.current_rss_bytes =
            match (aggregate.current_rss_bytes, diagnostics.current_rss_bytes) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            };
        aggregate.peak_rss_bytes = aggregate.peak_rss_bytes.max(diagnostics.peak_rss_bytes);
        aggregate.orchestrator_peak_rss_bytes = aggregate
            .orchestrator_peak_rss_bytes
            .max(diagnostics.orchestrator_peak_rss_bytes);
        aggregate.worker_peak_rss_bytes = aggregate
            .worker_peak_rss_bytes
            .max(diagnostics.worker_peak_rss_bytes);
        aggregate.empty_binary_baseline_bytes =
            match (aggregate.empty_binary_baseline_bytes, diagnostics.empty_binary_baseline_bytes) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            };
        aggregate.rss_after_compile_bytes = max_optional_u64(
            aggregate.rss_after_compile_bytes,
            diagnostics.rss_after_compile_bytes,
        );
        aggregate.rss_after_artifacts_bytes = max_optional_u64(
            aggregate.rss_after_artifacts_bytes,
            diagnostics.rss_after_artifacts_bytes,
        );
        aggregate.rss_after_warmup_bytes = max_optional_u64(
            aggregate.rss_after_warmup_bytes,
            diagnostics.rss_after_warmup_bytes,
        );
        aggregate.graph_working_set_peak_bytes = max_optional_u64(
            aggregate.graph_working_set_peak_bytes,
            diagnostics.graph_working_set_peak_bytes,
        );
        aggregate.logical_disk_bytes = aggregate
            .logical_disk_bytes
            .max(diagnostics.logical_disk_bytes);
        aggregate.eval_artifacts_disk.total.files += diagnostics.eval_artifacts_disk.total.files;
        aggregate.eval_artifacts_disk.total.bytes += diagnostics.eval_artifacts_disk.total.bytes;
        aggregate.eval_artifacts_disk.metric_response.files +=
            diagnostics.eval_artifacts_disk.metric_response.files;
        aggregate.eval_artifacts_disk.metric_response.bytes +=
            diagnostics.eval_artifacts_disk.metric_response.bytes;
        aggregate.eval_artifacts_disk.metric_dataframe.files +=
            diagnostics.eval_artifacts_disk.metric_dataframe.files;
        aggregate.eval_artifacts_disk.metric_dataframe.bytes +=
            diagnostics.eval_artifacts_disk.metric_dataframe.bytes;
        aggregate.compile_index.preload_reuse_hits += diagnostics.compile_index.preload_reuse_hits;
        aggregate.compile_index.postload_identity_collapses +=
            diagnostics.compile_index.postload_identity_collapses;
        aggregate.compile_index.hits += diagnostics.compile_index.hits;
        aggregate.compile_index.misses += diagnostics.compile_index.misses;
        aggregate.compile_index.stale_entries += diagnostics.compile_index.stale_entries;
        aggregate.compile_index.fallback_loads += diagnostics.compile_index.fallback_loads;
        aggregate.compile_index.manifest_probes += diagnostics.compile_index.manifest_probes;
        aggregate.compile_index.manifest_stale_skips +=
            diagnostics.compile_index.manifest_stale_skips;
        aggregate.compile_index.artifact_loads_avoided +=
            diagnostics.compile_index.artifact_loads_avoided;
        aggregate.compile_index.mrg_eval_skips += diagnostics.compile_index.mrg_eval_skips;
        aggregate.compile_index.dataframe_eval_skips +=
            diagnostics.compile_index.dataframe_eval_skips;
        aggregate.compile_index.target_overlay_reuse_hits += diagnostics
            .compile_index
            .target_overlay_reuse_hits;
        aggregate.compile_index.mcg_assemble_only_count += diagnostics
            .compile_index
            .mcg_assemble_only_count;
        aggregate.assemble_only_count += diagnostics.assemble_only_count;
        aggregate.session_peak_identity_entries = aggregate
            .session_peak_identity_entries
            .max(diagnostics.session_peak_identity_entries);
        aggregate.hydrate_reuse_hits += diagnostics.hydrate_reuse_hits;
        aggregate.session_before_clear.scope_entries +=
            diagnostics.session_before_clear.scope_entries;
        aggregate.session_before_clear.cache_entries +=
            diagnostics.session_before_clear.cache_entries;
        aggregate.session_before_clear.identity_entries +=
            diagnostics.session_before_clear.identity_entries;
        aggregate.session_before_clear.target_entries +=
            diagnostics.session_before_clear.target_entries;
        aggregate.session_after_clear.scope_entries +=
            diagnostics.session_after_clear.scope_entries;
        aggregate.session_after_clear.cache_entries +=
            diagnostics.session_after_clear.cache_entries;
        aggregate.session_after_clear.identity_entries +=
            diagnostics.session_after_clear.identity_entries;
        aggregate.session_after_clear.target_entries +=
            diagnostics.session_after_clear.target_entries;
        aggregate.warmup_reuse_hits += diagnostics.warmup_reuse_hits;
        aggregate.plan_nodes.manifest_compile_scope_nodes +=
            diagnostics.plan_nodes.manifest_compile_scope_nodes;
        aggregate.plan_nodes.hot_compile_scope_nodes +=
            diagnostics.plan_nodes.hot_compile_scope_nodes;
        aggregate.plan_nodes.deferred_compile_scope_nodes +=
            diagnostics.plan_nodes.deferred_compile_scope_nodes;
        aggregate.plan_nodes.planned_warmup_request_nodes +=
            diagnostics.plan_nodes.planned_warmup_request_nodes;
        aggregate.plan_nodes.planned_warmup_scope_nodes +=
            diagnostics.plan_nodes.planned_warmup_scope_nodes;
        aggregate.plan_nodes.planned_metric_workset_nodes +=
            diagnostics.plan_nodes.planned_metric_workset_nodes;
        aggregate.plan_nodes.planned_response_artifact_nodes +=
            diagnostics.plan_nodes.planned_response_artifact_nodes;
        aggregate.plan_nodes.planned_dataframe_artifact_nodes +=
            diagnostics.plan_nodes.planned_dataframe_artifact_nodes;
        aggregate.plan_nodes.planned_total_nodes += diagnostics.plan_nodes.planned_total_nodes;
        aggregate.plan_nodes.canonical_prebuild_nodes +=
            diagnostics.plan_nodes.canonical_prebuild_nodes;
        aggregate.plan_nodes.budget.canonical_node_limit =
            diagnostics.plan_nodes.budget.canonical_node_limit;
        aggregate.plan_nodes.budget.startup_wall_ms_limit =
            diagnostics.plan_nodes.budget.startup_wall_ms_limit;
        aggregate.plan_nodes.budget.over_canonical_node_limit =
            aggregate.plan_nodes.budget.over_canonical_node_limit
                || diagnostics.plan_nodes.budget.over_canonical_node_limit;
        aggregate.critical_warmup.total_request_count +=
            diagnostics.critical_warmup.total_request_count;
        aggregate.critical_warmup.executed_request_count +=
            diagnostics.critical_warmup.executed_request_count;
        aggregate.critical_warmup.cache_hit_count += diagnostics.critical_warmup.cache_hit_count;
        aggregate.critical_warmup.total_ms += diagnostics.critical_warmup.total_ms;
        aggregate.critical_warmup.ok =
            aggregate.critical_warmup.ok || diagnostics.critical_warmup.ok;
        aggregate.deferred_warmup.total_request_count +=
            diagnostics.deferred_warmup.total_request_count;
        aggregate.deferred_warmup.executed_request_count +=
            diagnostics.deferred_warmup.executed_request_count;
        aggregate.deferred_warmup.cache_hit_count += diagnostics.deferred_warmup.cache_hit_count;
        aggregate.deferred_warmup.total_ms += diagnostics.deferred_warmup.total_ms;
        aggregate.deferred_warmup.ok =
            aggregate.deferred_warmup.ok || diagnostics.deferred_warmup.ok;
        slow_scopes.extend(diagnostics.slow_scopes.clone());
        slow_metrics.extend(diagnostics.slow_metrics.clone());
    }
    aggregate.expansion_ratio = if aggregate.unique_compile_result_count > 0 {
        aggregate.total_scope_checks as f64 / aggregate.unique_compile_result_count as f64
    } else {
        1.0
    };
    slow_scopes.sort_by_key(|entry| std::cmp::Reverse(entry.compile_ms));
    slow_scopes.truncate(8);
    slow_metrics.sort_by_key(|entry| std::cmp::Reverse(entry.ms));
    slow_metrics.truncate(8);
    aggregate.slow_scopes = slow_scopes;
    aggregate.slow_metrics = slow_metrics;
    aggregate
}

fn max_optional_u64(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

