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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClientBootstrapPayload {
    client_revision: String,
    bootstrap_scope: String,
    #[serde(rename = "targetFile")]
    target_file: String,
    #[serde(rename = "compileEpoch")]
    compile_epoch: String,
    #[serde(rename = "dataGeneration")]
    data_generation: String,
    #[serde(rename = "appId")]
    app_id: String,
    metrics: Vec<ClientBootstrapMetric>,
}

pub fn client_bootstrap_root(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_var_root(app_root).join("client-bootstrap")
}

pub fn client_bootstrap_path(app_root: &Path, scope: &str) -> PathBuf {
    client_bootstrap_root(app_root).join(format!("{scope}.json"))
}

pub fn compute_scope_client_revision(
    scope: &str,
    content_hashes: &[&str],
    data_generation: &str,
) -> String {
    crate::mrg::tier::compute_client_revision(
        scope,
        &content_hashes.join("|"),
        data_generation,
    )
}

pub fn manifest_revision_from_registry(
    registry: &MrgRegistry,
    manifest: &ClientBootstrapManifest,
    data_generation: &str,
) -> Option<String> {
    let hashes = content_hashes_for_manifest_metrics(registry, manifest);
    if hashes.is_empty() {
        return None;
    }
    let refs: Vec<&str> = hashes.iter().map(String::as_str).collect();
    Some(compute_scope_client_revision(
        manifest.scope.as_str(),
        refs.as_slice(),
        data_generation,
    ))
}

pub fn bootstrap_embed_allowed(
    registry: &MrgRegistry,
    manifest: &ClientBootstrapManifest,
    data_generation: &str,
) -> bool {
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
    let Some(expected) = manifest_revision_from_registry(registry, manifest, data_generation) else {
        return false;
    };
    expected == manifest.client_revision
}

