use crate::io::{read_json_registry, write_json_registry};
use crate::paths::mrg_registry_path;
use crate::types::{current_time_ms, stable_hash, GraphNodeId, MaterialState, PayloadRef};

pub const MRG_REGISTRY_SCHEMA_VERSION: &str = "mei-mrg-registry-v2";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MrgSlotId {
    pub node: GraphNodeId,
    #[serde(rename = "scopeKey")]
    pub scope_key: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MrgLastEval {
    #[serde(rename = "atMs")]
    pub at_ms: u64,
    #[serde(rename = "wallMs")]
    pub wall_ms: u64,
    #[serde(rename = "artifactHit")]
    pub artifact_hit: bool,
    #[serde(rename = "cacheLayer")]
    pub cache_layer: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MrgSlotRecord {
    #[serde(rename = "slotId")]
    pub slot_id: MrgSlotId,
    #[serde(rename = "slotRevision")]
    pub slot_revision: String,
    pub state: MaterialState,
    #[serde(rename = "ownerResourceId")]
    pub owner_resource_id: String,
    #[serde(rename = "metricDefBundleRevision")]
    pub metric_def_bundle_revision: String,
    #[serde(rename = "dataSourceRevision")]
    pub data_source_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "payloadRef")]
    pub payload_ref: Option<PayloadRef>,
    #[serde(rename = "cachePolicy")]
    pub cache_policy: String,
    #[serde(rename = "evalEngine")]
    pub eval_engine: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "lastEval")]
    pub last_eval: Option<MrgLastEval>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MrgRegistry {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(rename = "registryRevision")]
    pub registry_revision: String,
    #[serde(rename = "updatedAtMs")]
    pub updated_at_ms: u64,
    pub slots: Vec<MrgSlotRecord>,
}

impl MrgRegistry {
    pub fn empty(app_id: &str) -> Self {
        Self {
            schema_version: MRG_REGISTRY_SCHEMA_VERSION.to_string(),
            app_id: app_id.to_string(),
            registry_revision: String::new(),
            updated_at_ms: 0,
            slots: Vec::new(),
        }
    }

    pub fn upsert_slot(&mut self, record: MrgSlotRecord) {
        if let Some(existing) = self
            .slots
            .iter_mut()
            .find(|slot| slot.slot_id == record.slot_id)
        {
            *existing = record;
        } else {
            self.slots.push(record);
        }
    }

    pub fn finalize(&mut self) {
        self.updated_at_ms = current_time_ms();
        let mut keys = self
            .slots
            .iter()
            .map(|slot| {
                format!(
                    "{}@{}={}",
                    slot.slot_id.node.stable_key(),
                    slot.slot_id.scope_key,
                    slot.slot_revision
                )
            })
            .collect::<Vec<_>>();
        keys.sort();
        self.registry_revision = stable_hash(&keys.join("\n"));
    }

    pub fn has_scope_slots(&self, scope_key: &str) -> bool {
        self.slots
            .iter()
            .any(|slot| slot.slot_id.scope_key == scope_key)
    }
}

pub struct MrgRegistryWriter;

impl MrgRegistryWriter {
    pub fn load(source_root: &std::path::Path, app_id: &str) -> MrgRegistry {
        read_json_registry::<MrgRegistry>(&mrg_registry_path(source_root, app_id))
            .ok()
            .flatten()
            .filter(|registry| registry.schema_version == MRG_REGISTRY_SCHEMA_VERSION)
            .unwrap_or_else(|| MrgRegistry::empty(app_id))
    }

    pub fn save(source_root: &std::path::Path, registry: &MrgRegistry) -> anyhow::Result<()> {
        write_json_registry(&mrg_registry_path(source_root, registry.app_id.as_str()), registry)
    }
}
