use super::prelude::*;
use super::*;

pub(crate) fn initialize_startup_readiness(source_root: &Path, startup_policy: &str) {
    startup_run::initialize(source_root, startup_policy);
    reset_registry_for_source_root(source_root);
}

pub(crate) fn mark_host_bound() {
    let started_at_ms = startup_run::current_started_at_ms().or_else(|| Some(startup_run::now_ms_for_host_message()));
    let _ = with_registry(|registry| {
        registry.host_bound = true;
        if registry.host_started_at_ms.is_none() {
            registry.host_started_at_ms = started_at_ms;
        }
        sync_registry_phase(registry);
    });
    startup_run::record_phase(
        "host_bound",
        Some(serde_json::json!({
            "phase": registry_snapshot().phase,
        })),
    );
}

fn default_landing_ready(source_root: &Path) -> bool {
    let workspace = mei_lang_kernel::load_workspace_config(source_root);
    let default_app = workspace
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|app_id| mei_lang_kernel::resolve_app_id(source_root, app_id));
    default_app
        .as_deref()
        .map(|app_id| crate::readiness::scope_gate::default_app_access_ready(source_root, app_id))
        .unwrap_or(false)
}

fn startup_prebuild_skip_reason(source_root: &Path) -> Option<String> {
    if std::env::var("MEI_STARTUP_FORCE_PREBUILD")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
    {
        return None;
    }
    if let Ok(Some((report, age))) = crate::prebuild::recent_ok_prebuild_report(source_root) {
        let profile = match report.scope_profile {
            PrebuildScopeProfile::Full => "full",
            PrebuildScopeProfile::HotOnly => "hot_only",
            PrebuildScopeProfile::BlockScoped => "block_scoped",
        };
        if age.as_secs() <= crate::prebuild::RECENT_PREBUILD_TRUST_LANDING_SECS {
            return Some(format!(
                "recent prebuild ok ({profile}, {:.0}s ago, {:.0}s wall) — reuse CLI prebuild-last.json"
            ,
                age.as_secs_f64(),
                report.total_wall_ms as f64 / 1000.0
            ));
        }
        if age.as_secs() <= crate::prebuild::RECENT_PREBUILD_SKIP_MAX_AGE_SECS
            && default_landing_ready(source_root)
        {
            return Some(format!(
                "recent prebuild ok ({profile}, {:.0}min ago) + default landing ready",
                age.as_secs_f64() / 60.0
            ));
        }
    }
    if !default_landing_ready(source_root) {
        return None;
    }
    let workspace = mei_lang_kernel::load_workspace_config(source_root);
    let default_app = workspace
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let app_root = mei_lang_kernel::resolve_app_root(source_root, default_app);
    let current = mei_lang_kernel::resolve_app_build_generation_from_current(app_root.as_path())
        .ok()
        .filter(|value| !value.trim().is_empty())?;
    let matched = crate::prebuild_fingerprint::try_match_prebuild_fingerprint(source_root).ok()??;
    if matched.stored.artifact_coverage_summary.total_missing_artifacts > 0 {
        return None;
    }
    Some(format!(
        "fingerprint match + env/current={current} + default landing ready"
    ))
}

pub(crate) fn verify_startup_artifacts(source_root: &Path) -> Result<PrebuildReport> {
    if cfg!(test) {
        let report = PrebuildReport {
            schema_version: "mei-prebuild-report-v1".to_string(),
            mode: PrebuildMode::Verify,
            scope_profile: PrebuildScopeProfile::Full,
            clean: false,
            clean_wall_ms: 0,
            total_wall_ms: 0,
            source_root: source_root.display().to_string(),
            manifest_path: manifest_path_for(source_root).display().to_string(),
            manifest_source: "test_skip".to_string(),
            ok: true,
            succeeded_apps: Vec::new(),
            failed_apps: Vec::new(),
            error_summary: Vec::new(),
            diagnostics: PrebuildDiagnosticsReport::default(),
            apps: Vec::new(),
        };
        reset_registry_for_source_root(source_root);
        let _ = with_registry(|registry| {
            registry.phase = "skipped".to_string();
            registry.manifest_source = "test_skip".to_string();
        });
        return Ok(report);
    }
    begin_job(PrebuildMode::Verify, None, "startup")?;
    startup_run::record_phase(
        "startup_prebuild_started",
        Some(serde_json::json!({
            "job": "startup:verify:workspace",
            "scopeProfile": "full",
            "mode": "verify",
        })),
    );
    match run_prebuild_job_sync_inner(
        source_root,
        PrebuildMode::Verify,
        None,
        PrebuildScopeProfile::Full,
    ) {
        Ok(report) => {
            status_from_report(&report, None, false, ScopeGateRefreshMode::Full);
            Ok(report)
        }
        Err(error) => {
            let error_text = error.to_string();
            mark_job_failed(None, PrebuildMode::Verify, &error_text, false);
            Err(error)
        }
    }
}

