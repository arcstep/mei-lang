use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::graph::io::{read_json_registry, write_json_registry};
use crate::graph::paths::mcg_registry_path;
use crate::graph::types::{stable_hash, GraphNodeId, MaterialState, PayloadRef};

pub const MCG_REGISTRY_SCHEMA_VERSION: &str = "mei-mcg-registry-v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssemblyInputRef {
    pub kind: String,
    pub key: String,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McgNodeRecord {
    pub id: GraphNodeId,
    pub revision: String,
    pub state: MaterialState,
    pub layer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_ref: Option<PayloadRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "defsFingerprint"
    )]
    pub defs_fingerprint: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "ownerResourceId"
    )]
    pub owner_resource_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "assemblyInputs"
    )]
    pub assembly_inputs: Vec<AssemblyInputRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<BTreeMap<String, u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McgEdgeRecord {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    pub edges: Vec<McgEdgeRecord>,
}

impl McgRegistry {
    pub fn empty(app_id: &str) -> Self {
        Self {
            schema_version: MCG_REGISTRY_SCHEMA_VERSION.to_string(),
            app_id: app_id.to_string(),
            registry_revision: String::new(),
            updated_at_ms: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
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
        write_json_registry(
            &mcg_registry_path(source_root, app_id_from_registry(registry)),
            registry,
        )
    }
}

fn app_id_from_registry(registry: &McgRegistry) -> &str {
    registry.app_id.as_str()
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::types::GraphNodeKind;

    #[test]
    fn upsert_and_finalize_revision() {
        let mut registry = McgRegistry::empty("zhifa");
        registry.upsert_node(McgNodeRecord {
            id: GraphNodeId::new(GraphNodeKind::ScenePayload, "scenes/home.mei"),
            revision: "nr:abc".to_string(),
            state: MaterialState::Ready,
            layer: "compile".to_string(),
            payload_ref: None,
            deps: Vec::new(),
            defs_fingerprint: None,
            owner_resource_id: None,
            assembly_inputs: Vec::new(),
            stats: None,
        });
        registry.finalize();
        assert!(!registry.registry_revision.is_empty());
        assert_eq!(
            registry.node_revision("scene_payload", "scenes/home.mei"),
            Some("nr:abc".to_string())
        );
    }
}
