use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    prebuild::{run_prebuild, PrebuildAppReport, PrebuildMode, PrebuildOptions, PrebuildReport},
    AppState,
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostScopeReadinessResponse {
    #[serde(rename = "sceneId")]
    pub scene_id: Option<String>,
    #[serde(rename = "targetFile")]
    pub target_file: Option<String>,
    pub phase: String,
    #[serde(rename = "compileRevision")]
    pub compile_revision: Option<String>,
    #[serde(rename = "lastError")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostAppReadinessResponse {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub ready: bool,
    pub phase: String,
    #[serde(rename = "lastError")]
    pub last_error: Option<String>,
    pub warnings: Vec<String>,
    #[serde(rename = "compileScopeCount")]
    pub compile_scope_count: usize,
    #[serde(rename = "readyScopeCount")]
    pub ready_scope_count: usize,
    #[serde(rename = "failedScopeCount")]
    pub failed_scope_count: usize,
    pub scopes: Vec<HostScopeReadinessResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostReadyResponse {
    pub ready: bool,
    #[serde(rename = "hostReady")]
    pub host_ready: bool,
    #[serde(rename = "accessReady")]
    pub access_ready: bool,
    pub phase: String,
    #[serde(rename = "manifestPath")]
    pub manifest_path: String,
    #[serde(rename = "manifestSource")]
    pub manifest_source: String,
    #[serde(rename = "warmedApps")]
    pub warmed_apps: Vec<String>,
    #[serde(rename = "failedApps")]
    pub failed_apps: Vec<String>,
    #[serde(rename = "buildingApps")]
    pub building_apps: Vec<String>,
    #[serde(rename = "activeJob")]
    pub active_job: Option<String>,
    #[serde(rename = "errorSummary")]
    pub error_summary: Vec<String>,
    pub apps: Vec<HostAppReadinessResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostHeartbeatResponse {
    #[serde(rename = "buildVersion")]
    pub build_version: String,
    pub ready: bool,
    #[serde(rename = "hostReady")]
    pub host_ready: bool,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ArtifactGateStatus {
    #[serde(rename = "hostPhase")]
    pub host_phase: String,
    #[serde(rename = "appPhase")]
    pub app_phase: Option<String>,
    #[serde(rename = "scopePhase")]
    pub scope_phase: Option<String>,
    #[serde(rename = "lastError")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct HostBuildRequest {
    #[serde(default, rename = "appId")]
    pub app_id: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostBuildJobResponse {
    pub accepted: bool,
    pub phase: String,
    #[serde(rename = "activeJob")]
    pub active_job: String,
    #[serde(rename = "appId")]
    pub app_id: Option<String>,
    pub mode: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HostReadinessRegistry {
    host_bound: bool,
    phase: String,
    manifest_path: String,
    manifest_source: String,
    warmed_apps: Vec<String>,
    failed_apps: Vec<String>,
    building_apps: Vec<String>,
    error_summary: Vec<String>,
    active_job: Option<String>,
    apps: BTreeMap<String, HostAppReadinessState>,
}

#[derive(Debug, Clone, Default)]
struct HostAppReadinessState {
    phase: String,
    last_error: Option<String>,
    warnings: Vec<String>,
    scopes: BTreeMap<String, HostScopeReadinessState>,
}

#[derive(Debug, Clone, Default)]
struct HostScopeReadinessState {
    scene_id: Option<String>,
    target_file: Option<String>,
    phase: String,
    compile_revision: Option<String>,
    last_error: Option<String>,
}

fn host_readiness_registry() -> &'static Mutex<HostReadinessRegistry> {
    static STATUS: OnceLock<Mutex<HostReadinessRegistry>> = OnceLock::new();
    STATUS.get_or_init(|| Mutex::new(HostReadinessRegistry::default()))
}

fn with_registry<T>(f: impl FnOnce(&mut HostReadinessRegistry) -> T) -> Option<T> {
    host_readiness_registry().lock().ok().map(|mut guard| f(&mut guard))
}

fn manifest_path_for(source_root: &Path) -> PathBuf {
    source_root.join(mei_lang_kernel::WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL)
}

fn manifest_source_label(source_root: &Path) -> &'static str {
    if manifest_path_for(source_root).is_file() {
        "runtime_manifest"
    } else {
        "workspace_config_fallback"
    }
}

fn phase_ready(phase: &str) -> bool {
    matches!(phase, "ready" | "degraded" | "skipped")
}

fn normalize_scope_key(scene_id: Option<&str>, target_file: Option<&str>) -> String {
    format!(
        "{}|{}",
        scene_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(""),
        target_file
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("")
    )
}

fn scope_response_from_state(scope: HostScopeReadinessState) -> HostScopeReadinessResponse {
    HostScopeReadinessResponse {
        scene_id: scope.scene_id,
        target_file: scope.target_file,
        phase: if scope.phase.trim().is_empty() {
            "missing".to_string()
        } else {
            scope.phase
        },
        compile_revision: scope.compile_revision,
        last_error: scope.last_error,
    }
}

fn app_response(app_id: String, state: HostAppReadinessState) -> HostAppReadinessResponse {
    let scopes = state
        .scopes
        .into_values()
        .map(scope_response_from_state)
        .collect::<Vec<_>>();
    let ready_scope_count = scopes
        .iter()
        .filter(|scope| phase_ready(scope.phase.as_str()))
        .count();
    let failed_scope_count = scopes
        .iter()
        .filter(|scope| matches!(scope.phase.as_str(), "failed"))
        .count();
    HostAppReadinessResponse {
        app_id,
        ready: phase_ready(state.phase.as_str()),
        phase: if state.phase.trim().is_empty() {
            "pending".to_string()
        } else {
            state.phase
        },
        last_error: state.last_error,
        warnings: state.warnings,
        compile_scope_count: scopes.len(),
        ready_scope_count,
        failed_scope_count,
        scopes,
    }
}

fn registry_snapshot() -> HostReadyResponse {
    let snapshot = host_readiness_registry()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let apps = snapshot
        .apps
        .into_iter()
        .map(|(app_id, state)| app_response(app_id, state))
        .collect::<Vec<_>>();
    let access_ready = matches!(snapshot.phase.as_str(), "ready" | "skipped");
    HostReadyResponse {
        ready: access_ready,
        host_ready: snapshot.host_bound,
        access_ready,
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
        error_summary: snapshot.error_summary,
        apps,
    }
}

fn reset_registry_for_source_root(source_root: &Path) {
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
            phase: "starting".to_string(),
            manifest_path: manifest_path.display().to_string(),
            manifest_source,
            warmed_apps: Vec::new(),
            failed_apps: Vec::new(),
            building_apps: Vec::new(),
            error_summary: Vec::new(),
            active_job: None,
            apps,
        };
    });
}

