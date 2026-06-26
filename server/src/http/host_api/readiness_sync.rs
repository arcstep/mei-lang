use super::prelude::*;
use super::*;

fn compile_scope_keys_from_report(report: &PrebuildReport, app_id: &str) -> BTreeSet<String> {
    report
        .apps
        .iter()
        .find(|app| app.app_id == app_id)
        .map(|app| {
            app.compile_scopes
                .iter()
                .map(|scope| {
                    normalize_scope_key(
                        scope
                            .requested_scene_id
                            .as_deref()
                            .or(scope.active_scene_id.as_deref()),
                        Some(scope.active_target_file.as_str())
                            .or(scope.requested_target_file.as_deref()),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn prune_stale_scopes(registry: &mut HostReadinessRegistry, report: &PrebuildReport) {
    for app_id in &report.succeeded_apps {
        let keep = compile_scope_keys_from_report(report, app_id.as_str());
        if let Some(app_state) = registry.apps.get_mut(app_id.as_str()) {
            app_state.scopes.retain(|key, _| keep.contains(key));
        }
    }
}

#[cfg(test)]
pub(crate) fn scope_registry_has_degraded_errors(registry: &HostReadinessRegistry) -> bool {
    registry.apps.values().any(|app| {
        app.scopes.values().any(|scope| {
            scope.phase == "degraded"
                || scope
                    .last_error
                    .as_deref()
                    .is_some_and(|err| !err.trim().is_empty())
        })
    })
}

fn app_default_scope_access_ready(source_root: &Path, app_id: &str) -> bool {
    use crate::graph::mrg::navigation::resolve_default_scope;
    use crate::readiness::types::UiMode;

    let nav = resolve_default_scope(source_root, app_id, UiMode::App);
    crate::readiness::scope_gate::check_scope_gate_silent(
        source_root,
        app_id,
        Some(nav.scope.scene_id.as_str()),
        Some(nav.scope.target_file.as_str()),
        true,
    )
    .access_ready
}

fn apply_global_access_flags(registry: &mut HostReadinessRegistry, source_root: &Path) {
    use mei_lang_kernel::{load_workspace_config, resolve_app_id};

    let workspace = load_workspace_config(source_root);
    let configured_default = workspace
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    registry.default_app_id = configured_default.as_ref().map(|preferred| {
        let canonical = resolve_app_id(source_root, preferred.as_str());
        if registry.apps.contains_key(canonical.as_str()) {
            canonical
        } else {
            preferred.clone()
        }
    });
    registry.any_app_access_ready = registry.apps.values().any(|app| app.access_ready);
    registry.default_app_access_ready = registry
        .default_app_id
        .as_ref()
        .and_then(|app_id| registry.apps.get(app_id.as_str()))
        .is_some_and(|app| app.access_ready);
    registry.access_ready = if registry.default_app_id.is_some() {
        registry.default_app_access_ready
    } else {
        registry.any_app_access_ready
    };
}

fn refresh_registry_scope_gates(
    source_root: &Path,
    registry: &mut HostReadinessRegistry,
    app_ids: &[String],
) -> ScopeGateSweepSummary {
    let mut summary = ScopeGateSweepSummary::default();
    for app_id in app_ids {
        let scope_keys = registry
            .apps
            .get(app_id.as_str())
            .map(|app| app.scopes.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut app_summary = ScopeGateSweepSummary::default();
        for key in scope_keys {
            let Some(scope) = registry
                .apps
                .get(app_id.as_str())
                .and_then(|app| app.scopes.get(&key))
                .cloned()
            else {
                continue;
            };
            let gate = crate::readiness::scope_gate::check_scope_gate_silent(
                source_root,
                app_id.as_str(),
                scope.scene_id.as_deref(),
                scope.target_file.as_deref(),
                true,
            );
            if !gate.navigation_ready {
                summary.l2_miss += 1;
                app_summary.l2_miss += 1;
            }
            if !gate.assembly_ready {
                summary.l3_fail += 1;
                app_summary.l3_fail += 1;
            }
            if !gate.data_ready {
                summary.l4_stale += 1;
                app_summary.l4_stale += 1;
            }
            let Some(app_state) = registry.apps.get_mut(app_id.as_str()) else {
                continue;
            };
            let entry = app_state.scopes.entry(key.clone()).or_default();
            if gate.access_ready {
                entry.phase = "ready".to_string();
                entry.last_error = None;
            } else {
                entry.phase = "degraded".to_string();
                entry.last_error = gate.blockers.first().cloned();
                summary.degraded_scopes.push(key.clone());
                app_summary.degraded_scopes.push(key);
            }
        }
        if let Some(app_state) = registry.apps.get_mut(app_id.as_str()) {
            app_state.gate_summary = Some(app_summary);
            app_state.access_ready = app_default_scope_access_ready(source_root, app_id.as_str());
            app_state.phase = if app_state.access_ready {
                "ready".to_string()
            } else if app_state
                .gate_summary
                .as_ref()
                .is_some_and(|gate| gate.l2_miss > 0 || gate.l3_fail > 0 || gate.l4_stale > 0)
            {
                "degraded".to_string()
            } else {
                "building".to_string()
            };
        }
    }
    apply_global_access_flags(registry, source_root);
    summary
}

pub(crate) fn apply_success_app_report(app_report: &PrebuildAppReport, app_state: &mut HostAppReadinessState) {
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
    let artifacts_ready = report.ok && access_artifacts_ready;
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
    let mut scope_gate_ready = true;
    let mut gate_summary = ScopeGateSweepSummary::default();
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
        registry.artifacts_ready = artifacts_ready;
        for app_report in &report.apps {
            let app_state = registry.apps.entry(app_report.app_id.clone()).or_default();
            apply_success_app_report(app_report, app_state);
        }
        prune_stale_scopes(registry, report);
        gate_summary = refresh_registry_scope_gates(
            Path::new(&report.source_root),
            registry,
            &report.succeeded_apps,
        );
        scope_gate_ready = gate_summary.l2_miss == 0
            && gate_summary.l3_fail == 0
            && gate_summary.l4_stale == 0;
        registry.scope_gate_ready = scope_gate_ready;
        registry.gate_summary = Some(gate_summary.clone());
        registry.full_warmup_ready =
            artifacts_ready && scope_gate_ready && !deferred_warmup_pending;
        registry.deferred_warmup_pending =
            artifacts_ready && scope_gate_ready && deferred_warmup_pending;
        if gate_summary.l2_miss > 0 || gate_summary.l3_fail > 0 || gate_summary.l4_stale > 0 {
            tracing::info!(
                l2_miss = gate_summary.l2_miss,
                l3_fail = gate_summary.l3_fail,
                l4_stale = gate_summary.l4_stale,
                access_ready = registry.access_ready,
                default_app_access_ready = registry.default_app_access_ready,
                any_app_access_ready = registry.any_app_access_ready,
                "scope gate sweep summary"
            );
        }
        emit_prebuild_status_line(
            "gate sweep",
            "1;36",
            &format!(
                "L2={} L3={} L4={} | defaultAppAccessReady={} | anyAppAccessReady={}",
                gate_summary.l2_miss,
                gate_summary.l3_fail,
                gate_summary.l4_stale,
                registry.default_app_access_ready,
                registry.any_app_access_ready
            ),
        );
        registry.active_job = None;
        sync_registry_phase(registry);
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
    if failed_app_count == 0 && artifacts_ready && scope_gate_ready {
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
        let snapshot = registry_snapshot();
        let app_lines = snapshot
            .apps
            .iter()
            .map(|app| {
                let gate = app
                    .gate_summary
                    .as_ref()
                    .map(|summary| {
                        format!(
                            "L2={} L3={} L4={}",
                            summary.l2_miss, summary.l3_fail, summary.l4_stale
                        )
                    })
                    .unwrap_or_else(|| "L2=? L3=? L4=?".to_string());
                format!(
                    "  {}: {} | accessReady={} | {} | warnings={}",
                    app.app_id,
                    app.phase,
                    app.access_ready,
                    gate,
                    app.warnings.len()
                )
            })
            .collect::<Vec<_>>();
        let gate_detail = if !scope_gate_ready {
            format!(
                "scope_gate L2={} L3={} L4={}",
                gate_summary.l2_miss, gate_summary.l3_fail, gate_summary.l4_stale
            )
        } else if !artifacts_ready {
            "artifacts incomplete".to_string()
        } else {
            String::new()
        };
        emit_prebuild_status_line(
            "WORKSPACE ACCESS INCOMPLETE",
            "1;33",
            &format!(
                "[PREBUILD +{:.1}s] host shell unaffected | failed_apps={} | warnings={} | {gate_detail} | compile={}ms | warmup={}ms\n{}",
                report.total_wall_ms as f64 / 1000.0,
                failed_app_count,
                warning_count,
                compile_ms,
                warmup_ms,
                app_lines.join("\n")
            ),
        );
        tracing::warn!(
            total_wall_ms = report.total_wall_ms,
            compile_ms,
            warmup_ms,
            app_count = report.apps.len(),
            failed_app_count,
            warning_count,
            scope_gate_ready,
            artifacts_ready,
            "workspace access incomplete (host shell remains available)"
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
        let mrg_slots = crate::graph::mrg::slots::mrg_slot_count(source_root, app_id.as_str());
        tracing::info!(
            app_id = %app_id,
            mrg_slot_count = mrg_slots,
            "verified MRG slot registry after prebuild"
        );
    }
}

pub(crate) fn preload_metric_response_indices_for_workspace(source_root: &Path) {
    let Ok(Some(manifest)) = mei_lang_kernel::resolve_runtime_warmup_manifest(source_root) else {
        return;
    };
    for app in manifest.apps {
        let mrg_slots =
            crate::graph::mrg::slots::mrg_slot_count(source_root, app.app_id.as_str());
        tracing::info!(
            app_id = %app.app_id,
            mrg_slot_count = mrg_slots,
            "preloaded MRG slot registry"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_registry_has_degraded_errors_detects_scope_last_error() {
        let mut registry = HostReadinessRegistry::default();
        registry.apps.insert(
            "zhifa".to_string(),
            HostAppReadinessState {
                phase: "ready".to_string(),
                scopes: BTreeMap::from([(
                    "home/scenes/home.mei".to_string(),
                    HostScopeReadinessState {
                        scene_id: Some("home".to_string()),
                        target_file: Some("scenes/home.mei".to_string()),
                        phase: "degraded".to_string(),
                        last_error: Some("invalid_resource_ref".to_string()),
                        ..Default::default()
                    },
                )]),
                ..Default::default()
            },
        );
        assert!(scope_registry_has_degraded_errors(&registry));
    }

    #[test]
    fn scope_registry_has_degraded_errors_allows_ready_scope() {
        let mut registry = HostReadinessRegistry::default();
        registry.apps.insert(
            "zhifa".to_string(),
            HostAppReadinessState {
                phase: "ready".to_string(),
                scopes: BTreeMap::from([(
                    "home/scenes/home.mei".to_string(),
                    HostScopeReadinessState {
                        phase: "ready".to_string(),
                        ..Default::default()
                    },
                )]),
                ..Default::default()
            },
        );
        assert!(!scope_registry_has_degraded_errors(&registry));
    }

    #[test]
    fn apply_global_access_flags_prefers_default_app_when_configured() {
        let root = std::env::temp_dir().join(format!(
            "mei-access-flags-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        std::fs::write(
            root.join(".mei-workspace.json"),
            r#"{"workspace":{"defaultApp":"zhifa"}}"#,
        )
        .expect("write workspace config");
        std::fs::create_dir_all(root.join("zhifa")).expect("create app dir");
        std::fs::create_dir_all(root.join("qunfu")).expect("create app dir");

        let mut registry = HostReadinessRegistry::default();
        registry.apps.insert(
            "zhifa".to_string(),
            HostAppReadinessState {
                phase: "degraded".to_string(),
                access_ready: false,
                ..Default::default()
            },
        );
        registry.apps.insert(
            "qunfu".to_string(),
            HostAppReadinessState {
                phase: "ready".to_string(),
                access_ready: true,
                ..Default::default()
            },
        );
        apply_global_access_flags(&mut registry, root.as_path());
        assert_eq!(registry.default_app_id.as_deref(), Some("zhifa"));
        assert!(!registry.default_app_access_ready);
        assert!(registry.any_app_access_ready);
        assert!(!registry.access_ready);

        let _ = std::fs::remove_dir_all(root);
    }
}

