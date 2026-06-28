use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use mei_host_core::EvalSlotDescriptor;
use mei_lang_kernel::{load_cache_generation, resolve_app_root, MetricContract};
use serde::{Deserialize, Serialize};

use crate::mrg::registry::MrgRegistry;
use crate::types::MaterialState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientBootstrapMetric {
    pub id: String,
    pub contract: MetricContract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientBootstrapManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub scope: String,
    #[serde(rename = "clientRevision")]
    pub client_revision: String,
    #[serde(rename = "worksetId")]
    pub workset_id: String,
    pub metrics: Vec<ClientBootstrapMetric>,
}

pub fn client_bootstrap_root(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_var_root(app_root).join("client-bootstrap")
}

pub fn client_bootstrap_path(app_root: &Path, scope: &str) -> PathBuf {
    client_bootstrap_root(app_root).join(format!("{scope}.json"))
}

pub fn bootstrap_embed_allowed(registry: &MrgRegistry, manifest: &ClientBootstrapManifest) -> bool {
    let client_slots: Vec<_> = registry
        .slots
        .iter()
        .filter(|slot| slot.slot_id.scope_key == manifest.scope && slot.client_eligible)
        .collect();
    if client_slots.is_empty() {
        return true;
    }
    if client_slots
        .iter()
        .any(|slot| !matches!(slot.state, MaterialState::Ready))
    {
        return false;
    }
    let revisions: BTreeSet<&str> = client_slots
        .iter()
        .filter_map(|slot| slot.client_revision.as_deref())
        .collect();
    if revisions.is_empty() {
        return true;
    }
    revisions.contains(manifest.client_revision.as_str())
}

pub fn clear_client_bootstrap_for_scope(app_root: &Path, scope: &str) -> bool {
    let path = client_bootstrap_path(app_root, scope);
    if path.is_file() {
        fs::remove_file(&path).is_ok()
    } else {
        false
    }
}

pub fn clear_client_bootstraps_for_stale_scopes(
    app_root: &Path,
    registry: &MrgRegistry,
) -> usize {
    let mut scopes = BTreeSet::new();
    for slot in &registry.slots {
        if slot.client_eligible && matches!(slot.state, MaterialState::Stale) {
            scopes.insert(slot.slot_id.scope_key.clone());
        }
    }
    scopes
        .iter()
        .filter(|scope| clear_client_bootstrap_for_scope(app_root, scope.as_str()))
        .count()
}

pub fn write_client_bootstrap(
    app_root: &Path,
    app_id: &str,
    scope: &str,
    workset_id: &str,
    descriptors: &[EvalSlotDescriptor],
    metrics: &BTreeMap<String, MetricContract>,
    max_metrics: usize,
) -> anyhow::Result<Option<ClientBootstrapManifest>> {
    let eligible: Vec<_> = descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.client_eligible && descriptor.cache_layers_ready.client
        })
        .take(max_metrics)
        .collect();
    if eligible.is_empty() {
        return Ok(None);
    }
    let data_generation = load_cache_generation(app_root, app_id).data_generation;
    let client_revision = crate::mrg::tier::compute_client_revision(
        scope,
        &eligible
            .iter()
            .map(|descriptor| descriptor.content_hash.as_str())
            .collect::<Vec<_>>()
            .join("|"),
        data_generation.as_str(),
    );
    let mut manifest_metrics = Vec::new();
    for descriptor in eligible {
        let metric_id = descriptor
            .slot_key
            .rsplit("::")
            .next()
            .unwrap_or(descriptor.slot_key.as_str());
        if let Some(contract) = metrics.get(metric_id) {
            manifest_metrics.push(ClientBootstrapMetric {
                id: metric_id.to_string(),
                contract: contract.clone(),
            });
        }
    }
    if manifest_metrics.is_empty() {
        return Ok(None);
    }
    let manifest = ClientBootstrapManifest {
        schema_version: "mei-client-bootstrap-v1".to_string(),
        scope: scope.to_string(),
        client_revision,
        workset_id: workset_id.to_string(),
        metrics: manifest_metrics,
    };
    let path = client_bootstrap_path(app_root, scope);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(&manifest)?)?;
    Ok(Some(manifest))
}

pub fn read_client_bootstrap(
    source_root: &Path,
    app_id: &str,
    scope: &str,
) -> Option<ClientBootstrapManifest> {
    let app_root = resolve_app_root(source_root, app_id);
    let path = client_bootstrap_path(app_root.as_path(), scope);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mrg::registry::{MrgLastEval, MrgRegistry, MrgSlotId, MrgSlotRecord};
    use crate::types::{GraphNodeId, GraphNodeKind, PayloadRef};

    fn sample_manifest() -> ClientBootstrapManifest {
        ClientBootstrapManifest {
            schema_version: "mei-client-bootstrap-v1".to_string(),
            scope: "home".to_string(),
            client_revision: "rev-a".to_string(),
            workset_id: "workset:home:0".to_string(),
            metrics: Vec::new(),
        }
    }

    fn sample_slot(state: MaterialState, client_revision: Option<&str>) -> MrgSlotRecord {
        MrgSlotRecord {
            slot_id: MrgSlotId {
                node: GraphNodeId::new(GraphNodeKind::MaterialSlot, "workset:home:0::metric_a".to_string()),
                scope_key: "home".to_string(),
            },
            slot_revision: "sr:1".to_string(),
            state,
            owner_resource_id: "__world_metrics__::bundle".to_string(),
            metric_def_bundle_revision: "bundle".to_string(),
            data_source_revision: "ds".to_string(),
            payload_ref: Some(PayloadRef::new(
                "metric_response",
                "cache-key",
                "mei-metric-response-result-artifact-v1",
            )),
            cache_policy: "artifact_sealed".to_string(),
            eval_engine: "json_walk".to_string(),
            last_eval: Some(MrgLastEval {
                at_ms: 0,
                wall_ms: 1,
                artifact_hit: true,
                cache_layer: "disk".to_string(),
            }),
            resident_tier: "disk_only".to_string(),
            client_eligible: true,
            client_revision: client_revision.map(str::to_string),
            payload_bytes: None,
            tiers_ready: None,
            access_count: 0,
            last_access_ms: None,
            workset_id: Some("workset:home:0".to_string()),
        }
    }

    #[test]
    fn bootstrap_embed_allowed_rejects_stale_client_slots() {
        let mut registry = MrgRegistry::empty("demo");
        registry.upsert_slot(sample_slot(MaterialState::Stale, Some("rev-a")));
        assert!(!bootstrap_embed_allowed(&registry, &sample_manifest()));
    }

    #[test]
    fn bootstrap_embed_allowed_requires_matching_client_revision() {
        let mut registry = MrgRegistry::empty("demo");
        registry.upsert_slot(sample_slot(MaterialState::Ready, Some("rev-b")));
        assert!(!bootstrap_embed_allowed(&registry, &sample_manifest()));
        let mut ready = sample_slot(MaterialState::Ready, Some("rev-a"));
        ready.slot_id.node.key = "workset:home:1::metric_b".to_string();
        registry.upsert_slot(ready);
        assert!(bootstrap_embed_allowed(&registry, &sample_manifest()));
    }

    #[test]
    fn client_bootstrap_manifest_roundtrip_json() {
        let manifest = sample_manifest();
        let raw = serde_json::to_string(&manifest).expect("serialize");
        let parsed: ClientBootstrapManifest = serde_json::from_str(&raw).expect("deserialize");
        assert_eq!(parsed.client_revision, "rev-a");
        assert_eq!(parsed.scope, "home");
    }
}