pub fn build_client_bootstrap_head_fragment(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Option<String> {
    let manifest = read_client_bootstrap(workspace_root, app_id, scene_id)?;
    let registry = crate::mrg::registry::MrgRegistryWriter::load(workspace_root, app_id);
    let app_root = resolve_app_root(workspace_root, app_id);
    let data_generation = load_cache_generation(app_root.as_path(), app_id).data_generation;
    if !bootstrap_embed_allowed(&registry, &manifest, data_generation.as_str()) {
        return None;
    }
    let target_file = format!("src/scene/{}/assembly.mei", manifest.scope);
    let compile_epoch = format!(
        "{}|{}|{}",
        mei_lang_kernel::scene_payload_cache_epoch(),
        mei_lang_kernel::dataset_materialize_cache_epoch(),
        target_file
    );
    let payload = ClientBootstrapPayload {
        client_revision: manifest.client_revision.clone(),
        bootstrap_scope: manifest.scope.clone(),
        target_file: target_file.clone(),
        compile_epoch,
        data_generation: data_generation.clone(),
        app_id: app_id.to_string(),
        metrics: manifest.metrics.clone(),
    };
    let payload_json =
        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    let metric_count = manifest.metrics.len();
    Some(format!(
        r#"<meta name="mei-bootstrap-inlined" content="1" /><meta name="mei-bootstrap-metric-count" content="{metric_count}" /><script type="application/json" id="mei-client-bootstrap">{payload_json}</script><script>window.__mei=window.__mei||{{}};(function(){{try{{var el=document.getElementById("mei-client-bootstrap");if(!el)return;var p=JSON.parse(el.textContent||"{{}}");if(p.client_revision)window.__mei.client_revision=p.client_revision;if(p.bootstrap_scope)window.__mei.bootstrap_scope=p.bootstrap_scope;if(p.targetFile)window.__mei.bootstrap_target_file=p.targetFile;if(p.compileEpoch)window.__mei.bootstrap_compile_epoch=p.compileEpoch;if(p.dataGeneration)window.__mei.bootstrap_data_generation=p.dataGeneration;if(p.appId)window.__mei.bootstrap_app_id=p.appId;if(Array.isArray(p.metrics))window.__mei.bootstrap_metrics=p.metrics;}}catch(e){{}}}})();</script>"#
    ))
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
    let mut eligible: Vec<_> = descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.client_eligible && descriptor.cache_layers_ready.client
        })
        .collect();
    eligible.sort_by(|left, right| left.slot_key.cmp(&right.slot_key));
    let eligible: Vec<_> = eligible.into_iter().take(max_metrics).collect();
    if eligible.is_empty() {
        return Ok(None);
    }
    let data_generation = load_cache_generation(app_root, app_id).data_generation;
    let content_hashes: Vec<&str> = eligible
        .iter()
        .map(|descriptor| descriptor.content_hash.as_str())
        .collect();
    let client_revision =
        compute_scope_client_revision(scope, content_hashes.as_slice(), data_generation.as_str());
    let mut manifest_metrics = Vec::new();
    for descriptor in eligible {
        let metric_id = descriptor
            .slot_key
            .rsplit("::")
            .next()
            .unwrap_or(descriptor.slot_key.as_str());
        let Some(contract) = metrics.get(metric_id) else {
            continue;
        };
        let dataset_id = Some(descriptor.owner_resource_id.clone()).or_else(|| {
            contract
                .dataset
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
        manifest_metrics.push(ClientBootstrapMetric {
            id: metric_id.to_string(),
            dataset_id,
            contract: contract.clone(),
        });
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

fn content_hashes_for_manifest_metrics(
    registry: &MrgRegistry,
    manifest: &ClientBootstrapManifest,
) -> Vec<String> {
    let metric_ids: BTreeSet<&str> = manifest.metrics.iter().map(|metric| metric.id.as_str()).collect();
    let mut slots: Vec<_> = registry
        .slots
        .iter()
        .filter(|slot| {
            slot.slot_id.scope_key == manifest.scope
                && slot.client_eligible
                && matches!(slot.state, MaterialState::Ready)
        })
        .collect();
    slots.sort_by(|left, right| left.slot_id.node.key.cmp(&right.slot_id.node.key));
    let mut hashes = Vec::new();
    for slot in slots {
        let metric_id = slot
            .slot_id
            .node
            .key
            .rsplit("::")
            .next()
            .unwrap_or("");
        if !metric_ids.contains(metric_id) {
            continue;
        }
        if let Some(payload_ref) = slot.payload_ref.as_ref() {
            hashes.push(payload_ref.content_hash.clone());
        }
    }
    hashes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mrg::registry::{MrgLastEval, MrgRegistry, MrgSlotId, MrgSlotRecord};
    use crate::types::{GraphNodeId, GraphNodeKind, PayloadRef};
    use mei_lang_kernel::MetricShape;

    fn sample_slot(
        state: MaterialState,
        slot_key: &str,
        content_hash: &str,
    ) -> MrgSlotRecord {
        MrgSlotRecord {
            slot_id: MrgSlotId {
                node: GraphNodeId::new(GraphNodeKind::MaterialSlot, slot_key.to_string()),
                scope_key: "home".to_string(),
            },
            slot_revision: "sr:1".to_string(),
            state,
            owner_resource_id: "__world_metrics__::metrics/demo.bundle.mei".to_string(),
            metric_def_bundle_revision: "bundle".to_string(),
            data_source_revision: "ds".to_string(),
            payload_ref: Some(PayloadRef::new(
                "metric_response",
                content_hash,
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
            client_revision: None,
            payload_bytes: None,
            tiers_ready: None,
            access_count: 0,
            last_access_ms: None,
            workset_id: Some("workset:home:0".to_string()),
        }
    }

    fn sample_descriptor(slot_key: &str, content_hash: &str) -> EvalSlotDescriptor {
        EvalSlotDescriptor {
            slot_key: slot_key.to_string(),
            scope_key: "home".to_string(),
            owner_resource_id: "__world_metrics__::metrics/demo.bundle.mei".to_string(),
            metric_def_bundle_revision: "bundle".to_string(),
            data_source_revision: "ds".to_string(),
            payload_kind: "metric_response".to_string(),
            content_hash: content_hash.to_string(),
            schema_version: "mei-metric-response-result-artifact-v1".to_string(),
            wall_ms: 1,
            artifact_hit: true,
            workset_id: "workset:home:0".to_string(),
            cache_layer: "client".to_string(),
            cache_layers_ready: mei_host_core::CacheLayersReady {
                disk: true,
                memory: true,
                client: true,
            },
            client_revision: None,
            resident_tier: "memory_resident".to_string(),
            client_eligible: true,
            payload_bytes: None,
        }
    }

    #[test]
    fn bootstrap_embed_allowed_rejects_stale_client_slots() {
        let manifest = ClientBootstrapManifest {
            schema_version: "mei-client-bootstrap-v1".to_string(),
            scope: "home".to_string(),
            client_revision: "rev".to_string(),
            workset_id: "workset:home:0".to_string(),
            metrics: vec![ClientBootstrapMetric {
                id: "metric_a".to_string(),
                dataset_id: None,
                contract: MetricContract {
                    id: "metric_a".to_string(),
                    label: None,
                    unit: None,
                    value_format: None,
                    purpose: None,
                    shape: MetricShape::Scalar,
                    schema: vec![],
                    dataset: None,
                    transforms: vec![],
                    value: serde_json::json!(1),
                },
            }],
        };
        let mut registry = MrgRegistry::empty("demo");
        registry.upsert_slot(sample_slot(
            MaterialState::Stale,
            "workset:home:0::metric_a",
            "hash-a",
        ));
        assert!(!bootstrap_embed_allowed(&registry, &manifest, "gen-1"));
    }

    #[test]
    fn bootstrap_embed_allowed_matches_aggregate_revision() {
        let data_generation = "gen-1";
        let revision = compute_scope_client_revision("home", &["hash-a"], data_generation);
        let manifest = ClientBootstrapManifest {
            schema_version: "mei-client-bootstrap-v1".to_string(),
            scope: "home".to_string(),
            client_revision: revision.clone(),
            workset_id: "workset:home:0".to_string(),
            metrics: vec![ClientBootstrapMetric {
                id: "metric_a".to_string(),
                dataset_id: Some("__world_metrics__::metrics/demo.bundle.mei".to_string()),
                contract: MetricContract {
                    id: "metric_a".to_string(),
                    label: None,
                    unit: None,
                    value_format: None,
                    purpose: None,
                    shape: MetricShape::Scalar,
                    schema: vec![],
                    dataset: None,
                    transforms: vec![],
                    value: serde_json::json!(1),
                },
            }],
        };
        let mut registry = MrgRegistry::empty("demo");
        registry.upsert_slot(sample_slot(
            MaterialState::Ready,
            "workset:home:0::metric_a",
            "hash-a",
        ));
        assert!(bootstrap_embed_allowed(&registry, &manifest, data_generation));
        let mut stale_manifest = manifest.clone();
        stale_manifest.client_revision = "stale".to_string();
        assert!(!bootstrap_embed_allowed(
            &registry,
            &stale_manifest,
            data_generation
        ));
    }

    #[test]
    fn write_client_bootstrap_roundtrip_allows_embed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let app_root = temp.path().join("apps").join("demo");
        std::fs::create_dir_all(app_root.join("var/active")).expect("var");
        let mut metrics = BTreeMap::new();
        metrics.insert(
            "metric_a".to_string(),
            MetricContract {
                id: "metric_a".to_string(),
                label: None,
                unit: None,
                value_format: None,
                purpose: None,
                shape: MetricShape::Scalar,
                schema: vec![],
                dataset: None,
                transforms: vec![],
                value: serde_json::json!(42),
            },
        );
        let descriptor = sample_descriptor("workset:home:0::metric_a", "hash-a");
        let manifest = write_client_bootstrap(
            app_root.as_path(),
            "demo",
            "home",
            "workset:home:0",
            std::slice::from_ref(&descriptor),
            &metrics,
            32,
        )
        .expect("write")
        .expect("manifest");
        let mut registry = MrgRegistry::empty("demo");
        registry.upsert_slot(sample_slot(
            MaterialState::Ready,
            "workset:home:0::metric_a",
            "hash-a",
        ));
        let data_generation = load_cache_generation(app_root.as_path(), "demo").data_generation;
        assert!(bootstrap_embed_allowed(&registry, &manifest, data_generation.as_str()));
        assert_eq!(manifest.metrics[0].dataset_id.as_deref(), Some("__world_metrics__::metrics/demo.bundle.mei"));
    }

    #[test]
    fn client_bootstrap_manifest_roundtrip_json() {
        let manifest = ClientBootstrapManifest {
            schema_version: "mei-client-bootstrap-v1".to_string(),
            scope: "home".to_string(),
            client_revision: "rev-a".to_string(),
            workset_id: "workset:home:0".to_string(),
            metrics: Vec::new(),
        };
        let raw = serde_json::to_string(&manifest).expect("serialize");
        let parsed: ClientBootstrapManifest = serde_json::from_str(&raw).expect("deserialize");
        assert_eq!(parsed.client_revision, "rev-a");
        assert_eq!(parsed.scope, "home");
    }

    #[test]
    fn build_client_bootstrap_head_fragment_includes_payload_and_meta() {
        let temp = tempfile::tempdir().expect("tempdir");
        let workspace = temp.path();
        let app_root = workspace.join("apps").join("demo");
        std::fs::create_dir_all(app_root.join("var/active")).expect("var");
        let mut metrics = BTreeMap::new();
        metrics.insert(
            "metric_a".to_string(),
            MetricContract {
                id: "metric_a".to_string(),
                label: None,
                unit: None,
                value_format: None,
                purpose: None,
                shape: MetricShape::Scalar,
                schema: vec![],
                dataset: None,
                transforms: vec![],
                value: serde_json::json!(7),
            },
        );
        let descriptor = sample_descriptor("workset:home:0::metric_a", "hash-a");
        write_client_bootstrap(
            app_root.as_path(),
            "demo",
            "home",
            "workset:home:0",
            std::slice::from_ref(&descriptor),
            &metrics,
            32,
        )
        .expect("write");
        let mut registry = MrgRegistry::empty("demo");
        registry.upsert_slot(sample_slot(
            MaterialState::Ready,
            "workset:home:0::metric_a",
            "hash-a",
        ));
        crate::mrg::registry::MrgRegistryWriter::save(workspace, &registry).expect("save mrg");
        let fragment =
            build_client_bootstrap_head_fragment(workspace, "demo", "home").expect("fragment");
        assert!(fragment.contains("mei-client-bootstrap"));
        assert!(fragment.contains("mei-bootstrap-inlined"));
        assert!(fragment.contains("bootstrap_compile_epoch") || fragment.contains("compileEpoch"));
    }
}
