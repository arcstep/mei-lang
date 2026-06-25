use serde::{Deserialize, Serialize};

pub const LAST_BUILD_SUMMARY_REL: &str = ".mei/prebuild/last-build-summary.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterializationDiagnosticsReport {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub sections: Vec<String>,
    pub disk: DiskDiagnosticsSection,
    pub eval: EvalDiagnosticsSection,
    pub mcg: McgDiagnosticsSection,
    pub mrg: MrgDiagnosticsSection,
    pub cache: CacheDiagnosticsSection,
    pub build: BuildDiagnosticsSection,
    pub alerts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reachability: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskDiagnosticsSection {
    #[serde(rename = "compiledAppFileCount")]
    pub compiled_app_file_count: usize,
    #[serde(rename = "compiledAppBytes")]
    pub compiled_app_bytes: u64,
    #[serde(rename = "scenePayloadFileCount")]
    pub scene_payload_file_count: usize,
    #[serde(rename = "scenePayloadBytes")]
    pub scene_payload_bytes: u64,
    #[serde(rename = "evalArtifactFileCount")]
    pub eval_artifact_file_count: usize,
    #[serde(rename = "evalArtifactBytes")]
    pub eval_artifact_bytes: u64,
    #[serde(rename = "graphBytes")]
    pub graph_bytes: u64,
    #[serde(rename = "dataSnapshotsBytes")]
    pub data_snapshots_bytes: u64,
    #[serde(rename = "prebuildBytes")]
    pub prebuild_bytes: u64,
    #[serde(rename = "appRootBytes")]
    pub app_root_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalDiagnosticsSection {
    #[serde(rename = "metricResponseFiles")]
    pub metric_response_files: usize,
    #[serde(rename = "metricResponseBytes")]
    pub metric_response_bytes: u64,
    #[serde(rename = "metricDataframeFiles")]
    pub metric_dataframe_files: usize,
    #[serde(rename = "metricDataframeBytes")]
    pub metric_dataframe_bytes: u64,
    #[serde(rename = "evalTotalFiles")]
    pub eval_total_files: usize,
    #[serde(rename = "evalTotalBytes")]
    pub eval_total_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McgDiagnosticsSection {
    #[serde(rename = "nodeCount")]
    pub node_count: usize,
    #[serde(rename = "scenePayloadNodes")]
    pub scene_payload_nodes: usize,
    #[serde(rename = "metricDefBundleNodes")]
    pub metric_def_bundle_nodes: usize,
    #[serde(rename = "appSkeletonPresent")]
    pub app_skeleton_present: bool,
    #[serde(rename = "registryRevision")]
    pub registry_revision: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MrgDiagnosticsSection {
    #[serde(rename = "slotCount")]
    pub slot_count: usize,
    #[serde(rename = "readySlots")]
    pub ready_slots: usize,
    #[serde(rename = "staleSlots")]
    pub stale_slots: usize,
    #[serde(rename = "failedSlots")]
    pub failed_slots: usize,
    #[serde(rename = "staleRatio")]
    pub stale_ratio: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheDiagnosticsSection {
    #[serde(rename = "accessSlimArtifacts")]
    pub access_slim_artifacts: bool,
    #[serde(rename = "canonicalArtifactPersist")]
    pub canonical_artifact_persist: bool,
    #[serde(rename = "graphRegistryDedup")]
    pub graph_registry_dedup: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildDiagnosticsSection {
    pub source: String,
    #[serde(rename = "reportPath", skip_serializing_if = "Option::is_none")]
    pub report_path: Option<String>,
    #[serde(rename = "recordedAtMs", skip_serializing_if = "Option::is_none")]
    pub recorded_at_ms: Option<u64>,
    #[serde(rename = "peakRssBytes", skip_serializing_if = "Option::is_none")]
    pub peak_rss_bytes: Option<u64>,
    #[serde(rename = "currentRssBytes", skip_serializing_if = "Option::is_none")]
    pub current_rss_bytes: Option<u64>,
    #[serde(rename = "compileIndexHits", skip_serializing_if = "Option::is_none")]
    pub compile_index_hits: Option<usize>,
    #[serde(rename = "compileIndexMisses", skip_serializing_if = "Option::is_none")]
    pub compile_index_misses: Option<usize>,
    #[serde(rename = "compileIndexStaleEntries", skip_serializing_if = "Option::is_none")]
    pub compile_index_stale_entries: Option<usize>,
    #[serde(rename = "mrgEvalSkips", skip_serializing_if = "Option::is_none")]
    pub mrg_eval_skips: Option<usize>,
    #[serde(rename = "dataframeEvalSkips", skip_serializing_if = "Option::is_none")]
    pub dataframe_eval_skips: Option<usize>,
    #[serde(rename = "compileIndexEntries", skip_serializing_if = "Option::is_none")]
    pub compile_index_entries: Option<usize>,
    #[serde(rename = "compileIndexGeneratedAtMs", skip_serializing_if = "Option::is_none")]
    pub compile_index_generated_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastBuildSummary {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(rename = "recordedAtMs")]
    pub recorded_at_ms: u64,
    #[serde(rename = "peakRssBytes")]
    pub peak_rss_bytes: u64,
    #[serde(rename = "currentRssBytes", skip_serializing_if = "Option::is_none")]
    pub current_rss_bytes: Option<u64>,
    #[serde(rename = "compileIndexHits")]
    pub compile_index_hits: usize,
    #[serde(rename = "compileIndexMisses")]
    pub compile_index_misses: usize,
    #[serde(rename = "compileIndexStaleEntries")]
    pub compile_index_stale_entries: usize,
    #[serde(rename = "mrgEvalSkips")]
    pub mrg_eval_skips: usize,
    #[serde(rename = "dataframeEvalSkips")]
    pub dataframe_eval_skips: usize,
}

impl LastBuildSummary {
    pub const SCHEMA: &'static str = "mei-last-build-summary-v1";

    pub fn from_prebuild_diagnostics(
        app_id: &str,
        diagnostics: &crate::prebuild::PrebuildDiagnosticsReport,
    ) -> Self {
        Self {
            schema_version: Self::SCHEMA.to_string(),
            app_id: app_id.to_string(),
            recorded_at_ms: crate::http::startup_run::now_ms_for_host_message() as u64,
            peak_rss_bytes: diagnostics.peak_rss_bytes,
            current_rss_bytes: diagnostics.current_rss_bytes,
            compile_index_hits: diagnostics.compile_index.hits,
            compile_index_misses: diagnostics.compile_index.misses,
            compile_index_stale_entries: diagnostics.compile_index.stale_entries,
            mrg_eval_skips: diagnostics.compile_index.mrg_eval_skips,
            dataframe_eval_skips: diagnostics.compile_index.dataframe_eval_skips,
        }
    }
}
