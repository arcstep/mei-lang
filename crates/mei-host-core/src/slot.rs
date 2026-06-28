/// Descriptor returned by plug-ds; shell writes MRG slots from these.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CacheLayersReady {
    #[serde(default)]
    pub disk: bool,
    #[serde(default)]
    pub memory: bool,
    #[serde(default)]
    pub client: bool,
}

impl Default for CacheLayersReady {
    fn default() -> Self {
        Self {
            disk: true,
            memory: false,
            client: false,
        }
    }
}

/// Descriptor returned by plug-ds; shell writes MRG slots from these.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EvalSlotDescriptor {
    pub slot_key: String,
    pub scope_key: String,
    pub owner_resource_id: String,
    pub metric_def_bundle_revision: String,
    pub data_source_revision: String,
    pub payload_kind: String,
    pub content_hash: String,
    pub schema_version: String,
    pub wall_ms: u64,
    pub artifact_hit: bool,
    #[serde(default)]
    pub workset_id: String,
    #[serde(default)]
    pub cache_layer: String,
    #[serde(default)]
    pub cache_layers_ready: CacheLayersReady,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_revision: Option<String>,
    #[serde(default)]
    pub resident_tier: String,
    #[serde(default)]
    pub client_eligible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_bytes: Option<u64>,
}
