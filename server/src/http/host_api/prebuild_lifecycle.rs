use super::prelude::*;
use super::*;

pub(crate) fn mark_job_failed(
    app_filter: Option<&str>,
    mode: PrebuildMode,
    error: &str,
    preserve_access_ready: bool,
) {
    let active_job = with_registry(|registry| {
        let active_job = registry.active_job.clone();
        registry.error_summary = vec![error.to_string()];
        registry.active_job_started_at = None;
        if !preserve_access_ready {
            registry.access_ready = false;
            registry.default_app_access_ready = false;
            registry.any_app_access_ready = false;
            registry.artifacts_ready = false;
            registry.scope_gate_ready = false;
        }
        registry.full_warmup_ready = false;
        registry.deferred_warmup_pending = false;
        registry.last_critical_warmup_ms = None;
        registry.last_deferred_warmup_ms = None;
        registry.last_critical_warmup_request_count = 0;
        registry.last_deferred_warmup_request_count = 0;
        registry.last_build_diagnostics = None;
        registry.correctness_failed = true;
        registry.warning_categories.clear();
        registry.warning_category_counts.clear();
        registry.failing_datasets.clear();
        if let Some(app_id) = app_filter.map(str::trim).filter(|value| !value.is_empty()) {
            let app_state = registry.apps.entry(app_id.to_string()).or_default();
            app_state.phase = "failed".to_string();
            app_state.last_error = Some(error.to_string());
        } else {
            for app_state in registry.apps.values_mut() {
                app_state.phase = "failed".to_string();
                app_state.last_error = Some(error.to_string());
            }
        }
        registry.active_job = None;
        registry.phase = match mode {
            PrebuildMode::Build => "failed".to_string(),
            PrebuildMode::Verify => "failed".to_string(),
        };
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
    if let Some(job) = active_job.flatten() {
        if job.starts_with("startup_deferred:") {
            startup_run::write_prebuild_error(
                "full",
                error,
                Some(serde_json::json!({ "job": job, "mode": format!("{mode:?}") })),
            );
        } else if job.starts_with("startup:") {
            let slot = if preserve_access_ready || mode == PrebuildMode::Verify {
                "full"
            } else {
                "hot"
            };
            startup_run::write_prebuild_error(
                slot,
                error,
                Some(serde_json::json!({ "job": job, "mode": format!("{mode:?}") })),
            );
        }
        if job.starts_with("startup:") || job.starts_with("startup_deferred:") {
            startup_run::record_phase(
                "access_not_ready",
                Some(serde_json::json!({
                    "error": error,
                    "job": job,
                })),
            );
            startup_run::record_phase(
                "startup_finished",
                Some(serde_json::json!({
                    "phase": snapshot.phase,
                    "ok": false,
                    "startupOutcome": "failed",
                    "error": error,
                })),
            );
        }
    }
    tracing::warn!(mode = ?mode, %error, "host build job failed");
}

pub(crate) fn begin_job(
    mode: PrebuildMode,
    app_filter: Option<&str>,
    origin: &str,
) -> Result<String> {
    with_registry(|registry| {
        if registry.active_job.is_some() {
            return Err(anyhow!("host build job is already running"));
        }
        let mode_label = match mode {
            PrebuildMode::Build => "build",
            PrebuildMode::Verify => "verify",
        };
        let job = if let Some(app_id) = app_filter.map(str::trim).filter(|value| !value.is_empty())
        {
            format!("{origin}:{mode_label}:{app_id}")
        } else {
            format!("{origin}:{mode_label}:workspace")
        };
        registry.active_job = Some(job.clone());
        registry.active_job_started_at = Some(Instant::now());
        let selected = set_selected_apps_phase(registry, app_filter, "building");
        if selected.is_empty() && app_filter.is_none() {
            registry.phase = "skipped".to_string();
        } else {
            registry.building_apps = selected;
            registry.phase = match mode {
                PrebuildMode::Build => "building".to_string(),
                PrebuildMode::Verify => "verifying".to_string(),
            };
        }
        Ok(job)
    })
    .unwrap_or_else(|| Err(anyhow!("host readiness registry is unavailable")))
}

pub(crate) fn run_prebuild_job_sync_inner(
    source_root: &Path,
    mode: PrebuildMode,
    app_filter: Option<&str>,
    scope_profile: PrebuildScopeProfile,
) -> Result<PrebuildReport> {
    if let Ok(package_root) = crate::cli::util::resolve_package_root() {
        let _ = mei_lang_toolchain::ensure_workspace_author_skill_package(
            source_root,
            package_root.as_path(),
        );
    }
    run_prebuild(
        source_root,
        &PrebuildOptions {
            app_filter: app_filter
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            mode,
            clean: false,
            force_rebuild: false,
            scope_profile,
            dirty_only: false,
            block_node: None,
            diagnose_on_fail: true,
            continue_from: None,
        },
    )
}
