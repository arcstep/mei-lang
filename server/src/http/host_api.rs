use std::collections::{BTreeMap, BTreeSet};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use anyhow::{anyhow, Result};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    http::compile_cache::{
        compile_app_with_cache, compile_outcome_from_shared, resolve_runtime_compile_shared,
        CompileWithCacheOutcome, RuntimeAccessPolicies,
    },
    http::startup_run,
    prebuild::{
        app_has_deferred_warmup_work, run_prebuild, PrebuildAppReport, PrebuildDiagnosticsReport,
        PrebuildMode, PrebuildOptions, PrebuildReport, PrebuildScopeProfile,
        PrebuildWarningReport,
    },
    AppState,
};
use mei_lang_datasets::preload_prebuild_metric_response_index;
use mei_lang_kernel::{
    resolve_app_root, resolve_runtime_warmup_manifest, CompileOptions, CompiledApp, Severity,
};
use mei_lang_toolchain::resolve_components_root;

fn supports_ansi_stderr() -> bool {
    std::io::stderr().is_terminal()
}

fn ansi_wrap(text: &str, code: &str) -> String {
    if supports_ansi_stderr() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn emit_prebuild_status_line(status: &str, color_code: &str, detail: &str) {
    let prefix = ansi_wrap(status, color_code);
    eprintln!("{prefix} {detail}");
    let _ = std::io::stderr().flush();
}

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
    #[serde(rename = "warningDetails")]
    pub warning_details: Vec<PrebuildWarningReport>,
    #[serde(rename = "warningCategories")]
    pub warning_categories: Vec<String>,
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
    #[serde(rename = "runId")]
    pub run_id: Option<String>,
    #[serde(rename = "startupPolicy")]
    pub startup_policy: Option<String>,
    #[serde(rename = "buildDescriptor")]
    pub build_descriptor: serde_json::Value,
    #[serde(rename = "startupArtifactDir")]
    pub startup_artifact_dir: Option<String>,
    #[serde(rename = "hostStartedAtMs")]
    pub host_started_at_ms: Option<u64>,
    #[serde(rename = "hostReady")]
    pub host_ready: bool,
    #[serde(rename = "accessReady")]
    pub access_ready: bool,
    #[serde(rename = "fullWarmupReady")]
    pub full_warmup_ready: bool,
    #[serde(rename = "deferredWarmupPending")]
    pub deferred_warmup_pending: bool,
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
    #[serde(rename = "activeJobElapsedMs")]
    pub active_job_elapsed_ms: Option<u64>,
    #[serde(rename = "lastBuildTotalMs")]
    pub last_build_total_ms: Option<u64>,
    #[serde(rename = "lastBuildCompileMs")]
    pub last_build_compile_ms: Option<u64>,
    #[serde(rename = "lastBuildWarmupMs")]
    pub last_build_warmup_ms: Option<u64>,
    #[serde(rename = "lastCriticalWarmupMs")]
    pub last_critical_warmup_ms: Option<u64>,
    #[serde(rename = "lastDeferredWarmupMs")]
    pub last_deferred_warmup_ms: Option<u64>,
    #[serde(rename = "lastCriticalWarmupRequestCount")]
    pub last_critical_warmup_request_count: usize,
    #[serde(rename = "lastDeferredWarmupRequestCount")]
    pub last_deferred_warmup_request_count: usize,
    #[serde(rename = "lastWarningCount")]
    pub last_warning_count: usize,
    #[serde(rename = "lastBuildDiagnostics")]
    pub last_build_diagnostics: Option<PrebuildDiagnosticsReport>,
    #[serde(rename = "correctnessFailed")]
    pub correctness_failed: bool,
    #[serde(rename = "warningCategories")]
    pub warning_categories: Vec<String>,
    #[serde(rename = "warningCategoryCounts")]
    pub warning_category_counts: BTreeMap<String, usize>,
    #[serde(rename = "failingDatasets")]
    pub failing_datasets: Vec<String>,
    #[serde(rename = "readyAppCount")]
    pub ready_app_count: usize,
    #[serde(rename = "degradedAppCount")]
    pub degraded_app_count: usize,
    #[serde(rename = "failedAppCount")]
    pub failed_app_count: usize,
    #[serde(rename = "errorSummary")]
    pub error_summary: Vec<String>,
    pub apps: Vec<HostAppReadinessResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_gate: Option<crate::readiness::scope_gate::ScopeGateReport>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostHeartbeatResponse {
    #[serde(rename = "buildVersion")]
    pub build_version: String,
    #[serde(rename = "runId")]
    pub run_id: Option<String>,
    #[serde(rename = "startupPolicy")]
    pub startup_policy: Option<String>,
    #[serde(rename = "buildDescriptor")]
    pub build_descriptor: serde_json::Value,
    #[serde(rename = "startupArtifactDir")]
    pub startup_artifact_dir: Option<String>,
    #[serde(rename = "hostStartedAtMs")]
    pub host_started_at_ms: Option<u64>,
    /// Host service is bound and core APIs are reachable.
    pub ready: bool,
    #[serde(rename = "hostReady")]
    pub host_ready: bool,
    #[serde(rename = "accessReady")]
    pub access_ready: bool,
    #[serde(rename = "fullWarmupReady")]
    pub full_warmup_ready: bool,
    #[serde(rename = "deferredWarmupPending")]
    pub deferred_warmup_pending: bool,
    pub phase: String,
    #[serde(rename = "activeJob")]
    pub active_job: Option<String>,
    #[serde(rename = "activeJobElapsedMs")]
    pub active_job_elapsed_ms: Option<u64>,
    #[serde(rename = "lastBuildTotalMs")]
    pub last_build_total_ms: Option<u64>,
    #[serde(rename = "lastBuildCompileMs")]
    pub last_build_compile_ms: Option<u64>,
    #[serde(rename = "lastBuildWarmupMs")]
    pub last_build_warmup_ms: Option<u64>,
    #[serde(rename = "lastCriticalWarmupMs")]
    pub last_critical_warmup_ms: Option<u64>,
    #[serde(rename = "lastDeferredWarmupMs")]
    pub last_deferred_warmup_ms: Option<u64>,
    #[serde(rename = "lastCriticalWarmupRequestCount")]
    pub last_critical_warmup_request_count: usize,
    #[serde(rename = "lastDeferredWarmupRequestCount")]
    pub last_deferred_warmup_request_count: usize,
    #[serde(rename = "lastWarningCount")]
    pub last_warning_count: usize,
    #[serde(rename = "lastBuildDiagnostics")]
    pub last_build_diagnostics: Option<PrebuildDiagnosticsReport>,
    #[serde(rename = "correctnessFailed")]
    pub correctness_failed: bool,
    #[serde(rename = "warningCategories")]
    pub warning_categories: Vec<String>,
    #[serde(rename = "warningCategoryCounts")]
    pub warning_category_counts: BTreeMap<String, usize>,
    #[serde(rename = "failingDatasets")]
    pub failing_datasets: Vec<String>,
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
    #[serde(default, rename = "sceneId")]
    pub scene_id: Option<String>,
    #[serde(default, rename = "targetFile")]
    pub target_file: Option<String>,
    #[serde(default, rename = "hotOnly")]
    pub hot_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScopedFeedbackStatus {
    Ready,
    ArtifactMissing,
    DiagnosticError,
}

impl ScopedFeedbackStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::ArtifactMissing => "artifact_missing",
            Self::DiagnosticError => "diagnostic_error",
        }
    }

    pub(crate) fn artifact_ready(self) -> bool {
        !matches!(self, Self::ArtifactMissing)
    }
}

