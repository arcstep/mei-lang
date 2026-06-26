use super::prelude::*;
use super::*;

pub(crate) fn registry_snapshot_with_scope_gate(
    source_root: Option<&Path>,
) -> HostReadyResponse {
    let mut response = registry_snapshot();
    if let Some(root) = source_root {
        let reachability = crate::readiness::reachability::check_reachability(root, None);
        response.scope_gate = Some(reachability.scope_gate);
    }
    response
}

pub(crate) fn registry_snapshot() -> HostReadyResponse {
    let snapshot = host_readiness_registry()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let host_started_at_ms = host_started_at_ms_from_registry(&snapshot);
    let apps = snapshot
        .apps
        .into_iter()
        .map(|(app_id, state)| app_response(app_id, state))
        .collect::<Vec<_>>();
    let active_job_elapsed_ms = snapshot
        .active_job_started_at
        .map(|started| started.elapsed().as_millis() as u64);
    let ready_app_count = apps.iter().filter(|app| app.access_ready).count();
    let degraded_app_count = apps.iter().filter(|app| app.phase == "degraded").count();
    let failed_app_count = apps.iter().filter(|app| app.phase == "failed").count();
    HostReadyResponse {
        ready: snapshot.host_bound,
        run_id: snapshot.run_id.clone(),
        startup_policy: snapshot.startup_policy.clone(),
        build_descriptor: crate::build_info::descriptor(),
        startup_artifact_dir: snapshot.startup_artifact_dir.clone(),
        host_started_at_ms,
        host_ready: snapshot.host_bound,
        artifacts_ready: snapshot.artifacts_ready,
        scope_gate_ready: snapshot.scope_gate_ready,
        access_ready: snapshot.access_ready,
        default_app_id: snapshot.default_app_id.clone(),
        default_app_access_ready: snapshot.default_app_access_ready,
        any_app_access_ready: snapshot.any_app_access_ready,
        full_warmup_ready: snapshot.full_warmup_ready,
        deferred_warmup_pending: snapshot.deferred_warmup_pending,
        phase: if snapshot.phase.trim().is_empty() {
            "starting".to_string()
        } else {
            snapshot.phase
        },
        manifest_path: snapshot.manifest_path,
        manifest_source: snapshot.manifest_source,
        warmed_apps: snapshot.warmed_apps,
        failed_apps: snapshot.failed_apps,
        building_apps: snapshot.building_apps,
        active_job: snapshot.active_job,
        active_job_elapsed_ms,
        last_build_total_ms: snapshot.last_build_total_ms,
        last_build_compile_ms: snapshot.last_build_compile_ms,
        last_build_warmup_ms: snapshot.last_build_warmup_ms,
        last_critical_warmup_ms: snapshot.last_critical_warmup_ms,
        last_deferred_warmup_ms: snapshot.last_deferred_warmup_ms,
        last_critical_warmup_request_count: snapshot.last_critical_warmup_request_count,
        last_deferred_warmup_request_count: snapshot.last_deferred_warmup_request_count,
        last_warning_count: snapshot.last_warning_count,
        last_build_diagnostics: snapshot.last_build_diagnostics.clone(),
        correctness_failed: snapshot.correctness_failed,
        warning_categories: snapshot.warning_categories,
        warning_category_counts: snapshot.warning_category_counts,
        failing_datasets: snapshot.failing_datasets,
        ready_app_count,
        degraded_app_count,
        failed_app_count,
        error_summary: snapshot.error_summary,
        apps,
        scope_gate: None,
        gate_summary: snapshot.gate_summary,
    }
}

