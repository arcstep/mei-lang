use std::path::{Path, PathBuf};

use crate::io::{read_json_registry, write_json_registry};
use crate::mrg::nodes::{deserialize_mrg_nodes, serialize_mrg_nodes, MrgNodeRecord};
use crate::paths::{mrg_registry_path, resolve_graph_root};
use crate::types::{current_time_ms, stable_hash, GraphNodeId, MaterialState, PayloadRef};
use mei_host_core::CacheLayersReady;
use mei_lang_kernel::resolve_workspace_graph_root;

/// Canonical MRG registry schema: host v3 fat slots + telemetry ∪ server v2 nodes.
pub const MRG_REGISTRY_SCHEMA_VERSION: &str = "mei-mrg-registry-v4";
pub const MRG_REGISTRY_SCHEMA_V3: &str = "mei-mrg-registry-v3";
pub const MRG_REGISTRY_SCHEMA_V2: &str = "mei-mrg-registry-v2";

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
    #[serde(rename = "dataSourceRevision", default)]
    pub data_source_revision: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "payloadRef",
        alias = "payload_ref"
    )]
    pub payload_ref: Option<PayloadRef>,
    #[serde(rename = "cachePolicy", default = "default_cache_policy")]
    pub cache_policy: String,
    #[serde(rename = "evalEngine", default = "default_eval_engine")]
    pub eval_engine: String,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "lastEval")]
    pub last_eval: Option<MrgLastEval>,
    #[serde(default, rename = "residentTier")]
    pub resident_tier: String,
    #[serde(default, rename = "clientEligible")]
    pub client_eligible: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "clientRevision"
    )]
    pub client_revision: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "payloadBytes"
    )]
    pub payload_bytes: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "tiersReady"
    )]
    pub tiers_ready: Option<CacheLayersReady>,
    #[serde(default, rename = "accessCount")]
    pub access_count: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "lastAccessMs"
    )]
    pub last_access_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "worksetId")]
    pub workset_id: Option<String>,
}

fn default_cache_policy() -> String {
    "artifact_sealed".to_string()
}

fn default_eval_engine() -> String {
    "json_walk".to_string()
}

impl MrgSlotRecord {
    /// Lean constructor used by server eval paths that don't track client tiers.
    pub fn lean(
        slot_id: MrgSlotId,
        slot_revision: String,
        state: MaterialState,
        owner_resource_id: String,
        metric_def_bundle_revision: String,
        data_source_revision: String,
        payload_ref: Option<PayloadRef>,
        cache_policy: String,
        eval_engine: String,
        last_eval: Option<MrgLastEval>,
    ) -> Self {
        Self {
            slot_id,
            slot_revision,
            state,
            owner_resource_id,
            metric_def_bundle_revision,
            data_source_revision,
            payload_ref,
            cache_policy,
            eval_engine,
            last_eval,
            resident_tier: "disk_only".to_string(),
            client_eligible: false,
            client_revision: None,
            payload_bytes: None,
            tiers_ready: None,
            access_count: 0,
            last_access_ms: None,
            workset_id: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MrgEdgeRecord {
    pub from: String,
    pub to: String,
    pub kind: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct MrgTelemetrySummary {
    #[serde(rename = "assembleCount")]
    pub assemble_count: u64,
    #[serde(rename = "metricsApiCount")]
    pub metrics_api_count: u64,
    #[serde(rename = "cacheHits")]
    pub cache_hits: u64,
    #[serde(rename = "cacheMisses")]
    pub cache_misses: u64,
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
    #[serde(
        default,
        deserialize_with = "deserialize_mrg_nodes",
        serialize_with = "serialize_mrg_nodes"
    )]
    pub nodes: Vec<MrgNodeRecord>,
    pub slots: Vec<MrgSlotRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<MrgEdgeRecord>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "telemetrySummary"
    )]
    pub telemetry_summary: Option<MrgTelemetrySummary>,
}

#[derive(Debug, Clone)]
pub enum MrgRegistryLoadError {
    UnsupportedSchema { got: String, path: PathBuf },
    Io(String),
}

