use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterializationDiagnosticsReport {
    #[serde(rename = "appId")]
    pub app_id: String,
    pub sections: Vec<String>,
    pub disk: DiskDiagnosticsSection,
    pub mcg: McgDiagnosticsSection,
    pub mrg: MrgDiagnosticsSection,
    pub cache: CacheDiagnosticsSection,
    pub alerts: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiskDiagnosticsSection {
    #[serde(rename = "compiledAppFileCount")]
    pub compiled_app_file_count: usize,
    #[serde(rename = "compiledAppBytes")]
    pub compiled_app_bytes: u64,
    #[serde(rename = "scenePayloadFileCount")]
    pub scene_payload_file_count: usize,
    #[serde(rename = "evalArtifactFileCount")]
    pub eval_artifact_file_count: usize,
    #[serde(rename = "appRootBytes")]
    pub app_root_bytes: u64,
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