fn startup_watcher_detail(snapshot: &HostReadyResponse, source_root: &Path) -> String {
    if snapshot.active_job.is_some() {
        return "background prebuild running".to_string();
    }
    if matches!(snapshot.phase.as_str(), "building" | "verifying" | "starting") {
        return "startup warmup in progress".to_string();
    }
    if !snapshot.scope_gate_ready && snapshot.access_ready {
        return "landing ready; manifest scope sweep still running in background".to_string();
    }
    if let Some(app_id) = snapshot.default_app_id.as_deref() {
        let gate =
            crate::readiness::scope_gate::resolve_default_app_access_gate(source_root, app_id);
        if !gate.access_ready {
            return gate
                .blockers
                .first()
                .cloned()
                .unwrap_or_else(|| format!("landing gate not ready for `{app_id}`"));
        }
    }
    "waiting for landing gate".to_string()
}

pub(crate) fn spawn_startup_status_watcher(source_root: PathBuf) {
    tokio::spawn(async move {
        let started = std::time::Instant::now();
        let mut ticks = 0u32;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            ticks += 1;
            let snapshot = registry_snapshot();
            if snapshot.access_ready {
                if ticks == 1 {
                    let note = if !snapshot.scope_gate_ready {
                        " (manifest scope sweep may still run in background)"
                    } else {
                        ""
                    };
                    tracing::info!(
                        target: "mei.startup",
                        elapsed_secs = started.elapsed().as_secs(),
                        phase = %snapshot.phase,
                        scope_gate_ready = snapshot.scope_gate_ready,
                        "✓ default app ACCESS READY{note} — /host and landing routes should work"
                    );
                }
                break;
            }
            let job = snapshot
                .active_job
                .as_deref()
                .unwrap_or("(none)")
                .to_string();
            let detail = startup_watcher_detail(&snapshot, source_root.as_path());
            tracing::info!(
                target: "mei.startup",
                elapsed_secs = started.elapsed().as_secs(),
                phase = %snapshot.phase,
                active_job = %job,
                host_bound = snapshot.host_ready,
                access_ready = snapshot.access_ready,
                scope_gate_ready = snapshot.scope_gate_ready,
                "{detail} — port is open; open /host now"
            );
            crate::prebuild::prebuild_emit_notice(format!(
                "⏳ startup {:.0}s | phase={} | job={job} | {detail} | /host available now",
                started.elapsed().as_secs_f64(),
                snapshot.phase,
            ));
            if ticks >= 360 {
                tracing::warn!(
                    target: "mei.startup",
                    "startup status watcher stopped after 1h; check runtime/prebuild-last.json"
                );
                break;
            }
            let _ = source_root;
        }
    });
}

pub(crate) fn spawn_startup_build(source_root: PathBuf) -> Result<()> {
    if let Some(reason) = startup_prebuild_skip_reason(source_root.as_path()) {
        tracing::info!(skip_reason = %reason, "startup prebuild skipped (recent prebuild still valid)");
        crate::prebuild::prebuild_emit_success_banner(
            "STARTUP PREBUILD SKIPPED",
            &[
                &reason,
                "host shell at /host — no duplicate build on serve startup",
            ],
        );
        startup_run::record_phase(
            "startup_prebuild_skipped",
            Some(serde_json::json!({ "reason": reason })),
        );
        spawn_startup_status_watcher(source_root.clone());
        if let Ok(Some(report)) = crate::prebuild::load_prebuild_report(source_root.as_path()) {
            tracing::info!(
                target: "mei.startup",
                app_count = report.succeeded_apps.len(),
                "startup report loaded (landing-only)"
            );
            crate::prebuild::prebuild_emit_notice(
                "startup report loaded (landing-only) — applying ACCESS gate now",
            );
            status_from_report(
                &report,
                None,
                false,
                ScopeGateRefreshMode::LandingOnly,
            );
            spawn_deferred_scope_gate_sweep(
                source_root.clone(),
                report.succeeded_apps.clone(),
            );
        }
        return Ok(());
    }
    begin_job(PrebuildMode::Build, None, "startup")?;
    startup_run::record_phase(
        "startup_prebuild_started",
        Some(serde_json::json!({
            "job": "startup:build:workspace",
            "scopeProfile": "hot_then_full",
            "mode": "build",
        })),
    );
    tracing::info!("startup background prebuild scheduled (hot-only then deferred full)");
    crate::prebuild::prebuild_emit_notice(
        "startup hot prebuild running — port is already open; default app may 503 until ACCESS READY banner",
    );
    let source_root_for_watcher = source_root.clone();
    tokio::spawn(async move {
        let source_root_hot = source_root.clone();
        let hot_result = tokio::task::spawn_blocking(move || {
            run_prebuild_job_sync_inner(
                source_root_hot.as_path(),
                PrebuildMode::Build,
                None,
                PrebuildScopeProfile::HotOnly,
            )
        })
        .await;
        match hot_result {
            Ok(Ok(hot_report)) => {
                status_from_report(
                    &hot_report,
                    None,
                    false,
                    ScopeGateRefreshMode::LandingOnly,
                );
                startup_run::record_phase(
                    "startup_prebuild_hot_finished",
                    Some(serde_json::json!({
                        "ok": hot_report.ok,
                        "totalWallMs": hot_report.total_wall_ms,
                    })),
                );
            }
            Ok(Err(error)) => {
                mark_job_failed(None, PrebuildMode::Build, &error.to_string(), false);
                return;
            }
            Err(error) => {
                mark_job_failed(
                    None,
                    PrebuildMode::Build,
                    &format!("startup hot build worker join failed: {error}"),
                    false,
                );
                return;
            }
        }
        let source_root_full = source_root.clone();
        let full_result = tokio::task::spawn_blocking(move || {
            run_prebuild_job_sync_inner(
                source_root_full.as_path(),
                PrebuildMode::Build,
                None,
                PrebuildScopeProfile::Full,
            )
        })
        .await;
        match full_result {
            Ok(Ok(report)) => status_from_report(&report, None, false, ScopeGateRefreshMode::Full),
            Ok(Err(error)) => mark_job_failed(None, PrebuildMode::Build, &error.to_string(), false),
            Err(error) => mark_job_failed(
                None,
                PrebuildMode::Build,
                &format!("startup full build worker join failed: {error}"),
                false,
            ),
        }
    });
    spawn_startup_status_watcher(source_root_for_watcher);
    Ok(())
}