impl std::fmt::Display for MrgRegistryLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema { got, path } => write!(
                f,
                "unsupported MRG registry schema `{got}` at {} (expected {MRG_REGISTRY_SCHEMA_VERSION}|{MRG_REGISTRY_SCHEMA_V3}|{MRG_REGISTRY_SCHEMA_V2})",
                path.display()
            ),
            Self::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for MrgRegistryLoadError {}

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
            telemetry_summary: Some(MrgTelemetrySummary::default()),
        }
    }

    pub fn migrate_in_place(&mut self) {
        if self.schema_version == MRG_REGISTRY_SCHEMA_VERSION {
            if self.telemetry_summary.is_none() {
                self.telemetry_summary = Some(MrgTelemetrySummary::default());
            }
            for slot in &mut self.slots {
                if slot.resident_tier.trim().is_empty() {
                    slot.resident_tier = "disk_only".to_string();
                }
                if slot.cache_policy.trim().is_empty() {
                    slot.cache_policy = default_cache_policy();
                }
                if slot.eval_engine.trim().is_empty() {
                    slot.eval_engine = default_eval_engine();
                }
            }
            return;
        }
        if self.schema_version == MRG_REGISTRY_SCHEMA_V2
            || self.schema_version == MRG_REGISTRY_SCHEMA_V3
        {
            if self.telemetry_summary.is_none() {
                self.telemetry_summary = Some(MrgTelemetrySummary::default());
            }
            for slot in &mut self.slots {
                if slot.resident_tier.trim().is_empty() {
                    slot.resident_tier = "disk_only".to_string();
                }
                if slot.cache_policy.trim().is_empty() {
                    slot.cache_policy = default_cache_policy();
                }
                if slot.eval_engine.trim().is_empty() {
                    slot.eval_engine = default_eval_engine();
                }
            }
            self.schema_version = MRG_REGISTRY_SCHEMA_VERSION.to_string();
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

    pub fn upsert_edge(&mut self, edge: MrgEdgeRecord) {
        if self.edges.iter().any(|existing| {
            existing.from == edge.from && existing.to == edge.to && existing.kind == edge.kind
        }) {
            return;
        }
        self.edges.push(edge);
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
            .filter(|slot| matches!(slot.state, MaterialState::Stale | MaterialState::Missing))
            .collect()
    }

    pub fn upsert_navigation_node(
        &mut self,
        key: &str,
        url: &str,
        scene_id: &str,
        target_file: &str,
        state: MaterialState,
    ) {
        let record = MrgNodeRecord::navigation(key, url, scene_id, target_file, state);
        if let Some(existing) = self.nodes.iter_mut().find(|node| node.node_key() == key) {
            *existing = record;
        } else {
            self.nodes.push(record);
        }
        let mut seen = false;
        self.nodes.retain(|node| {
            if node.node_key() == key {
                if seen {
                    return false;
                }
                seen = true;
            }
            true
        });
    }

    pub fn upsert_typed_node(&mut self, node: serde_json::Value) {
        let Some(record) = MrgNodeRecord::from_legacy_json(&node) else {
            return;
        };
        let key = record.node_key().to_string();
        if let Some(existing) = self
            .nodes
            .iter_mut()
            .find(|entry| entry.node_key() == key.as_str())
        {
            *existing = record;
        } else {
            self.nodes.push(record);
        }
    }

    pub fn finalize(&mut self) {
        self.schema_version = MRG_REGISTRY_SCHEMA_VERSION.to_string();
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

    pub fn tier_counts(&self) -> (usize, usize, usize) {
        let disk = self
            .slots
            .iter()
            .filter(|slot| matches!(slot.state, MaterialState::Ready))
            .count();
        let memory = self
            .slots
            .iter()
            .filter(|slot| slot.resident_tier == "memory_resident")
            .count();
        let client = self
            .slots
            .iter()
            .filter(|slot| slot.client_eligible)
            .count();
        (disk, memory, client)
    }
}

fn legacy_workspace_mrg_registry_path(source_root: &Path, app_id: &str) -> PathBuf {
    resolve_workspace_graph_root(source_root, app_id).join("mrg-registry.json")
}

fn accepted_schema(schema: &str) -> bool {
    schema == MRG_REGISTRY_SCHEMA_VERSION
        || schema == MRG_REGISTRY_SCHEMA_V3
        || schema == MRG_REGISTRY_SCHEMA_V2
}

pub struct MrgRegistryWriter;

impl MrgRegistryWriter {
    /// Load registry for mutation. Missing file → empty. Unsupported schema → empty
    /// **without writing**; callers that must fail should use [`Self::load_strict`].
    pub fn load(source_root: &Path, app_id: &str) -> MrgRegistry {
        match Self::load_strict(source_root, app_id) {
            Ok(registry) => registry,
            Err(MrgRegistryLoadError::UnsupportedSchema { got, path }) => {
                eprintln!(
                    "mei_host_graph::mrg: refusing unsupported MRG schema `{got}` at {}; returning empty in-memory only",
                    path.display()
                );
                MrgRegistry::empty(app_id)
            }
            Err(MrgRegistryLoadError::Io(message)) => {
                eprintln!(
                    "mei_host_graph::mrg: MRG registry load failed ({message}); returning empty"
                );
                MrgRegistry::empty(app_id)
            }
        }
    }

    pub fn load_strict(
        source_root: &Path,
        app_id: &str,
    ) -> Result<MrgRegistry, MrgRegistryLoadError> {
        let canonical = mrg_registry_path(source_root, app_id);
        if let Some(mut registry) = read_registry_at(&canonical)? {
            if !accepted_schema(registry.schema_version.as_str()) {
                return Err(MrgRegistryLoadError::UnsupportedSchema {
                    got: registry.schema_version,
                    path: canonical,
                });
            }
            registry.migrate_in_place();
            return Ok(registry);
        }

        let legacy = legacy_workspace_mrg_registry_path(source_root, app_id);
        if let Some(mut registry) = read_registry_at(&legacy)? {
            if !accepted_schema(registry.schema_version.as_str()) {
                return Err(MrgRegistryLoadError::UnsupportedSchema {
                    got: registry.schema_version,
                    path: legacy,
                });
            }
            registry.migrate_in_place();
            // One-shot promote into canonical app registry root.
            let _ = Self::save(source_root, &registry);
            return Ok(registry);
        }

        Ok(MrgRegistry::empty(app_id))
    }

    pub fn save(source_root: &Path, registry: &MrgRegistry) -> anyhow::Result<()> {
        let mut owned = registry.clone();
        owned.migrate_in_place();
        owned.schema_version = MRG_REGISTRY_SCHEMA_VERSION.to_string();
        let path = mrg_registry_path(source_root, owned.app_id.as_str());
        // Ensure parent exists even when app registry root was never created.
        let _ = resolve_graph_root(source_root, owned.app_id.as_str());
        write_json_registry(&path, &owned)
    }
}

fn read_registry_at(path: &Path) -> Result<Option<MrgRegistry>, MrgRegistryLoadError> {
    match read_json_registry::<MrgRegistry>(path) {
        Ok(value) => Ok(value),
        Err(error) => Err(MrgRegistryLoadError::Io(format!(
            "read/parse MRG registry {}: {error}",
            path.display()
        ))),
    }
}
