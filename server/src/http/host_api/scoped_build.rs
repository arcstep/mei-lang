use super::prelude::*;
use super::*;

pub(crate) fn normalized_optional_scope(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn compile_feedback_from_compiled(compiled: &CompiledApp) -> (usize, usize, Option<String>) {
    let diagnostic_error_count = compiled
        .diagnostics
        .iter()
        .filter(|diag| matches!(diag.severity, Severity::Error))
        .count();
    let warning_count = compiled
        .diagnostics
        .iter()
        .filter(|diag| matches!(diag.severity, Severity::Warning))
        .count();
    let diagnostic_summary = compiled
        .diagnostics
        .iter()
        .find(|diag| matches!(diag.severity, Severity::Error))
        .map(|diag| {
            let source = diag
                .source_path
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("unknown");
            format!("{} @ {}: {}", diag.code, source, diag.message)
        });
    (diagnostic_error_count, warning_count, diagnostic_summary)
}

pub(crate) fn summarize_scoped_compile_feedback(
    outcome: CompileWithCacheOutcome,
) -> ScopedCompileFeedback {
    let (diagnostic_error_count, warning_count, diagnostic_summary) =
        compile_feedback_from_compiled(&outcome.compiled);
    ScopedCompileFeedback {
        status: if diagnostic_error_count > 0 {
            ScopedFeedbackStatus::DiagnosticError
        } else {
            ScopedFeedbackStatus::Ready
        },
        outcome: Some(outcome),
        diagnostic_error_count,
        warning_count,
        diagnostic_summary,
    }
}

pub(crate) fn inspect_scoped_artifact(
    state: &AppState,
    app_id: &str,
    scene_id: Option<String>,
    target_file: Option<String>,
) -> ScopedCompileFeedback {
    let components_root = resolve_components_root(state.source_root.as_ref().as_path());
    let mut options = CompileOptions {
        scene: normalized_optional_scope(scene_id),
        preview_target: normalized_optional_scope(target_file),
        ..Default::default()
    };
    if options.preview_target.is_none() {
        if let Some(scene) = options.scene.as_deref() {
            if let Some(hint) = access_scene_target_hint(app_id, scene) {
                options.preview_target = Some(hint);
            }
        }
    }
    let access_policies = RuntimeAccessPolicies::default_for_access_host();
    match resolve_runtime_compile_shared(
        state,
        app_id,
        &options,
        components_root.as_path(),
        access_policies,
        mei_lang_app::UiRouteMode::App,
    ) {
        Ok(Some(resolution)) => {
            summarize_scoped_compile_feedback(compile_outcome_from_shared(resolution.outcome))
        }
        Ok(None) | Err(_) => ScopedCompileFeedback {
            status: ScopedFeedbackStatus::ArtifactMissing,
            outcome: None,
            diagnostic_error_count: 0,
            warning_count: 0,
            diagnostic_summary: None,
        },
    }
}

pub(crate) fn record_scoped_compile_feedback(
    app_id: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
    feedback: &ScopedCompileFeedback,
) {
    let normalized_scene = scene_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let normalized_target = target_file
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if normalized_scene.is_none() && normalized_target.is_none() {
        return;
    }
    let phase = match feedback.status {
        ScopedFeedbackStatus::Ready => "ready",
        ScopedFeedbackStatus::ArtifactMissing => "missing",
        ScopedFeedbackStatus::DiagnosticError => "degraded",
    };
    let _ = with_registry(|registry| {
        let app_state = registry.apps.entry(app_id.to_string()).or_default();
        let key = normalize_scope_key(normalized_scene.as_deref(), normalized_target.as_deref());
        let scope = app_state.scopes.entry(key).or_default();
        scope.scene_id = normalized_scene.clone();
        scope.target_file = normalized_target.clone();
        scope.phase = phase.to_string();
        scope.compile_revision = feedback
            .outcome
            .as_ref()
            .map(|outcome| outcome.compile_revision.clone());
        scope.last_error = match feedback.status {
            ScopedFeedbackStatus::Ready => None,
            ScopedFeedbackStatus::ArtifactMissing => Some("artifact missing".to_string()),
            ScopedFeedbackStatus::DiagnosticError => feedback.diagnostic_summary.clone(),
        };
        if matches!(feedback.status, ScopedFeedbackStatus::DiagnosticError) {
            app_state.last_error = feedback.diagnostic_summary.clone();
        } else if matches!(feedback.status, ScopedFeedbackStatus::Ready) {
            app_state.last_error = None;
        }
        sync_registry_phase(registry);
    });
}

pub(crate) fn scoped_response_status(status: ScopedFeedbackStatus) -> StatusCode {
    match status {
        ScopedFeedbackStatus::Ready => StatusCode::OK,
        ScopedFeedbackStatus::ArtifactMissing => StatusCode::NOT_FOUND,
        ScopedFeedbackStatus::DiagnosticError => StatusCode::CONFLICT,
    }
}

pub(crate) fn host_build_response_from_scoped_feedback(
    app_id: &str,
    mode: &str,
    scene_id: Option<String>,
    target_file: Option<String>,
    feedback: ScopedCompileFeedback,
    materialize: Option<crate::prebuild::ScopedMaterializeReport>,
) -> HostBuildJobResponse {
    let compile_revision = feedback
        .outcome
        .as_ref()
        .map(|outcome| outcome.compile_revision.clone());
    let compile_ms = feedback.outcome.as_ref().map(|outcome| outcome.compile_ms);
    let cache_hit = feedback.outcome.as_ref().map(|outcome| outcome.cache_hit);
    let artifact_cache_hit = feedback
        .outcome
        .as_ref()
        .map(|outcome| outcome.artifact_cache_hit);
    HostBuildJobResponse {
        accepted: feedback.status.artifact_ready(),
        phase: registry_snapshot().phase,
        active_job: None,
        app_id: Some(app_id.to_string()),
        mode: mode.to_string(),
        scope_profile: "scoped_aot_build".to_string(),
        status: feedback.status.as_str().to_string(),
        artifact_ready: feedback.status.artifact_ready(),
        diagnostic_error_count: feedback.diagnostic_error_count,
        warning_count: feedback.warning_count,
        diagnostic_summary: feedback.diagnostic_summary.clone(),
        scoped_build: true,
        scene_id,
        target_file,
        compile_revision,
        compile_ms,
        cache_hit,
        artifact_cache_hit,
        scope_artifacts_ms: materialize.as_ref().map(|report| report.scope_artifacts_ms),
        mrg_slots_ready: materialize.as_ref().map(|report| report.mrg_slots_ready),
        eval_artifacts_warmed: materialize.as_ref().map(|report| report.eval_artifacts_warmed),
        block_eval_hint: materialize.as_ref().and_then(|report| report.block_eval_hint.clone()),
    }
}

pub(crate) fn run_scoped_build(
    state: &AppState,
    app_id: &str,
    scene_id: Option<String>,
    target_file: Option<String>,
) -> Result<HostBuildJobResponse> {
    let scene_id = normalized_optional_scope(scene_id);
    let target_file = normalized_optional_scope(target_file);
    let components_root = resolve_components_root(state.source_root.as_ref().as_path());
    let options = CompileOptions {
        scene: scene_id.clone(),
        preview_target: target_file.clone(),
        ..Default::default()
    };
    let outcome = compile_app_with_cache(state, app_id, &options, components_root.as_path())
        .map_err(|failure| failure.error)?;
    let materialize = crate::prebuild::materialize_scope_after_compile(
        state.source_root.as_path(),
        app_id,
        scene_id.as_deref(),
        target_file.as_deref(),
        &outcome,
        PrebuildMode::Build,
    )
    .ok();
    if let Some(ref report) = materialize {
        tracing::info!(
            target: "mei.scoped_build",
            app_id = %app_id,
            scene_id = ?scene_id,
            target_file = ?target_file,
            eval_warmed = report.eval_artifacts_warmed,
            scope_ms = report.scope_artifacts_ms,
            hint = ?report.block_eval_hint,
            "scoped build materialize complete"
        );
    }
    let feedback = summarize_scoped_compile_feedback(outcome);
    record_scoped_compile_feedback(
        app_id,
        scene_id.as_deref(),
        target_file.as_deref(),
        &feedback,
    );
    if let Some(scene) = scene_id.as_deref() {
        crate::graph::schedule_warmup_frontier(
            state.source_root.as_path(),
            app_id,
            scene,
        );
    }
    Ok(host_build_response_from_scoped_feedback(
        app_id,
        "scope-build",
        scene_id,
        target_file,
        feedback,
        materialize,
    ))
}

pub(crate) fn mark_access_artifact_degraded(
    app_id: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
    error: &str,
) {
    let normalized_scene = scene_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let normalized_target = target_file
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let normalized_error = error.trim();
    if normalized_error.is_empty() {
        return;
    }
    let _ = with_registry(|registry| {
        let app_state = registry.apps.entry(app_id.to_string()).or_default();
        app_state.last_error = Some(normalized_error.to_string());
        if !app_state
            .warnings
            .iter()
            .any(|warning| warning == normalized_error)
        {
            app_state.warnings.push(normalized_error.to_string());
        }
        if normalized_scene.is_some() || normalized_target.is_some() {
            let key =
                normalize_scope_key(normalized_scene.as_deref(), normalized_target.as_deref());
            let scope = app_state.scopes.entry(key).or_default();
            scope.scene_id = normalized_scene.clone().or(scope.scene_id.clone());
            scope.target_file = normalized_target.clone().or(scope.target_file.clone());
            scope.phase = "degraded".to_string();
            scope.last_error = Some(normalized_error.to_string());
        }
        sync_registry_phase(registry);
    });
}

