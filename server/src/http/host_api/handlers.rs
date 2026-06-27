use super::prelude::*;
use super::*;

pub async fn api_host_ready() -> impl IntoResponse {
    let response = registry_snapshot();
    let status = if response.host_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(response))
}

pub async fn api_host_readiness(State(state): State<AppState>) -> impl IntoResponse {
    Json(registry_snapshot_with_scope_gate(Some(state.source_root.as_path())))
}

#[derive(Debug, Deserialize)]
pub struct HostDiagnosticsQuery {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub sections: Option<String>,
    #[serde(rename = "sceneId")]
    pub scene_id: Option<String>,
    #[serde(rename = "targetFile")]
    pub target_file: Option<String>,
}

pub async fn api_host_diagnostics(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<HostDiagnosticsQuery>,
) -> impl IntoResponse {
    let sections = query
        .sections
        .as_deref()
        .map(|text| {
            text.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let report = crate::diagnostics::collect_materialization_diagnostics(
        state.source_root.as_path(),
        query.app_id.as_str(),
        sections.as_slice(),
        query.scene_id.as_deref(),
        query.target_file.as_deref(),
    );
    Json(report)
}

#[derive(Debug, Deserialize)]
pub struct HostGraphQuery {
    #[serde(rename = "appId")]
    pub app_id: Option<String>,
}

pub async fn api_host_graph_status(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<HostGraphQuery>,
) -> impl IntoResponse {
    let report = crate::graph::run_graph_status(
        state.source_root.as_path(),
        query.app_id.as_deref(),
    );
    Json(report)
}

pub async fn api_host_graph_doctor(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<HostGraphQuery>,
) -> impl IntoResponse {
    let app_id = query
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("zhifa");
    let report = crate::graph::run_graph_doctor(state.source_root.as_path(), app_id);
    let status = if report.ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(report))
}

pub async fn api_host_heartbeat() -> impl IntoResponse {
    let ready = registry_snapshot();
    Json(HostHeartbeatResponse {
        build_version: crate::build_info::BUILD_VERSION.to_string(),
        run_id: ready.run_id,
        startup_policy: ready.startup_policy,
        build_descriptor: ready.build_descriptor,
        startup_artifact_dir: ready.startup_artifact_dir,
        host_started_at_ms: ready.host_started_at_ms,
        ready: ready.host_ready,
        host_ready: ready.host_ready,
        access_ready: ready.access_ready,
        default_app_id: ready.default_app_id,
        default_app_access_ready: ready.default_app_access_ready,
        any_app_access_ready: ready.any_app_access_ready,
        full_warmup_ready: ready.full_warmup_ready,
        deferred_warmup_pending: ready.deferred_warmup_pending,
        phase: ready.phase,
        active_job: ready.active_job,
        active_job_elapsed_ms: ready.active_job_elapsed_ms,
        last_build_total_ms: ready.last_build_total_ms,
        last_build_compile_ms: ready.last_build_compile_ms,
        last_build_warmup_ms: ready.last_build_warmup_ms,
        last_critical_warmup_ms: ready.last_critical_warmup_ms,
        last_deferred_warmup_ms: ready.last_deferred_warmup_ms,
        last_critical_warmup_request_count: ready.last_critical_warmup_request_count,
        last_deferred_warmup_request_count: ready.last_deferred_warmup_request_count,
        last_warning_count: ready.last_warning_count,
        last_build_diagnostics: ready.last_build_diagnostics,
        correctness_failed: ready.correctness_failed,
        warning_categories: ready.warning_categories,
        warning_category_counts: ready.warning_category_counts,
        failing_datasets: ready.failing_datasets,
        apps: ready
            .apps
            .iter()
            .map(|app| HostHeartbeatAppSummary {
                app_id: app.app_id.clone(),
                phase: app.phase.clone(),
                access_ready: app.access_ready,
            })
            .collect(),
    })
}

pub async fn api_host_build(
    State(state): State<AppState>,
    Json(request): Json<HostBuildRequest>,
) -> impl IntoResponse {
    let mode_text = request
        .mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("build");
    let mode = match mode_text {
        "build" => PrebuildMode::Build,
        "verify" => PrebuildMode::Verify,
        other => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("unsupported host build mode `{other}`; expected `build` or `verify`")
                })),
            )
                .into_response();
        }
    };
    let scene_id = normalized_optional_scope(request.scene_id.clone());
    let target_file = normalized_optional_scope(request.target_file.clone());
    let scope_requested = scene_id.is_some() || target_file.is_some();
    if scope_requested {
        let Some(app_id) = request
            .app_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "accepted": false,
                    "error": "scoped host build requires `appId`",
                })),
            )
                .into_response();
        };
        if mode == PrebuildMode::Verify {
            let feedback =
                inspect_scoped_artifact(&state, app_id, scene_id.clone(), target_file.clone());
            record_scoped_compile_feedback(
                app_id,
                scene_id.as_deref(),
                target_file.as_deref(),
                &feedback,
            );
            let status = scoped_response_status(feedback.status);
            return (
                status,
                Json(host_build_response_from_scoped_feedback(
                    app_id,
                    "scope-verify",
                    scene_id,
                    target_file,
                    feedback,
                    None,
                )),
            )
                .into_response();
        }
        return match run_scoped_build(&state, app_id, scene_id, target_file) {
            Ok(response) => (
                if response.status == ScopedFeedbackStatus::DiagnosticError.as_str() {
                    StatusCode::CONFLICT
                } else {
                    StatusCode::OK
                },
                Json(response),
            )
                .into_response(),
            Err(error) => (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "accepted": false,
                    "error": error.to_string(),
                })),
            )
                .into_response(),
        };
    }
    let scope_profile = if request.hot_only {
        PrebuildScopeProfile::HotOnly
    } else {
        PrebuildScopeProfile::Full
    };
    match spawn_manual_job(
        state.source_root.as_ref().clone(),
        mode,
        request.app_id.clone(),
        scope_profile,
    ) {
        Ok(job) => (
            StatusCode::ACCEPTED,
            Json(HostBuildJobResponse {
                accepted: true,
                phase: registry_snapshot().phase,
                active_job: Some(job),
                app_id: request.app_id,
                mode: match mode {
                    PrebuildMode::Build => "build".to_string(),
                    PrebuildMode::Verify => "verify".to_string(),
                },
                scope_profile: match scope_profile {
                    PrebuildScopeProfile::Full => "full".to_string(),
                    PrebuildScopeProfile::HotOnly => "hot_only".to_string(),
                    PrebuildScopeProfile::BlockScoped => "block_scoped".to_string(),
                },
                status: "accepted".to_string(),
                artifact_ready: false,
                diagnostic_error_count: 0,
                warning_count: 0,
                diagnostic_summary: None,
                scoped_build: false,
                scene_id: None,
                target_file: None,
                compile_revision: None,
                compile_ms: None,
                cache_hit: None,
                artifact_cache_hit: None,
                scope_artifacts_ms: None,
                mrg_slots_ready: None,
                eval_artifacts_warmed: None,
                block_eval_hint: None,
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "accepted": false,
                "error": error.to_string(),
                "phase": registry_snapshot().phase,
                "activeJob": registry_snapshot().active_job,
            })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    pub(crate) fn host_heartbeat_response_includes_build_version() {
        let ready = registry_snapshot();
        let heartbeat = HostHeartbeatResponse {
            build_version: crate::build_info::BUILD_VERSION.to_string(),
            run_id: ready.run_id,
            startup_policy: ready.startup_policy,
            build_descriptor: ready.build_descriptor,
            startup_artifact_dir: ready.startup_artifact_dir,
            host_started_at_ms: ready.host_started_at_ms,
            ready: ready.ready,
            host_ready: ready.host_ready,
            access_ready: ready.access_ready,
            default_app_id: ready.default_app_id,
            default_app_access_ready: ready.default_app_access_ready,
            any_app_access_ready: ready.any_app_access_ready,
            full_warmup_ready: ready.full_warmup_ready,
            deferred_warmup_pending: ready.deferred_warmup_pending,
            phase: ready.phase,
            active_job: ready.active_job,
            active_job_elapsed_ms: ready.active_job_elapsed_ms,
            last_build_total_ms: ready.last_build_total_ms,
            last_build_compile_ms: ready.last_build_compile_ms,
            last_build_warmup_ms: ready.last_build_warmup_ms,
            last_critical_warmup_ms: ready.last_critical_warmup_ms,
            last_deferred_warmup_ms: ready.last_deferred_warmup_ms,
            last_critical_warmup_request_count: ready.last_critical_warmup_request_count,
            last_deferred_warmup_request_count: ready.last_deferred_warmup_request_count,
            last_warning_count: ready.last_warning_count,
            last_build_diagnostics: ready.last_build_diagnostics,
            correctness_failed: ready.correctness_failed,
            warning_categories: ready.warning_categories,
            warning_category_counts: ready.warning_category_counts,
            failing_datasets: ready.failing_datasets,
            apps: ready
                .apps
                .iter()
                .map(|app| HostHeartbeatAppSummary {
                    app_id: app.app_id.clone(),
                    phase: app.phase.clone(),
                    access_ready: app.access_ready,
                })
                .collect(),
        };
        assert!(!heartbeat.build_version.is_empty());
        assert_eq!(heartbeat.build_version, crate::build_info::BUILD_VERSION);
    }

    #[test]
    pub(crate) fn manifest_source_label_distinguishes_runtime_file_and_fallback() {
        let root = std::env::temp_dir().join(format!(
            "mei-host-warmup-source-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        assert_eq!(
            manifest_source_label(root.as_path()),
            "workspace_config_fallback"
        );

        let manifest_path = manifest_path_for(root.as_path());
        if let Some(parent) = manifest_path.parent() {
            fs::create_dir_all(parent).expect("create manifest parent");
        }
        fs::write(&manifest_path, r#"{"enabled":false,"apps":[]}"#).expect("write manifest");
        assert_eq!(manifest_source_label(root.as_path()), "runtime_manifest");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    pub(crate) fn registry_phase_becomes_degraded_when_apps_partially_fail() {
        let mut registry = HostReadinessRegistry {
            host_bound: true,
            access_ready: true,
            apps: BTreeMap::from([
                (
                    "ready-app".to_string(),
                    HostAppReadinessState {
                        phase: "ready".to_string(),
                        access_ready: true,
                        ..Default::default()
                    },
                ),
                (
                    "failed-app".to_string(),
                    HostAppReadinessState {
                        phase: "failed".to_string(),
                        last_error: Some("boom".to_string()),
                        ..Default::default()
                    },
                ),
            ]),
            ..Default::default()
        };
        sync_registry_phase(&mut registry);
        assert_eq!(registry.phase, "degraded");
        assert_eq!(registry.warmed_apps, vec!["ready-app".to_string()]);
        assert_eq!(registry.failed_apps, vec!["failed-app".to_string()]);
    }
}