pub(crate) fn reset_registry_for_source_root(source_root: &Path) {
    let manifest_path = manifest_path_for(source_root);
    let manifest_source = manifest_source_label(source_root).to_string();
    let mut apps = BTreeMap::new();
    if let Ok(Some(manifest)) = mei_lang_kernel::resolve_runtime_warmup_manifest(source_root) {
        for app in manifest.apps {
            apps.insert(
                app.app_id,
                HostAppReadinessState {
                    phase: "pending".to_string(),
                    ..Default::default()
                },
            );
        }
    }
    let _ = with_registry(|registry| {
        *registry = HostReadinessRegistry {
            host_bound: false,
            host_started_at_ms: startup_run::current_started_at_ms(),
            artifacts_ready: false,
            scope_gate_ready: false,
            gate_summary: None,
            access_ready: false,
            default_app_id: None,
            default_app_access_ready: false,
            any_app_access_ready: false,
            full_warmup_ready: false,
            deferred_warmup_pending: false,
            run_id: startup_run::current_run_id(),
            startup_policy: startup_run::current_startup_policy(),
            startup_artifact_dir: startup_run::current_artifact_dir(),
            phase: "starting".to_string(),
            manifest_path: manifest_path.display().to_string(),
            manifest_source,
            warmed_apps: Vec::new(),
            failed_apps: Vec::new(),
            building_apps: Vec::new(),
            error_summary: Vec::new(),
            active_job: None,
            active_job_started_at: None,
            last_build_total_ms: None,
            last_build_compile_ms: None,
            last_build_warmup_ms: None,
            last_critical_warmup_ms: None,
            last_deferred_warmup_ms: None,
            last_critical_warmup_request_count: 0,
            last_deferred_warmup_request_count: 0,
            last_warning_count: 0,
            last_build_diagnostics: None,
            correctness_failed: false,
            warning_categories: Vec::new(),
            warning_category_counts: BTreeMap::new(),
            failing_datasets: Vec::new(),
            apps,
        };
    });
}

pub(crate) fn set_selected_apps_phase(
    registry: &mut HostReadinessRegistry,
    app_filter: Option<&str>,
    phase: &str,
) -> Vec<String> {
    let mut selected = Vec::new();
    if let Some(app_id) = app_filter.map(str::trim).filter(|value| !value.is_empty()) {
        let entry = registry.apps.entry(app_id.to_string()).or_default();
        entry.phase = phase.to_string();
        entry.last_error = None;
        selected.push(app_id.to_string());
    } else {
        if registry.apps.is_empty() {
            return selected;
        }
        for (app_id, app) in &mut registry.apps {
            app.phase = phase.to_string();
            app.last_error = None;
            selected.push(app_id.clone());
        }
    }
    selected
}

pub(crate) fn sync_registry_phase(registry: &mut HostReadinessRegistry) {
    if registry.active_job.is_some() {
        registry.phase = "building".to_string();
        return;
    }
    if registry.apps.is_empty() {
        registry.phase = if registry.host_bound {
            "skipped".to_string()
        } else {
            "starting".to_string()
        };
        return;
    }
    let ready_count = registry
        .apps
        .values()
        .filter(|app| app.access_ready)
        .count();
    let degraded_count = registry
        .apps
        .values()
        .filter(|app| app.phase == "degraded")
        .count();
    let failed_count = registry
        .apps
        .values()
        .filter(|app| app.phase == "failed")
        .count();
    let building_count = registry
        .apps
        .values()
        .filter(|app| matches!(app.phase.as_str(), "pending" | "building"))
        .count();
    registry.warmed_apps = registry
        .apps
        .iter()
        .filter_map(|(app_id, app)| app.access_ready.then_some(app_id.clone()))
        .collect();
    registry.failed_apps = registry
        .apps
        .iter()
        .filter_map(|(app_id, app)| (app.phase == "failed").then_some(app_id.clone()))
        .collect();
    registry.building_apps = registry
        .apps
        .iter()
        .filter_map(|(app_id, app)| {
            matches!(app.phase.as_str(), "pending" | "building").then_some(app_id.clone())
        })
        .collect();
    registry.phase = if failed_count > 0 && (ready_count > 0 || degraded_count > 0) {
        "degraded".to_string()
    } else if failed_count > 0 && building_count == 0 {
        "failed".to_string()
    } else if building_count > 0 {
        if registry.phase == "verifying" {
            "verifying".to_string()
        } else {
            "building".to_string()
        }
    } else if degraded_count > 0 {
        "degraded".to_string()
    } else if ready_count == registry.apps.len() {
        "ready".to_string()
    } else if registry.host_bound {
        "bound".to_string()
    } else {
        "starting".to_string()
    };
}