fn set_selected_apps_phase(
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

fn sync_registry_phase(registry: &mut HostReadinessRegistry) {
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
        .filter(|app| phase_ready(app.phase.as_str()))
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
        .filter_map(|(app_id, app)| phase_ready(app.phase.as_str()).then_some(app_id.clone()))
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
    registry.phase = if failed_count > 0 && ready_count > 0 {
        "degraded".to_string()
    } else if failed_count > 0 && building_count == 0 {
        "failed".to_string()
    } else if building_count > 0 {
        "building".to_string()
    } else if ready_count == registry.apps.len() {
        "ready".to_string()
    } else if registry.host_bound {
        "bound".to_string()
    } else {
        "starting".to_string()
    };
}

fn apply_success_app_report(app_report: &PrebuildAppReport, app_state: &mut HostAppReadinessState) {
    app_state.phase = if app_report.warnings.is_empty() {
        "ready".to_string()
    } else {
        "degraded".to_string()
    };
    app_state.last_error = None;
    app_state.warnings = app_report.warnings.clone();
    let mut seen = BTreeSet::new();
    for scope in &app_report.compile_scopes {
        let key = normalize_scope_key(
            scope
                .requested_scene_id
                .as_deref()
                .or(scope.active_scene_id.as_deref()),
            Some(scope.active_target_file.as_str())
                .or(scope.requested_target_file.as_deref()),
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

fn status_from_report(report: &PrebuildReport, app_filter: Option<&str>) {
    let _ = with_registry(|registry| {
        registry.manifest_path = report.manifest_path.clone();
        registry.manifest_source = report.manifest_source.clone();
        registry.error_summary = report.error_summary.clone();
        for app_report in &report.apps {
            let app_state = registry.apps.entry(app_report.app_id.clone()).or_default();
            apply_success_app_report(app_report, app_state);
        }
        for app_id in &report.failed_apps {
            let app_state = registry.apps.entry(app_id.clone()).or_default();
            app_state.phase = "failed".to_string();
            app_state.last_error = report
                .error_summary
                .iter()
                .find_map(|line| line.strip_prefix(&format!("{app_id}: ")).map(str::to_string))
                .or_else(|| Some("prebuild failed".to_string()));
        }
        if let Some(app_id) = app_filter.map(str::trim).filter(|value| !value.is_empty()) {
            if !report.succeeded_apps.iter().any(|value| value == app_id)
                && !report.failed_apps.iter().any(|value| value == app_id)
            {
                let app_state = registry.apps.entry(app_id.to_string()).or_default();
                app_state.phase = "failed".to_string();
                app_state.last_error = Some("requested app missing from prebuild report".to_string());
            }
        }
        registry.active_job = None;
        sync_registry_phase(registry);
    });
}

fn mark_job_failed(app_filter: Option<&str>, mode: PrebuildMode, error: &str) {
    let _ = with_registry(|registry| {
        registry.error_summary = vec![error.to_string()];
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
    });
}

fn begin_job(mode: PrebuildMode, app_filter: Option<&str>, origin: &str) -> Result<String> {
    with_registry(|registry| {
        if registry.active_job.is_some() {
            return Err(anyhow!("host build job is already running"));
        }
        let mode_label = match mode {
            PrebuildMode::Build => "build",
            PrebuildMode::Verify => "verify",
        };
        let job = if let Some(app_id) = app_filter.map(str::trim).filter(|value| !value.is_empty()) {
            format!("{origin}:{mode_label}:{app_id}")
        } else {
            format!("{origin}:{mode_label}:workspace")
        };
        registry.active_job = Some(job.clone());
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

fn run_prebuild_job_sync_inner(
    source_root: &Path,
    mode: PrebuildMode,
    app_filter: Option<&str>,
) -> Result<PrebuildReport> {
    run_prebuild(
        source_root,
        &PrebuildOptions {
            app_filter: app_filter
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            mode,
            clean: false,
        },
    )
}

pub(crate) fn initialize_startup_readiness(source_root: &Path) {
    reset_registry_for_source_root(source_root);
}

pub(crate) fn mark_host_bound() {
    let _ = with_registry(|registry| {
        registry.host_bound = true;
        sync_registry_phase(registry);
    });
}

pub(crate) fn verify_startup_artifacts(source_root: &Path) -> Result<PrebuildReport> {
    if cfg!(test) {
        let report = PrebuildReport {
            schema_version: "mei-prebuild-report-v1".to_string(),
            mode: PrebuildMode::Verify,
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
    match run_prebuild_job_sync_inner(source_root, PrebuildMode::Verify, None) {
        Ok(report) => {
            status_from_report(&report, None);
            Ok(report)
        }
        Err(error) => {
            let error_text = error.to_string();
            mark_job_failed(None, PrebuildMode::Verify, &error_text);
            Err(error)
        }
    }
}

pub(crate) fn spawn_startup_build(source_root: PathBuf) -> Result<()> {
    begin_job(PrebuildMode::Build, None, "startup")?;
    tokio::spawn(async move {
        let source_root_for_job = source_root.clone();
        let report_result = tokio::task::spawn_blocking(move || {
            run_prebuild_job_sync_inner(source_root_for_job.as_path(), PrebuildMode::Build, None)
        })
        .await;
        match report_result {
            Ok(Ok(report)) => status_from_report(&report, None),
            Ok(Err(error)) => mark_job_failed(None, PrebuildMode::Build, &error.to_string()),
            Err(error) => mark_job_failed(
                None,
                PrebuildMode::Build,
                &format!("startup build worker join failed: {error}"),
            ),
        }
    });
    Ok(())
}

fn spawn_manual_job(source_root: PathBuf, mode: PrebuildMode, app_filter: Option<String>) -> Result<String> {
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
            )
        })
        .await;
        match report_result {
            Ok(Ok(report)) => status_from_report(&report, app_filter_owned.as_deref()),
            Ok(Err(error)) => {
                mark_job_failed(app_filter_owned.as_deref(), mode, &error.to_string())
            }
            Err(error) => mark_job_failed(
                app_filter_owned.as_deref(),
                mode,
                &format!("manual host build worker join failed: {error}"),
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
            normalize_scope_key(scope.scene_id.as_deref(), scope.target_file.as_deref()) == scope_key
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

pub async fn api_host_ready() -> impl IntoResponse {
    let response = registry_snapshot();
    let status = if response.access_ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(response))
}

pub async fn api_host_readiness() -> impl IntoResponse {
    Json(registry_snapshot())
}

pub async fn api_host_heartbeat() -> impl IntoResponse {
    let ready = registry_snapshot();
    Json(HostHeartbeatResponse {
        build_version: crate::build_info::BUILD_VERSION.to_string(),
        ready: ready.access_ready,
        host_ready: ready.host_ready,
        phase: ready.phase,
    })
}

pub async fn api_host_build(
    State(state): State<AppState>,
    Json(request): Json<HostBuildRequest>,
) -> impl IntoResponse {
    let mode = match request
        .mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("build")
    {
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
    match spawn_manual_job(state.source_root.as_ref().clone(), mode, request.app_id.clone()) {
        Ok(job) => (
            StatusCode::ACCEPTED,
            Json(HostBuildJobResponse {
                accepted: true,
                phase: registry_snapshot().phase,
                active_job: job,
                app_id: request.app_id,
                mode: match mode {
                    PrebuildMode::Build => "build".to_string(),
                    PrebuildMode::Verify => "verify".to_string(),
                },
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
    fn host_heartbeat_response_includes_build_version() {
        let ready = registry_snapshot();
        let heartbeat = HostHeartbeatResponse {
            build_version: crate::build_info::BUILD_VERSION.to_string(),
            ready: ready.ready,
            host_ready: ready.host_ready,
            phase: ready.phase,
        };
        assert!(!heartbeat.build_version.is_empty());
        assert_eq!(heartbeat.build_version, crate::build_info::BUILD_VERSION);
    }

    #[test]
    fn manifest_source_label_distinguishes_runtime_file_and_fallback() {
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
    fn registry_phase_becomes_degraded_when_apps_partially_fail() {
        let mut registry = HostReadinessRegistry {
            host_bound: true,
            apps: BTreeMap::from([
                (
                    "ready-app".to_string(),
                    HostAppReadinessState {
                        phase: "ready".to_string(),
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