pub(crate) fn spawn_manual_job(
    source_root: PathBuf,
    mode: PrebuildMode,
    app_filter: Option<String>,
    scope_profile: PrebuildScopeProfile,
) -> Result<String> {
    let app_filter_text = app_filter
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let job = begin_job(mode, app_filter_text, "manual")?;
    let app_filter_owned = app_filter_text.map(str::to_string);
    tokio::spawn(async move {
        let source_root_for_job = source_root.clone();
        let app_filter_for_job = app_filter_owned.clone();
        let report_result = tokio::task::spawn_blocking(move || {
            run_prebuild_job_sync_inner(
                source_root_for_job.as_path(),
                mode,
                app_filter_for_job.as_deref(),
                scope_profile,
            )
        })
        .await;
        match report_result {
            Ok(Ok(report)) => {
                status_from_report(
                    &report,
                    app_filter_owned.as_deref(),
                    false,
                    ScopeGateRefreshMode::Full,
                )
            }
            Ok(Err(error)) => {
                mark_job_failed(app_filter_owned.as_deref(), mode, &error.to_string(), false)
            }
            Err(error) => mark_job_failed(
                app_filter_owned.as_deref(),
                mode,
                &format!("manual host build worker join failed: {error}"),
                false,
            ),
        }
    });
    Ok(job)
}

pub(crate) fn artifact_gate_status(
    app_id: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
) -> ArtifactGateStatus {
    let snapshot = registry_snapshot();
    let app = snapshot.apps.iter().find(|app| app.app_id == app_id);
    let scope_key = normalize_scope_key(scene_id, target_file);
    let scope = app.and_then(|app| {
        app.scopes.iter().find(|scope| {
            normalize_scope_key(scope.scene_id.as_deref(), scope.target_file.as_deref())
                == scope_key
                || scope
                    .scene_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    == scene_id.map(str::trim).filter(|value| !value.is_empty())
        })
    });
    ArtifactGateStatus {
        host_phase: snapshot.phase,
        app_phase: app.map(|value| value.phase.clone()),
        scope_phase: scope.map(|value| value.phase.clone()),
        last_error: scope
            .and_then(|value| value.last_error.clone())
            .or_else(|| app.and_then(|value| value.last_error.clone()))
            .or_else(|| snapshot.error_summary.first().cloned()),
    }
}

pub(crate) fn access_scene_target_hint(app_id: &str, scene_id: &str) -> Option<String> {
    let normalized_scene = scene_id.trim();
    if normalized_scene.is_empty() {
        return None;
    }
    let canonical = mei_lang_kernel::canonical_app_source_rel_path(&format!(
        "scenes/{normalized_scene}.mei"
    ));
    let snapshot = registry_snapshot();
    let Some(app) = snapshot.apps.iter().find(|app| app.app_id == app_id) else {
        return Some(canonical);
    };
    let mut candidates = app
        .scopes
        .iter()
        .filter(|scope| {
            scope
                .scene_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                == Some(normalized_scene)
        })
        .filter_map(|scope| {
            scope
                .target_file
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Some(canonical);
    }
    candidates.sort();
    candidates.dedup();
    if let Some(hit) = candidates.iter().find(|target| {
        target.as_str() == canonical
            || mei_lang_kernel::canonical_app_source_rel_path(target.as_str()) == canonical
    }) {
        return Some(hit.clone());
    }
    candidates.into_iter().min_by_key(|target| {
        let cross_capsule_penalty = usize::from(
            (target.starts_with("scenes/") || target.starts_with("src/scenes/"))
                && target
                    .trim_start_matches("src/")
                    .strip_prefix("scenes/")
                    .and_then(|rest| rest.chars().next())
                    .is_some_and(|ch| ch.is_ascii_digit()),
        );
        (cross_capsule_penalty, target.len())
    })
}