#[derive(Clone)]
pub(crate) struct ScopedCompileFeedback {
    pub status: ScopedFeedbackStatus,
    pub outcome: Option<CompileWithCacheOutcome>,
    pub diagnostic_error_count: usize,
    pub warning_count: usize,
    pub diagnostic_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostBuildJobResponse {
    pub accepted: bool,
    pub phase: String,
    #[serde(rename = "activeJob")]
    pub active_job: Option<String>,
    #[serde(rename = "appId")]
    pub app_id: Option<String>,
    pub mode: String,
    #[serde(rename = "scopeProfile")]
    pub scope_profile: String,
    pub status: String,
    #[serde(rename = "artifactReady")]
    pub artifact_ready: bool,
    #[serde(rename = "diagnosticErrorCount")]
    pub diagnostic_error_count: usize,
    #[serde(rename = "warningCount")]
    pub warning_count: usize,
    #[serde(rename = "diagnosticSummary")]
    pub diagnostic_summary: Option<String>,
    #[serde(rename = "scopedBuild")]
    pub scoped_build: bool,
    #[serde(rename = "sceneId")]
    pub scene_id: Option<String>,
    #[serde(rename = "targetFile")]
    pub target_file: Option<String>,
    #[serde(rename = "compileRevision")]
    pub compile_revision: Option<String>,
    #[serde(rename = "compileMs")]
    pub compile_ms: Option<u64>,
    #[serde(rename = "cacheHit")]
    pub cache_hit: Option<bool>,
    #[serde(rename = "artifactCacheHit")]
    pub artifact_cache_hit: Option<bool>,
    #[serde(rename = "scopeArtifactsMs", skip_serializing_if = "Option::is_none")]
    pub scope_artifacts_ms: Option<u64>,
    #[serde(rename = "mrgSlotsReady", skip_serializing_if = "Option::is_none")]
    pub mrg_slots_ready: Option<usize>,
    #[serde(rename = "evalArtifactsWarmed", skip_serializing_if = "Option::is_none")]
    pub eval_artifacts_warmed: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HostReadinessRegistry {
    host_bound: bool,
    host_started_at_ms: Option<u64>,
    access_ready: bool,
    full_warmup_ready: bool,
    deferred_warmup_pending: bool,
    run_id: Option<String>,
    startup_policy: Option<String>,
    startup_artifact_dir: Option<String>,
    phase: String,
    manifest_path: String,
    manifest_source: String,
    warmed_apps: Vec<String>,
    failed_apps: Vec<String>,
    building_apps: Vec<String>,
    error_summary: Vec<String>,
    active_job: Option<String>,
    active_job_started_at: Option<Instant>,
    last_build_total_ms: Option<u64>,
    last_build_compile_ms: Option<u64>,
    last_build_warmup_ms: Option<u64>,
    last_critical_warmup_ms: Option<u64>,
    last_deferred_warmup_ms: Option<u64>,
    last_critical_warmup_request_count: usize,
    last_deferred_warmup_request_count: usize,
    last_warning_count: usize,
    last_build_diagnostics: Option<PrebuildDiagnosticsReport>,
    correctness_failed: bool,
    warning_categories: Vec<String>,
    warning_category_counts: BTreeMap<String, usize>,
    failing_datasets: Vec<String>,
    apps: BTreeMap<String, HostAppReadinessState>,
}

#[derive(Debug, Clone, Default)]
struct HostAppReadinessState {
    phase: String,
    last_error: Option<String>,
    warnings: Vec<String>,
    warning_details: Vec<PrebuildWarningReport>,
    warning_categories: Vec<String>,
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
    host_readiness_registry()
        .lock()
        .ok()
        .map(|mut guard| f(&mut guard))
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

fn host_started_at_ms_from_registry(snapshot: &HostReadinessRegistry) -> Option<u64> {
    snapshot
        .host_started_at_ms
        .or_else(startup_run::current_started_at_ms)
}

fn format_elapsed_zh(elapsed_ms: u64) -> String {
    if elapsed_ms < 1000 {
        return format!("{} 秒", elapsed_ms.max(1));
    }
    if elapsed_ms < 60_000 {
        let seconds = (elapsed_ms as f64 / 1000.0).round() as u64;
        return format!("{} 秒", seconds.max(1));
    }
    if elapsed_ms < 3_600_000 {
        let minutes = elapsed_ms / 60_000;
        let seconds = (elapsed_ms % 60_000) / 1000;
        if seconds == 0 {
            return format!("{} 分", minutes);
        }
        return format!("{} 分 {} 秒", minutes, seconds);
    }
    let hours = elapsed_ms / 3_600_000;
    let minutes = (elapsed_ms % 3_600_000) / 60_000;
    if minutes == 0 {
        return format!("{} 小时", hours);
    }
    format!("{} 小时 {} 分", hours, minutes)
}

pub(crate) fn host_warmup_in_progress() -> bool {
    let snapshot = host_readiness_registry()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    if !snapshot.host_bound {
        return true;
    }
    if !snapshot.access_ready {
        return true;
    }
    if snapshot.deferred_warmup_pending {
        return true;
    }
    matches!(
        snapshot.phase.as_str(),
        "starting" | "bound" | "building" | "verifying"
    )
}

pub(crate) fn warmup_pending_user_message() -> String {
    let snapshot = host_readiness_registry()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let started_at_ms = host_started_at_ms_from_registry(&snapshot);
    let elapsed_ms = started_at_ms.map(|started| {
        startup_run::now_ms_for_host_message()
            .saturating_sub(started)
    });
    let ago = elapsed_ms
        .map(format_elapsed_zh)
        .unwrap_or_else(|| "刚刚".to_string());
    let detail = if snapshot.deferred_warmup_pending {
        "后台仍在装载 deferred 指标"
    } else if matches!(
        snapshot.phase.as_str(),
        "building" | "verifying" | "bound"
    ) {
        "后台正在编译与预热"
    } else if !snapshot.access_ready {
        "启动预热尚未完成"
    } else {
        "访问态产物仍在装载"
    };
    format!(
        "系统于 {ago} 前刚刚启动，{detail}，该指标尚未装载，请稍候刷新页面。"
    )
}

pub(crate) fn is_warmup_transient_runtime_error(message: &str) -> bool {
    let text = message.trim();
    text.contains("not found in active scene resources")
        || text.contains("missing strict AOT metric result artifact")
        || text.contains("requires prebuilt access artifacts on access-only host")
        || text.contains("该指标尚未装载")
}

fn phase_access_ready(phase: &str) -> bool {
    matches!(phase, "ready" | "skipped")
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
        ready: phase_access_ready(state.phase.as_str()),
        phase: if state.phase.trim().is_empty() {
            "pending".to_string()
        } else {
            state.phase
        },
        last_error: state.last_error,
        warnings: state.warnings,
        warning_details: state.warning_details,
        warning_categories: state.warning_categories,
        compile_scope_count: scopes.len(),
        ready_scope_count,
        failed_scope_count,
        scopes,
    }
}

fn registry_snapshot_with_scope_gate(
    source_root: Option<&Path>,
) -> HostReadyResponse {
    let mut response = registry_snapshot();
    if let Some(root) = source_root {
        let reachability = crate::readiness::reachability::check_reachability(root, None);
        response.access_ready = reachability.access_ready;
        response.scope_gate = Some(reachability.scope_gate);
    }
    response
}

fn registry_snapshot() -> HostReadyResponse {
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
    let ready_app_count = apps
        .iter()
        .filter(|app| phase_access_ready(app.phase.as_str()))
        .count();
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
        access_ready: snapshot.access_ready,
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
            host_started_at_ms: startup_run::current_started_at_ms(),
            access_ready: false,
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
        .filter(|app| phase_access_ready(app.phase.as_str()))
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
        .filter_map(|(app_id, app)| {
            phase_access_ready(app.phase.as_str()).then_some(app_id.clone())
        })
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

fn apply_success_app_report(app_report: &PrebuildAppReport, app_state: &mut HostAppReadinessState) {
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

fn status_from_report(
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

fn refresh_metric_response_indices_after_prebuild(
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

fn mark_job_failed(
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

fn begin_job(mode: PrebuildMode, app_filter: Option<&str>, origin: &str) -> Result<String> {
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

fn run_prebuild_job_sync_inner(
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
        },
    )
}

fn startup_deferred_warmup_pending(source_root: &Path) -> bool {
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

fn spawn_manual_job(
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
    let canonical = format!("scenes/{normalized_scene}.mei");
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
    if let Some(hit) = candidates.iter().find(|target| target.as_str() == canonical) {
        return Some(hit.clone());
    }
    candidates.into_iter().min_by_key(|target| {
        let cross_capsule_penalty = usize::from(
            target.starts_with("scenes/")
                && target
                    .strip_prefix("scenes/")
                    .and_then(|rest| rest.chars().next())
                    .is_some_and(|ch| ch.is_ascii_digit()),
        );
        (cross_capsule_penalty, target.len())
    })
}

fn normalized_optional_scope(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn compile_feedback_from_compiled(compiled: &CompiledApp) -> (usize, usize, Option<String>) {
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

fn scoped_response_status(status: ScopedFeedbackStatus) -> StatusCode {
    match status {
        ScopedFeedbackStatus::Ready => StatusCode::OK,
        ScopedFeedbackStatus::ArtifactMissing => StatusCode::NOT_FOUND,
        ScopedFeedbackStatus::DiagnosticError => StatusCode::CONFLICT,
    }
}

fn host_build_response_from_scoped_feedback(
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
    }
}

fn run_scoped_build(
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
    );
    Json(report)
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
            run_id: ready.run_id,
            startup_policy: ready.startup_policy,
            build_descriptor: ready.build_descriptor,
            startup_artifact_dir: ready.startup_artifact_dir,
            host_started_at_ms: ready.host_started_at_ms,
            ready: ready.ready,
            host_ready: ready.host_ready,
            access_ready: ready.access_ready,
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
            access_ready: true,
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
