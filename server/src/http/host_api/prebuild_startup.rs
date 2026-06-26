use super::prelude::*;
use super::*;

pub(crate) fn startup_deferred_warmup_pending(source_root: &Path) -> bool {
    let Ok(Some(manifest)) = resolve_runtime_warmup_manifest(source_root) else {
        return false;
    };
    manifest.apps.iter().any(app_has_deferred_warmup_work)
}

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
            status_from_report(&report, None, false);
            Ok(report)
        }
        Err(error) => {
            let error_text = error.to_string();
            mark_job_failed(None, PrebuildMode::Verify, &error_text, false);
            Err(error)
        }
    }
}

pub(crate) fn spawn_startup_build(source_root: PathBuf) -> Result<()> {
    begin_job(PrebuildMode::Build, None, "startup")?;
    startup_run::record_phase(
        "startup_prebuild_started",
        Some(serde_json::json!({
            "job": "startup:build:workspace",
            "scopeProfile": if startup_deferred_warmup_pending(source_root.as_path()) {
                "hot_only"
            } else {
                "full"
            },
            "mode": "build",
        })),
    );
    tracing::info!("startup background prebuild scheduled");
    tokio::spawn(async move {
        let source_root_for_job = source_root.clone();
        let deferred_pending = startup_deferred_warmup_pending(source_root.as_path());
        let report_result = tokio::task::spawn_blocking(move || {
            run_prebuild_job_sync_inner(
                source_root_for_job.as_path(),
                PrebuildMode::Build,
                None,
                if deferred_pending {
                    PrebuildScopeProfile::HotOnly
                } else {
                    PrebuildScopeProfile::Full
                },
            )
        })
        .await;
        match report_result {
            Ok(Ok(report)) => {
                status_from_report(&report, None, deferred_pending);
                if deferred_pending && report.ok {
                    if let Err(error) = begin_job(PrebuildMode::Build, None, "startup_deferred") {
                        mark_job_failed(None, PrebuildMode::Build, &error.to_string(), true);
                        return;
                    }
                    startup_run::record_phase(
                        "startup_prebuild_started",
                        Some(serde_json::json!({
                            "job": "startup_deferred:build:workspace",
                            "scopeProfile": "full",
                            "mode": "build",
                        })),
                    );
                    let source_root_for_deferred = source_root.clone();
                    let deferred_result = tokio::task::spawn_blocking(move || {
                        run_prebuild_job_sync_inner(
                            source_root_for_deferred.as_path(),
                            PrebuildMode::Build,
                            None,
                            PrebuildScopeProfile::Full,
                        )
                    })
                    .await;
                    match deferred_result {
                        Ok(Ok(report)) => status_from_report(&report, None, false),
                        Ok(Err(error)) => {
                            mark_job_failed(None, PrebuildMode::Build, &error.to_string(), true)
                        }
                        Err(error) => mark_job_failed(
                            None,
                            PrebuildMode::Build,
                            &format!("startup deferred build worker join failed: {error}"),
                            true,
                        ),
                    }
                }
            }
            Ok(Err(error)) => mark_job_failed(None, PrebuildMode::Build, &error.to_string(), false),
            Err(error) => mark_job_failed(
                None,
                PrebuildMode::Build,
                &format!("startup build worker join failed: {error}"),
                false,
            ),
        }
    });
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
            Ok(Ok(report)) => status_from_report(&report, app_filter_owned.as_deref(), false),
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

