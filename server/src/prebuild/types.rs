use super::prelude::*;
pub(crate) fn is_script_target(path: &str) -> bool {
    path.ends_with(".mei")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrebuildMode {
    Build,
    Verify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrebuildScopeProfile {
    Full,
    HotOnly,
}

#[derive(Debug, Clone)]
pub struct PrebuildOptions {
    pub app_filter: Option<String>,
    pub mode: PrebuildMode,
    pub clean: bool,
    pub force_rebuild: bool,
    pub scope_profile: PrebuildScopeProfile,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildScopeReport {
    pub requested_scene_id: Option<String>,
    pub requested_target_file: Option<String>,
    pub active_scene_id: Option<String>,
    pub active_target_file: String,
    pub cache_hit: bool,
    pub artifact_cache_hit: bool,
    pub compile_revision: String,
    pub cache_lookup_ms: u64,
    pub artifact_load_ms: u64,
    pub compile_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildTimingReport {
    pub total_wall_ms: u64,
    pub compile_scopes_ms: u64,
    pub data_snapshots_ms: u64,
    pub scope_artifacts_ms: u64,
    pub warmup_requests_ms: u64,
    pub critical_warmup_requests_ms: u64,
    pub deferred_warmup_requests_ms: u64,
    pub critical_warmup_request_count: usize,
    pub deferred_warmup_request_count: usize,
    pub max_parallelism: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildCoverageReport {
    pub compile_artifacts_planned: usize,
    pub compile_artifacts_ready: usize,
    pub compile_artifacts_missing: usize,
    pub dataset_import_artifacts_planned: usize,
    pub dataset_import_artifacts_ready: usize,
    pub dataset_import_artifacts_missing: usize,
    pub metric_response_artifacts_planned: usize,
    pub metric_response_artifacts_ready: usize,
    pub metric_response_artifacts_built: usize,
    #[serde(default)]
    pub metric_response_artifacts_skipped_bundle_unchanged: usize,
    pub metric_response_artifacts_missing: usize,
    pub metric_dataframe_artifacts_planned: usize,
    pub metric_dataframe_artifacts_ready: usize,
    pub metric_dataframe_artifacts_built: usize,
    #[serde(default)]
    pub metric_dataframe_artifacts_skipped_bundle_unchanged: usize,
    pub metric_dataframe_artifacts_missing: usize,
    pub total_missing_artifacts: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildDiskUsageReport {
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildEvalArtifactDiskReport {
    pub total: PrebuildDiskUsageReport,
    pub metric_response: PrebuildDiskUsageReport,
    pub metric_dataframe: PrebuildDiskUsageReport,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildCompileIndexStatsReport {
    pub preload_reuse_hits: usize,
    pub postload_identity_collapses: usize,
    pub hits: usize,
    pub misses: usize,
    pub stale_entries: usize,
    pub fallback_loads: usize,
    #[serde(default)]
    pub manifest_probes: usize,
    #[serde(default)]
    pub manifest_stale_skips: usize,
    #[serde(default)]
    pub artifact_loads_avoided: usize,
    #[serde(default)]
    pub mrg_eval_skips: usize,
    #[serde(default)]
    pub dataframe_eval_skips: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildSessionEntryStatsReport {
    pub scope_entries: usize,
    pub cache_entries: usize,
    pub identity_entries: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildNodeBudgetReport {
    pub canonical_node_limit: usize,
    pub startup_wall_ms_limit: u64,
    pub over_canonical_node_limit: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildPlanNodeStatsReport {
    pub manifest_compile_scope_nodes: usize,
    pub hot_compile_scope_nodes: usize,
    pub deferred_compile_scope_nodes: usize,
    pub planned_warmup_request_nodes: usize,
    pub planned_warmup_scope_nodes: usize,
    pub planned_metric_workset_nodes: usize,
    pub planned_response_artifact_nodes: usize,
    pub planned_dataframe_artifact_nodes: usize,
    pub planned_total_nodes: usize,
    pub canonical_prebuild_nodes: usize,
    pub budget: PrebuildNodeBudgetReport,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildSlowScopeDiagnostic {
    pub scene_id: Option<String>,
    pub target_file: String,
    pub compile_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildSlowMetricDiagnostic {
    pub kind: String,
    pub dataset: String,
    pub metric: String,
    pub scene: String,
    pub ms: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildWarmupDiagnosticReport {
    pub total_request_count: usize,
    pub executed_request_count: usize,
    pub cache_hit_count: usize,
    pub total_ms: u64,
    pub ok: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildDiagnosticsReport {
    pub total_scope_checks: usize,
    pub real_compile_count: usize,
    pub cache_hit_count: usize,
    pub unique_compile_result_count: usize,
    pub canonical_identity_count: usize,
    pub redundant_scope_checks: usize,
    pub expansion_ratio: f64,
    pub cache_probe_ms: u64,
    pub compile_miss_ms: u64,
    pub current_rss_bytes: Option<u64>,
    pub peak_rss_bytes: u64,
    pub eval_artifacts_disk: PrebuildEvalArtifactDiskReport,
    pub compile_index: PrebuildCompileIndexStatsReport,
    pub session_before_clear: PrebuildSessionEntryStatsReport,
    pub session_after_clear: PrebuildSessionEntryStatsReport,
    pub warmup_reuse_hits: usize,
    pub plan_nodes: PrebuildPlanNodeStatsReport,
    pub critical_warmup: PrebuildWarmupDiagnosticReport,
    pub deferred_warmup: PrebuildWarmupDiagnosticReport,
    pub slow_scopes: Vec<PrebuildSlowScopeDiagnostic>,
    pub slow_metrics: Vec<PrebuildSlowMetricDiagnostic>,
    #[serde(default)]
    pub fingerprint_skip: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildWarningReport {
    pub phase: String,
    pub category: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_selector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
    pub error: String,
}

impl PrebuildWarningReport {
    pub fn display_message(&self) -> &str {
        self.message.as_str()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildAppReport {
    pub app_id: String,
    pub compile_scopes: Vec<PrebuildScopeReport>,
    pub coverage: PrebuildCoverageReport,
    pub timings: PrebuildTimingReport,
    pub data_snapshots: Option<PublishDataSnapshotsReport>,
    pub diagnostics: PrebuildDiagnosticsReport,
    pub warnings: Vec<PrebuildWarningReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildReport {
    pub schema_version: String,
    pub mode: PrebuildMode,
    pub scope_profile: PrebuildScopeProfile,
    pub clean: bool,
    pub clean_wall_ms: u64,
    pub total_wall_ms: u64,
    pub source_root: String,
    pub manifest_path: String,
    pub manifest_source: String,
    pub ok: bool,
    pub succeeded_apps: Vec<String>,
    pub failed_apps: Vec<String>,
    pub error_summary: Vec<String>,
    pub diagnostics: PrebuildDiagnosticsReport,
    pub apps: Vec<PrebuildAppReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildScopeSummary {
    pub requested_scene_id: Option<String>,
    pub requested_target_file: Option<String>,
    pub active_scene_id: Option<String>,
    pub active_target_file: String,
    pub cache_hit: bool,
    pub artifact_cache_hit: bool,
    pub cache_lookup_ms: u64,
    pub artifact_load_ms: u64,
    pub compile_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildAppSummary {
    pub app_id: String,
    pub compile_scopes: Vec<PrebuildScopeSummary>,
    pub coverage: PrebuildCoverageReport,
    pub timings: PrebuildTimingReport,
    pub diagnostics: PrebuildDiagnosticsReport,
    pub warnings: Vec<PrebuildWarningReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildReportSummary {
    pub schema_version: String,
    pub mode: PrebuildMode,
    pub scope_profile: PrebuildScopeProfile,
    pub clean: bool,
    pub clean_wall_ms: u64,
    pub total_wall_ms: u64,
    pub source_root: String,
    pub manifest_path: String,
    pub manifest_source: String,
    pub ok: bool,
    pub succeeded_apps: Vec<String>,
    pub failed_apps: Vec<String>,
    pub error_summary: Vec<String>,
    pub diagnostics: PrebuildDiagnosticsReport,
    pub apps: Vec<PrebuildAppSummary>,
}

impl PrebuildReport {
    pub fn warning_categories(&self) -> Vec<String> {
        let mut categories = self
            .apps
            .iter()
            .flat_map(|app| app.warnings.iter().map(|warning| warning.category.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        categories.sort();
        categories
    }

    pub fn warning_category_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::<String, usize>::new();
        for warning in self.apps.iter().flat_map(|app| app.warnings.iter()) {
            *counts.entry(warning.category.clone()).or_insert(0) += 1;
        }
        counts
    }

    pub fn failing_datasets(&self) -> Vec<String> {
        let mut datasets = self
            .apps
            .iter()
            .flat_map(|app| {
                app.warnings
                    .iter()
                    .filter_map(|warning| warning.dataset_selector.clone())
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        datasets.sort();
        datasets
    }

    pub fn correctness_failed(&self) -> bool {
        !self.ok
            || self
                .apps
                .iter()
                .any(|app| !app.warnings.is_empty())
            || !self.failed_apps.is_empty()
    }

    pub fn summary(&self) -> PrebuildReportSummary {
        PrebuildReportSummary {
            schema_version: self.schema_version.clone(),
            mode: self.mode,
            scope_profile: self.scope_profile,
            clean: self.clean,
            clean_wall_ms: self.clean_wall_ms,
            total_wall_ms: self.total_wall_ms,
            source_root: self.source_root.clone(),
            manifest_path: self.manifest_path.clone(),
            manifest_source: self.manifest_source.clone(),
            ok: self.ok,
            succeeded_apps: self.succeeded_apps.clone(),
            failed_apps: self.failed_apps.clone(),
            error_summary: self.error_summary.clone(),
            diagnostics: self.diagnostics.clone(),
            apps: self
                .apps
                .iter()
                .map(|app| PrebuildAppSummary {
                    app_id: app.app_id.clone(),
                    compile_scopes: app
                        .compile_scopes
                        .iter()
                        .map(|scope| PrebuildScopeSummary {
                            requested_scene_id: scope.requested_scene_id.clone(),
                            requested_target_file: scope.requested_target_file.clone(),
                            active_scene_id: scope.active_scene_id.clone(),
                            active_target_file: scope.active_target_file.clone(),
                            cache_hit: scope.cache_hit,
                            artifact_cache_hit: scope.artifact_cache_hit,
                            cache_lookup_ms: scope.cache_lookup_ms,
                            artifact_load_ms: scope.artifact_load_ms,
                            compile_ms: scope.compile_ms,
                        })
                        .collect(),
                    coverage: app.coverage.clone(),
                    timings: app.timings.clone(),
                    diagnostics: app.diagnostics.clone(),
                    warnings: app.warnings.clone(),
                })
                .collect(),
        }
    }
}

