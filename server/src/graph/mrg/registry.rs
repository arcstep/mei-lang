use serde::{Deserialize, Serialize};

use crate::graph::io::{read_json_registry, write_json_registry};
use crate::graph::paths::mrg_registry_path;
use crate::graph::types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef, stable_hash};

pub const MRG_REGISTRY_SCHEMA_VERSION: &str = "mei-mrg-registry-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrgSlotId {
    pub node: GraphNodeId,
    #[serde(rename = "scopeKey")]
    pub scope_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(rename = "dataSourceRevision", default)]
    pub data_source_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_ref: Option<PayloadRef>,
    #[serde(rename = "cachePolicy", default = "default_cache_policy")]
    pub cache_policy: String,
    #[serde(rename = "evalEngine", default = "default_eval_engine")]
    pub eval_engine: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "lastEval")]
    pub last_eval: Option<MrgLastEval>,
}

fn default_cache_policy() -> String {
    "artifact_sealed".to_string()
}

fn default_eval_engine() -> String {
    "json_walk".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrgEdgeRecord {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MrgRegistry {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(rename = "registryRevision")]
    pub registry_revision: String,
    #[serde(rename = "updatedAtMs")]
    pub updated_at_ms: u64,
    #[serde(default)]
    pub nodes: Vec<serde_json::Value>,
    pub slots: Vec<MrgSlotRecord>,
    #[serde(default)]
    pub edges: Vec<MrgEdgeRecord>,
}

impl MrgRegistry {
    pub fn empty(app_id: &str) -> Self {
        Self {
            schema_version: MRG_REGISTRY_SCHEMA_VERSION.to_string(),
            app_id: app_id.to_string(),
            registry_revision: String::new(),
            updated_at_ms: 0,
            nodes: Vec::new(),
            slots: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn upsert_slot(&mut self, record: MrgSlotRecord) {
        if let Some(existing) = self.slots.iter_mut().find(|slot| {
            slot.slot_id.node.key == record.slot_id.node.key
                && slot.slot_id.scope_key == record.slot_id.scope_key
        }) {
            *existing = record;
        } else {
            self.slots.push(record);
        }
    }

    pub fn mark_owner_slots_stale(&mut self, owner_id: &str, _bundle_revision: &str) {
        for slot in &mut self.slots {
            if slot.owner_resource_id == owner_id {
                slot.state = MaterialState::Stale;
            }
        }
    }

    pub fn dirty_slots(&self) -> Vec<&MrgSlotRecord> {
        self.slots
            .iter()
            .filter(|slot| {
                matches!(
                    slot.state,
                    MaterialState::Stale | MaterialState::Missing
                )
            })
            .collect()
    }

    pub fn navigation_entries(&self) -> Vec<crate::graph::mrg::navigation::types::NavigationEntry> {
        self.nodes
            .iter()
            .filter_map(crate::graph::mrg::navigation::types::parse_navigation_node)
            .collect()
    }

    pub fn navigation_by_key(&self, key: &str) -> Option<crate::graph::mrg::navigation::types::NavigationEntry> {
        self.navigation_entries()
            .into_iter()
            .find(|entry| entry.key == key)
    }

    pub fn upsert_navigation_node(
        &mut self,
        key: &str,
        url: &str,
        scene_id: &str,
        target_file: &str,
        state: MaterialState,
    ) {
        let node = serde_json::json!({
            "id": { "kind": GraphNodeKind::Navigation.slug(), "key": key },
            "url": url,
            "sceneId": scene_id,
            "targetFile": target_file,
            "state": match state {
                MaterialState::Ready => "ready",
                MaterialState::Stale => "stale",
                MaterialState::Warming => "warming",
                MaterialState::Failed => "failed",
                MaterialState::Missing => "missing",
            },
        });
        if let Some(existing) = self.nodes.iter_mut().find(|value| {
            value
                .get("id")
                .and_then(|id| id.get("key"))
                .and_then(|v| v.as_str())
                == Some(key)
        }) {
            *existing = node;
        } else {
            self.nodes.push(node);
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
        write_json_registry(
            &mrg_registry_path(source_root, registry.app_id.as_str()),
            registry,
        )
    }
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
