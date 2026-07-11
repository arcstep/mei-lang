use super::prelude::*;
pub(crate) fn is_script_target(path: &str) -> bool {
    path.ends_with(".mei")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrebuildMode {
    Build,
    Verify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrebuildScopeProfile {
    Full,
    HotOnly,
    BlockScoped,
}

#[derive(Debug, Clone)]
pub struct PrebuildOptions {
    pub app_filter: Option<String>,
    pub mode: PrebuildMode,
    pub clean: bool,
    pub force_rebuild: bool,
    pub scope_profile: PrebuildScopeProfile,
    pub dirty_only: bool,
    pub block_node: Option<String>,
    pub diagnose_on_fail: bool,
    pub continue_from: Option<String>,
}

pub fn effective_prebuild_scope_profile(options: &PrebuildOptions) -> PrebuildScopeProfile {
    if options.block_node.is_some() {
        PrebuildScopeProfile::BlockScoped
    } else {
        options.scope_profile
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrebuildScopeReport {
    pub requested_scene_id: Option<String>,
    pub requested_target_file: Option<String>,
    pub active_scene_id: Option<String>,
    pub active_target_file: String,
    pub cache_hit: bool,
    pub artifact_cache_hit: bool,
    #[serde(default)]
    pub assemble_only: bool,
    pub compile_revision: String,
    pub cache_lookup_ms: u64,
    pub artifact_load_ms: u64,
    pub compile_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrebuildDiskUsageReport {
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrebuildEvalArtifactDiskReport {
    pub total: PrebuildDiskUsageReport,
    pub metric_response: PrebuildDiskUsageReport,
    pub metric_dataframe: PrebuildDiskUsageReport,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    #[serde(default)]
    pub target_overlay_reuse_hits: usize,
    #[serde(default)]
    pub mcg_assemble_only_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrebuildSessionEntryStatsReport {
    pub scope_entries: usize,
    pub cache_entries: usize,
    pub identity_entries: usize,
    #[serde(default)]
    pub target_entries: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrebuildNodeBudgetReport {
    pub canonical_node_limit: usize,
    pub startup_wall_ms_limit: u64,
    pub over_canonical_node_limit: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrebuildSlowScopeDiagnostic {
    pub scene_id: Option<String>,
    pub target_file: String,
    pub compile_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrebuildSlowMetricDiagnostic {
    pub kind: String,
    pub dataset: String,
    pub metric: String,
    pub scene: String,
    pub ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrebuildWarmupDiagnosticReport {
    pub total_request_count: usize,
    pub executed_request_count: usize,
    pub cache_hit_count: usize,
    pub total_ms: u64,
    pub ok: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrebuildDiagnosticsReport {
    pub total_scope_checks: usize,
    pub real_compile_count: usize,
    #[serde(default)]
    pub assemble_only_count: usize,
    pub cache_hit_count: usize,
    pub unique_compile_result_count: usize,
    pub canonical_identity_count: usize,
    pub redundant_scope_checks: usize,
    pub expansion_ratio: f64,
    pub cache_probe_ms: u64,
    pub compile_miss_ms: u64,
    pub current_rss_bytes: Option<u64>,
    pub peak_rss_bytes: u64,
    #[serde(default)]
    pub orchestrator_peak_rss_bytes: u64,
    #[serde(default)]
    pub worker_peak_rss_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_binary_baseline_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rss_after_compile_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rss_after_artifacts_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rss_after_warmup_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_working_set_peak_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_disk_bytes: Option<u64>,
    #[serde(default)]
    pub session_peak_identity_entries: usize,
    #[serde(default)]
    pub hydrate_reuse_hits: u64,
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
    #[serde(skip_serializing_if = "Option::is_none", rename = "planSource")]
    pub plan_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "dirtySlotCount")]
    pub dirty_slot_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(skip_serializing_if = "Option::is_none", rename = "scopeKey")]
    pub scope_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "mrgSlotKey")]
    pub mrg_slot_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "errorChain")]
    pub error_chain: Option<String>,
}

impl PrebuildWarningReport {
    pub fn display_message(&self) -> &str {
        self.message.as_str()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrebuildWarningSample {
    pub category: String,
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "targetFile")]
    pub target_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "datasetSelector")]
    pub dataset_selector: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrebuildWarningSummary {
    pub total: usize,
    #[serde(rename = "byCategory")]
    pub by_category: BTreeMap<String, usize>,
    #[serde(rename = "byPhase")]
    pub by_phase: BTreeMap<String, usize>,
    #[serde(rename = "failingDatasets")]
    pub failing_datasets: Vec<String>,
    pub samples: Vec<PrebuildWarningSample>,
    #[serde(rename = "truncatedSampleCount")]
    pub truncated_sample_count: usize,
}

pub(crate) fn build_warning_summary(
    warnings: &[PrebuildWarningReport],
    sample_limit: usize,
    dataset_limit: usize,
) -> PrebuildWarningSummary {
    let mut by_category = BTreeMap::<String, usize>::new();
    let mut by_phase = BTreeMap::<String, usize>::new();
    for warning in warnings {
        *by_category.entry(warning.category.clone()).or_insert(0) += 1;
        *by_phase.entry(warning.phase.clone()).or_insert(0) += 1;
    }
    let mut failing_datasets = warnings
        .iter()
        .filter_map(|warning| warning.dataset_selector.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    failing_datasets.sort();
    if failing_datasets.len() > dataset_limit {
        failing_datasets.truncate(dataset_limit);
    }
    let samples = warnings
        .iter()
        .take(sample_limit)
        .map(|warning| PrebuildWarningSample {
            category: warning.category.clone(),
            phase: warning.phase.clone(),
            scene_id: warning.scene_id.clone(),
            target_file: warning.target_file.clone(),
            dataset_selector: warning.dataset_selector.clone(),
            message: warning.message.clone(),
        })
        .collect::<Vec<_>>();
    let truncated_sample_count = warnings.len().saturating_sub(samples.len());
    PrebuildWarningSummary {
        total: warnings.len(),
        by_category,
        by_phase,
        failing_datasets,
        samples,
        truncated_sample_count,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrebuildAppReport {
    pub app_id: String,
    pub compile_scopes: Vec<PrebuildScopeReport>,
    pub coverage: PrebuildCoverageReport,
    pub timings: PrebuildTimingReport,
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub data_snapshots: Option<PublishDataSnapshotsReport>,
    pub diagnostics: PrebuildDiagnosticsReport,
    pub warnings: Vec<PrebuildWarningReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrebuildAppSummary {
    pub app_id: String,
    pub compile_scopes: Vec<PrebuildScopeSummary>,
    pub coverage: PrebuildCoverageReport,
    pub timings: PrebuildTimingReport,
    pub diagnostics: PrebuildDiagnosticsReport,
    #[serde(rename = "warningCount")]
    pub warning_count: usize,
    #[serde(rename = "warningSummary")]
    pub warning_summary: PrebuildWarningSummary,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(rename = "warningCount")]
    pub warning_count: usize,
    #[serde(rename = "warningSummary")]
    pub warning_summary: PrebuildWarningSummary,
    #[serde(skip_serializing_if = "Option::is_none", rename = "fullReportPath")]
    pub full_report_path: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "failedBlockHints"
    )]
    pub failed_block_hints: Vec<String>,
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
        !self.ok || !self.failed_apps.is_empty()
    }

    pub fn aggregate_warning_summary(&self) -> PrebuildWarningSummary {
        let warnings = self
            .apps
            .iter()
            .flat_map(|app| app.warnings.iter())
            .cloned()
            .collect::<Vec<_>>();
        build_warning_summary(&warnings, 8, 12)
    }

    #[allow(dead_code)]
    pub fn summary(&self, full_report_path: Option<String>) -> PrebuildReportSummary {
        let warning_summary = self.aggregate_warning_summary();
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
            warning_count: warning_summary.total,
            warning_summary: warning_summary.clone(),
            full_report_path,
            failed_block_hints: Vec::new(),
            apps: self
                .apps
                .iter()
                .map(|app| {
                    let app_warning_summary = build_warning_summary(&app.warnings, 5, 8);
                    PrebuildAppSummary {
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
                        warning_count: app.warnings.len(),
                        warning_summary: app_warning_summary,
                    }
                })
                .collect(),
        }
    }
}
