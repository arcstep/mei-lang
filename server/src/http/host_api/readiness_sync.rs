use super::prelude::*;
use super::*;

pub(crate) fn apply_success_app_report(app_report: &PrebuildAppReport, app_state: &mut HostAppReadinessState) {
    app_state.phase = if app_report.warnings.is_empty() {
        "ready".to_string()
    } else {
        "degraded".to_string()
    };
    app_state.last_error = None;
    app_state.warnings = app_report
        .warnings
        .iter()
        .map(|warning| warning.display_message().to_string())
        .collect();
    app_state.warning_details = app_report.warnings.clone();
    app_state.warning_categories = app_report
        .warnings
        .iter()
        .map(|warning| warning.category.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut seen = BTreeSet::new();
    for scope in &app_report.compile_scopes {
        let key = normalize_scope_key(
            scope
                .requested_scene_id
                .as_deref()
                .or(scope.active_scene_id.as_deref()),
            Some(scope.active_target_file.as_str()).or(scope.requested_target_file.as_deref()),
        );
        if !seen.insert(key.clone()) {
            continue;
        }
        app_state.scopes.insert(
            key,
            HostScopeReadinessState {
                scene_id: scope
                    .requested_scene_id
                    .clone()
                    .or(scope.active_scene_id.clone()),
                target_file: scope
                    .requested_target_file
                    .clone()
                    .or_else(|| Some(scope.active_target_file.clone())),
                phase: "ready".to_string(),
                compile_revision: Some(scope.compile_revision.clone()),
                last_error: None,
            },
        );
    }
}

pub(crate) fn status_from_report(
    report: &PrebuildReport,
    app_filter: Option<&str>,
    deferred_warmup_pending: bool,
) {
    let warning_count = report
        .apps
        .iter()
        .map(|app| app.warnings.len())
        .sum::<usize>();
    let failed_app_count = report.failed_apps.len();
    let shell_ready = crate::readiness::reachability::shell_ready_for_access_entry(
        Path::new(&report.source_root),
    );
    let access_artifacts_ready = failed_app_count == 0 && shell_ready;
    let compile_ms: u64 = report
        .apps
        .iter()
        .map(|app| app.timings.compile_scopes_ms)
        .sum();
    let warmup_ms: u64 = report
        .apps
        .iter()
        .map(|app| app.timings.warmup_requests_ms)
        .sum();
    let critical_warmup_ms: u64 = report
        .apps
        .iter()
        .map(|app| app.timings.critical_warmup_requests_ms)
        .sum();
    let deferred_warmup_ms: u64 = report
        .apps
        .iter()
        .map(|app| app.timings.deferred_warmup_requests_ms)
        .sum();
    let critical_warmup_request_count: usize = report
        .apps
        .iter()
        .map(|app| app.timings.critical_warmup_request_count)
        .sum();
    let deferred_warmup_request_count: usize = report
        .apps
        .iter()
        .map(|app| app.timings.deferred_warmup_request_count)
        .sum();
    let warning_categories = report.warning_categories();
    let warning_category_counts = report.warning_category_counts();
    let failing_datasets = report.failing_datasets();
    let correctness_failed = report.correctness_failed();
    let registry_update = with_registry(|registry| {
        let active_job = registry.active_job.clone();
        registry.manifest_path = report.manifest_path.clone();
        registry.manifest_source = report.manifest_source.clone();
        registry.error_summary = report.error_summary.clone();
        registry.active_job_started_at = None;
        registry.last_build_total_ms = Some(report.total_wall_ms);
        registry.last_build_compile_ms = Some(compile_ms);
        registry.last_build_warmup_ms = Some(warmup_ms);
        registry.last_critical_warmup_ms = Some(critical_warmup_ms);
        registry.last_deferred_warmup_ms = Some(deferred_warmup_ms);
        registry.last_critical_warmup_request_count = critical_warmup_request_count;
        registry.last_deferred_warmup_request_count = deferred_warmup_request_count;
        registry.last_warning_count = warning_count;
        registry.last_build_diagnostics = Some(report.diagnostics.clone());
        registry.correctness_failed = correctness_failed;
        registry.warning_categories = warning_categories.clone();
        registry.warning_category_counts = warning_category_counts.clone();
        registry.failing_datasets = failing_datasets.clone();
        registry.access_ready = report.ok && shell_ready && !correctness_failed;
        registry.full_warmup_ready =
            report.ok && shell_ready && !correctness_failed && !deferred_warmup_pending;
        registry.deferred_warmup_pending =
            report.ok && shell_ready && !correctness_failed && deferred_warmup_pending;
        for app_report in &report.apps {
            let app_state = registry.apps.entry(app_report.app_id.clone()).or_default();
            apply_success_app_report(app_report, app_state);
        }
        if report.diagnostics.fingerprint_skip {
            for app_id in &report.succeeded_apps {
                let gate = crate::readiness::scope_gate::check_scope_gate(
                    Path::new(&report.source_root),
                    app_id,
                    None,
                    None,
                );
                let app_state = registry.apps.entry(app_id.clone()).or_default();
                if gate.access_ready {
                    app_state.phase = "ready".to_string();
                    app_state.last_error = None;
                } else if app_state.phase == "building" || app_state.phase == "pending" {
                    app_state.phase = "degraded".to_string();
                    app_state.last_error = gate.blockers.first().cloned();
                }
            }
        }
        for app_id in &report.failed_apps {
            let app_state = registry.apps.entry(app_id.clone()).or_default();
            app_state.phase = "failed".to_string();
            app_state.last_error = report
                .error_summary
                .iter()
                .find_map(|line| {
                    line.strip_prefix(&format!("{app_id}: "))
                        .map(str::to_string)
                })
                .or_else(|| Some("prebuild failed".to_string()));
        }
        if let Some(app_id) = app_filter.map(str::trim).filter(|value| !value.is_empty()) {
            if !report.succeeded_apps.iter().any(|value| value == app_id)
                && !report.failed_apps.iter().any(|value| value == app_id)
            {
                let app_state = registry.apps.entry(app_id.to_string()).or_default();
                app_state.phase = "failed".to_string();
                app_state.last_error =
                    Some("requested app missing from prebuild report".to_string());
            }
        }
        registry.active_job = None;
        sync_registry_phase(registry);
        active_job
    });
    let snapshot = registry_snapshot();
    startup_run::update_readiness_snapshot(
        snapshot.phase.as_str(),
        snapshot.access_ready,
        snapshot.full_warmup_ready,
        snapshot.deferred_warmup_pending,
        &snapshot,
    );
    if let Some(active_job) = registry_update {
        if active_job
            .as_deref()
            .map(|job| job.starts_with("startup:") || job.starts_with("startup_deferred:"))
            .unwrap_or(false)
        {
            let slot = if active_job
                .as_deref()
                .map(|job| job.starts_with("startup_deferred:"))
                .unwrap_or(false)
                || report.scope_profile == PrebuildScopeProfile::Full
            {
                "full"
            } else {
                "hot"
            };
            startup_run::write_prebuild_report(slot, report);
            startup_run::record_startup_prebuild_outcome(
                slot,
                report,
                access_artifacts_ready,
                warning_count,
                failed_app_count,
                compile_ms,
                warmup_ms,
                !deferred_warmup_pending,
            );
        }
    }
    tracing::info!(
        mode = ?report.mode,
        total_wall_ms = report.total_wall_ms,
        succeeded_app_count = report.succeeded_apps.len(),
        failed_app_count,
        warning_count,
        "startup prebuild report applied"
    );
    if failed_app_count == 0 && warning_count == 0 {
        let ready_title = if deferred_warmup_pending {
            "ACCESS READY!"
        } else {
            "FULL READY!"
        };
        let ready_detail = if deferred_warmup_pending {
            "access artifacts ready; deferred warmup still running"
        } else {
            "full warmup artifacts ready"
        };
        emit_prebuild_status_line(
            ready_title,
            "1;32",
            &format!(
                "[PREBUILD +{:.1}s] {ready_detail} | apps={} | compile={}ms | warmup={}ms",
                report.total_wall_ms as f64 / 1000.0,
                report.succeeded_apps.len(),
                compile_ms,
                warmup_ms
            ),
        );
        tracing::info!(
            total_wall_ms = report.total_wall_ms,
            compile_ms,
            warmup_ms,
            app_count = report.succeeded_apps.len(),
            deferred_warmup_pending,
            "{ready_title} {ready_detail}"
        );
    } else {
        emit_prebuild_status_line(
            "NOT READY!",
            "1;31",
            &format!(
                "[PREBUILD +{:.1}s] access artifacts incomplete | apps={} | failed_apps={} | warnings={} | compile={}ms | warmup={}ms",
                report.total_wall_ms as f64 / 1000.0,
                report.apps.len(),
                failed_app_count,
                warning_count,
                compile_ms,
                warmup_ms
            ),
        );
        tracing::warn!(
            total_wall_ms = report.total_wall_ms,
            compile_ms,
            warmup_ms,
            app_count = report.apps.len(),
            failed_app_count,
            warning_count,
            "NOT READY! access artifacts incomplete"
        );
    }
    refresh_metric_response_indices_after_prebuild(report, app_filter);
}

pub(crate) fn refresh_metric_response_indices_after_prebuild(
    report: &PrebuildReport,
    app_filter: Option<&str>,
) {
    let source_root = Path::new(report.source_root.as_str());
    let app_ids: Vec<String> =
        if let Some(app_id) = app_filter.map(str::trim).filter(|value| !value.is_empty()) {
            vec![app_id.to_string()]
        } else {
            report.succeeded_apps.clone()
        };
    for app_id in app_ids {
        let app_root = resolve_app_root(source_root, app_id.as_str());
        match preload_prebuild_metric_response_index(app_root.as_path()) {
            Ok(stats) => {
                let mrg_slots = crate::graph::mrg::slots::mrg_slot_count(source_root, app_id.as_str());
                tracing::info!(
                    app_id = %app_id,
                    index_load_ms = stats.load_ms,
                    entry_count = stats.entry_count,
                    mrg_slot_count = mrg_slots,
                    rebuilt = stats.rebuilt,
                    "ensured metric response artifact index after prebuild"
                );
            }
            Err(error) => tracing::warn!(
                app_id = %app_id,
                %error,
                "failed to ensure metric response index after prebuild"
            ),
        }
    }
}

pub(crate) fn preload_metric_response_indices_for_workspace(source_root: &Path) {
    let Ok(Some(manifest)) = mei_lang_kernel::resolve_runtime_warmup_manifest(source_root) else {
        return;
    };
    for app in manifest.apps {
        let app_root = resolve_app_root(source_root, app.app_id.as_str());
        match preload_prebuild_metric_response_index(app_root.as_path()) {
            Ok(stats) => tracing::info!(
                app_id = %app.app_id,
                index_load_ms = stats.load_ms,
                entry_count = stats.entry_count,
                rebuilt = stats.rebuilt,
                "preloaded metric response artifact index"
            ),
            Err(error) => tracing::warn!(
                app_id = %app.app_id,
                %error,
                "metric response index preload failed"
            ),
        }
    }
}

