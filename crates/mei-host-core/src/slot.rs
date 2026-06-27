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
}
