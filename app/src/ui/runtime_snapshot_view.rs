//! Deserialized runtime snapshot for SSR panels (shape mirrors `/api/runtime/snapshot`).

use mei_lang_kernel::ReachabilityTreeRoot;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeHostView {
    pub phase: String,
    pub app_phase: String,
    pub access_ready: bool,
    pub scope_gate_ready: bool,
    pub last_build_total_ms: Option<u64>,
    pub last_build_compile_ms: Option<u64>,
    pub last_build_warmup_ms: Option<u64>,
    pub gate_l2_miss: Option<usize>,
    pub gate_l3_fail: Option<usize>,
    pub gate_l4_stale: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimePrebuildView {
    pub ok: bool,
    pub scope_profile: Option<String>,
    pub total_wall_ms: Option<u64>,
    pub compile_scopes_ms: Option<u64>,
    pub scope_artifacts_ms: Option<u64>,
    pub peak_rss_bytes: Option<u64>,
    pub current_rss_bytes: Option<u64>,
    pub compile_scope_count: Option<usize>,
    pub real_compile_count: Option<usize>,
    pub cache_hit_count: Option<usize>,
    pub expansion_ratio: Option<f64>,
    pub report_age: Option<String>,
    pub in_succeeded_apps: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeDiskView {
    pub compiled_app_file_count: usize,
    pub compiled_app_bytes: u64,
    pub scene_payload_file_count: usize,
    pub scene_payload_bytes: u64,
    pub eval_artifact_file_count: usize,
    pub eval_artifact_bytes: u64,
    pub graph_bytes: u64,
    pub data_snapshots_bytes: u64,
    pub prebuild_bytes: u64,
    pub app_root_bytes: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeEvalView {
    pub metric_response_files: usize,
    pub metric_response_bytes: u64,
    pub metric_dataframe_files: usize,
    pub metric_dataframe_bytes: u64,
    pub eval_total_files: usize,
    pub eval_total_bytes: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeMcgView {
    pub node_count: usize,
    pub scene_payload_nodes: usize,
    pub metric_def_bundle_nodes: usize,
    pub app_skeleton_present: bool,
    pub registry_revision: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeMrgView {
    pub slot_count: usize,
    pub ready_slots: usize,
    pub stale_slots: usize,
    pub failed_slots: usize,
    pub stale_ratio: f64,
    pub navigation_node_count: Option<usize>,
    pub navigation_duplicate_keys: Option<usize>,
    pub navigation_orphan_urls: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeCacheView {
    pub access_slim_artifacts: bool,
    pub canonical_artifact_persist: bool,
    pub graph_registry_dedup: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeScopeGateView {
    pub l2_miss: usize,
    pub l3_fail: usize,
    pub l4_stale: usize,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeContentStoreView {
    pub bytes: u64,
    pub files_by_kind: std::collections::BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeBuildDiagView {
    pub source: String,
    pub compile_index_hits: Option<usize>,
    pub compile_index_misses: Option<usize>,
    pub compile_index_stale_entries: Option<usize>,
    pub compile_index_entries: Option<usize>,
    pub mrg_eval_skips: Option<usize>,
    pub dataframe_eval_skips: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeDiagnosticsView {
    pub disk: RuntimeDiskView,
    pub eval: RuntimeEvalView,
    pub mcg: RuntimeMcgView,
    pub mrg: RuntimeMrgView,
    pub cache: RuntimeCacheView,
    pub build: RuntimeBuildDiagView,
    pub content_store: RuntimeContentStoreView,
    pub scope_gate_sweep: RuntimeScopeGateView,
    pub alerts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RuntimeSnapshotView {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub roots: Vec<ReachabilityTreeRoot>,
    pub diagnostics: RuntimeDiagnosticsView,
    pub host: RuntimeHostView,
    pub prebuild: RuntimePrebuildView,
}

pub(crate) fn parse_runtime_snapshot(raw: &str) -> Option<RuntimeSnapshotView> {
    serde_json::from_str(raw).ok()
}
