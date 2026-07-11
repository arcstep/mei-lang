use super::prelude::*;
use super::*;

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
    #[serde(rename = "accessReady")]
    pub access_ready: bool,
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
    #[serde(skip_serializing_if = "Option::is_none", rename = "gateSummary")]
    pub gate_summary: Option<ScopeGateSweepSummary>,
    pub scopes: Vec<HostScopeReadinessResponse>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct ScopeGateSweepSummary {
    #[serde(rename = "l2Miss")]
    pub l2_miss: usize,
    #[serde(rename = "l3Fail")]
    pub l3_fail: usize,
    #[serde(rename = "l4Stale")]
    pub l4_stale: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degraded_scopes: Vec<String>,
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
    #[serde(rename = "artifactsReady")]
    pub artifacts_ready: bool,
    #[serde(rename = "scopeGateReady")]
    pub scope_gate_ready: bool,
    #[serde(rename = "accessReady")]
    pub access_ready: bool,
    #[serde(rename = "defaultAppId", skip_serializing_if = "Option::is_none")]
    pub default_app_id: Option<String>,
    #[serde(rename = "defaultAppAccessReady")]
    pub default_app_access_ready: bool,
    #[serde(rename = "anyAppAccessReady")]
    pub any_app_access_ready: bool,
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
    #[serde(skip_serializing_if = "Option::is_none", rename = "gateSummary")]
    pub gate_summary: Option<ScopeGateSweepSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostHeartbeatAppSummary {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub phase: String,
    #[serde(rename = "accessReady")]
    pub access_ready: bool,
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
    #[serde(rename = "defaultAppId", skip_serializing_if = "Option::is_none")]
    pub default_app_id: Option<String>,
    #[serde(rename = "defaultAppAccessReady")]
    pub default_app_access_ready: bool,
    #[serde(rename = "anyAppAccessReady")]
    pub any_app_access_ready: bool,
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
    pub apps: Vec<HostHeartbeatAppSummary>,
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
    #[serde(
        rename = "evalArtifactsWarmed",
        skip_serializing_if = "Option::is_none"
    )]
    pub eval_artifacts_warmed: Option<usize>,
    #[serde(rename = "blockEvalHint", skip_serializing_if = "Option::is_none")]
    pub block_eval_hint: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HostReadinessRegistry {
    pub(crate) host_bound: bool,
    pub(crate) host_started_at_ms: Option<u64>,
    pub(crate) artifacts_ready: bool,
    pub(crate) scope_gate_ready: bool,
    pub(crate) gate_summary: Option<ScopeGateSweepSummary>,
    pub(crate) access_ready: bool,
    pub(crate) default_app_id: Option<String>,
    pub(crate) default_app_access_ready: bool,
    pub(crate) any_app_access_ready: bool,
    pub(crate) full_warmup_ready: bool,
    pub(crate) deferred_warmup_pending: bool,
    pub(crate) run_id: Option<String>,
    pub(crate) startup_policy: Option<String>,
    pub(crate) startup_artifact_dir: Option<String>,
    pub(crate) phase: String,
    pub(crate) manifest_path: String,
    pub(crate) manifest_source: String,
    pub(crate) warmed_apps: Vec<String>,
    pub(crate) failed_apps: Vec<String>,
    pub(crate) building_apps: Vec<String>,
    pub(crate) error_summary: Vec<String>,
    pub(crate) active_job: Option<String>,
    pub(crate) active_job_started_at: Option<Instant>,
    pub(crate) last_build_total_ms: Option<u64>,
    pub(crate) last_build_compile_ms: Option<u64>,
    pub(crate) last_build_warmup_ms: Option<u64>,
    pub(crate) last_critical_warmup_ms: Option<u64>,
    pub(crate) last_deferred_warmup_ms: Option<u64>,
    pub(crate) last_critical_warmup_request_count: usize,
    pub(crate) last_deferred_warmup_request_count: usize,
    pub(crate) last_warning_count: usize,
    pub(crate) last_build_diagnostics: Option<PrebuildDiagnosticsReport>,
    pub(crate) correctness_failed: bool,
    pub(crate) warning_categories: Vec<String>,
    pub(crate) warning_category_counts: BTreeMap<String, usize>,
    pub(crate) failing_datasets: Vec<String>,
    pub(crate) apps: BTreeMap<String, HostAppReadinessState>,
}
