use crate::io::{read_json_registry, write_json_registry};
use crate::paths::mcg_registry_path;
use crate::types::{current_time_ms, stable_hash, GraphNodeId, MaterialState, PayloadRef};

pub const MCG_REGISTRY_SCHEMA_VERSION: &str = "mei-mcg-registry-v2";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AssemblyInputRef {
    pub kind: String,
    pub key: String,
    pub revision: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McgNodeRecord {
    pub id: GraphNodeId,
    pub revision: String,
    pub state: MaterialState,
    pub layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_ref: Option<PayloadRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ownerResourceId")]
    pub owner_resource_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "assemblyInputs")]
    pub assembly_inputs: Vec<AssemblyInputRef>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McgRegistry {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(rename = "registryRevision")]
    pub registry_revision: String,
    #[serde(rename = "updatedAtMs")]
    pub updated_at_ms: u64,
    pub nodes: Vec<McgNodeRecord>,
}

impl McgRegistry {
    pub fn empty(app_id: &str) -> Self {
        Self {
            schema_version: MCG_REGISTRY_SCHEMA_VERSION.to_string(),
            app_id: app_id.to_string(),
            registry_revision: String::new(),
            updated_at_ms: 0,
            nodes: Vec::new(),
        }
    }

    pub fn node_revision(&self, kind: &str, key: &str) -> Option<String> {
        self.nodes
            .iter()
            .find(|node| node.id.kind.slug() == kind && node.id.key == key)
            .map(|node| node.revision.clone())
    }

    pub fn upsert_node(&mut self, record: McgNodeRecord) {
        if let Some(existing) = self.nodes.iter_mut().find(|node| node.id == record.id) {
            *existing = record;
        } else {
            self.nodes.push(record);
        }
    }

    pub fn finalize(&mut self) {
        self.updated_at_ms = current_time_ms();
        let mut keys = self
            .nodes
            .iter()
            .map(|node| format!("{}={}", node.id.stable_key(), node.revision))
            .collect::<Vec<_>>();
        keys.sort();
        self.registry_revision = stable_hash(&keys.join("\n"));
    }

    pub fn nodes_of_kind(&self, kind: crate::types::GraphNodeKind) -> impl Iterator<Item = &McgNodeRecord> {
        self.nodes.iter().filter(move |node| node.id.kind == kind)
    }
}

pub struct McgRegistryWriter;

impl McgRegistryWriter {
    pub fn load(source_root: &std::path::Path, app_id: &str) -> McgRegistry {
        read_json_registry::<McgRegistry>(&mcg_registry_path(source_root, app_id))
            .ok()
            .flatten()
            .filter(|registry| registry.schema_version == MCG_REGISTRY_SCHEMA_VERSION)
            .unwrap_or_else(|| McgRegistry::empty(app_id))
    }

    pub fn save(source_root: &std::path::Path, registry: &McgRegistry) -> anyhow::Result<()> {
        write_json_registry(&mcg_registry_path(source_root, registry.app_id.as_str()), registry)
    }
}
