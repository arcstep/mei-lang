use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use mei_host_core::{EvalSlotDescriptor, HostContext};
use mei_lang_kernel::{
    load_cache_generation, load_mei_config_for_app, resolve_app_root, MetricContract,
};
use serde::{Deserialize, Serialize};

use crate::mrg::registry::MrgRegistry;
use crate::types::MaterialState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientBootstrapMetric {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_rows: Option<usize>,
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
pub struct ClientBootstrapScopePayload {
    #[serde(rename = "clientRevision")]
    client_revision: String,
    #[serde(rename = "bootstrapScope")]
    bootstrap_scope: String,
    #[serde(rename = "targetFile")]
    target_file: String,
    #[serde(rename = "compileEpoch")]
    compile_epoch: String,
    metrics: Vec<ClientBootstrapMetric>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientBootstrapPayload {
    #[serde(rename = "clientRevision")]
    client_revision: String,
    #[serde(rename = "bootstrapScope")]
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
    #[serde(
        rename = "bootstrapScopes",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    bootstrap_scopes: Vec<ClientBootstrapScopePayload>,
    #[serde(
        rename = "layoutBudgetManifest",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    layout_budget_manifest: Option<mei_lang_kernel::LayoutBudgetManifest>,
}

/// Revision token used when the app/scene does not require client bootstrap artifacts.
pub const NO_CLIENT_BOOTSTRAP_REVISION: &str = "__no_client_bootstrap__";

pub fn empty_client_bootstrap_payload(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> ClientBootstrapPayload {
    let app_root = resolve_app_root(workspace_root, app_id);
    let data_generation = load_cache_generation(app_root.as_path(), app_id).data_generation;
    ClientBootstrapPayload {
        client_revision: NO_CLIENT_BOOTSTRAP_REVISION.to_string(),
        bootstrap_scope: scene_id.to_string(),
        target_file: String::new(),
        compile_epoch: String::new(),
        data_generation,
        app_id: app_id.to_string(),
        metrics: Vec::new(),
        bootstrap_scopes: Vec::new(),
        layout_budget_manifest: None,
    }
}

pub fn client_bootstrap_root(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_var_root(app_root).join("client-bootstrap")
}

pub fn client_bootstrap_path(app_root: &Path, scope: &str) -> PathBuf {
    client_bootstrap_root(app_root).join(format!("{scope}.json"))
}

pub fn scene_bootstrap_artifact_root(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_var_root(app_root).join("scene-bootstrap")
}

pub fn scene_bootstrap_artifact_path(
    app_root: &Path,
    scope: &str,
    client_revision: &str,
) -> PathBuf {
    scene_bootstrap_artifact_root(app_root)
        .join(format!("{scope}.{client_revision}.json"))
}

pub fn scene_bootstrap_artifact_public_url(
    app_id: &str,
    scope: &str,
    client_revision: &str,
) -> String {
    format!(
        "/api/host/scene-bootstrap?app={app_id}&scene={scope}&revision={client_revision}"
    )
}

pub fn write_scene_bootstrap_artifact(
    workspace_root: &Path,
    app_id: &str,
    scope: &str,
    payload: &ClientBootstrapPayload,
) -> Option<PathBuf> {
    let revision = payload.client_revision.trim();
    if revision.is_empty() {
        return None;
    }
    let app_root = resolve_app_root(workspace_root, app_id);
    let path = scene_bootstrap_artifact_path(app_root.as_path(), scope, revision);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let written = fs::write(path.as_path(), serde_json::to_string_pretty(payload).ok()?).is_ok();
    if written {
        Some(path)
    } else {
        None
    }
}

pub fn read_scene_bootstrap_artifact(
    workspace_root: &Path,
    app_id: &str,
    scope: &str,
    client_revision: &str,
) -> Option<ClientBootstrapPayload> {
    let app_root = resolve_app_root(workspace_root, app_id);
    let path = scene_bootstrap_artifact_path(app_root.as_path(), scope, client_revision);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn compute_scope_client_revision(
    scope: &str,
    content_hashes: &[&str],
    data_generation: &str,
) -> String {
    crate::mrg::tier::compute_client_revision(scope, &content_hashes.join("|"), data_generation)
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootstrapEmbedStatus {
    pub allowed: bool,
    pub reason: String,
    pub metric_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<String>,
}

pub fn bootstrap_embed_status_for_manifest(
    registry: &MrgRegistry,
    manifest: &ClientBootstrapManifest,
    data_generation: &str,
) -> BootstrapEmbedStatus {
    let metric_count = manifest.metrics.len();
    let client_revision = Some(manifest.client_revision.clone());
    let client_slots: Vec<_> = registry
        .slots
        .iter()
        .filter(|slot| slot.slot_id.scope_key == manifest.scope && slot.client_eligible)
        .collect();
    if client_slots.is_empty() {
        return BootstrapEmbedStatus {
            allowed: true,
            reason: "no_client_slots".to_string(),
            metric_count,
            client_revision,
            expected_revision: None,
        };
    }
    if client_slots
        .iter()
        .any(|slot| !matches!(slot.state, MaterialState::Ready))
    {
        return BootstrapEmbedStatus {
            allowed: false,
            reason: "slots_not_ready".to_string(),
            metric_count,
            client_revision,
            expected_revision: None,
        };
    }
    let Some(expected) = manifest_revision_from_registry(registry, manifest, data_generation) else {
        return BootstrapEmbedStatus {
            allowed: false,
            reason: "revision_unavailable".to_string(),
            metric_count,
            client_revision,
            expected_revision: None,
        };
    };
    if expected != manifest.client_revision {
        return BootstrapEmbedStatus {
            allowed: false,
            reason: "revision_mismatch".to_string(),
            metric_count,
            client_revision,
            expected_revision: Some(expected),
        };
    }
    BootstrapEmbedStatus {
        allowed: true,
        reason: "allowed".to_string(),
        metric_count,
        client_revision,
        expected_revision: Some(expected),
    }
}

pub fn bootstrap_embed_status(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> BootstrapEmbedStatus {
    let app_root = resolve_app_root(workspace_root, app_id);
    let Some(manifest) = read_client_bootstrap(workspace_root, app_id, scene_id) else {
        if !scene_requires_client_bootstrap(workspace_root, app_id, scene_id) {
            return BootstrapEmbedStatus {
                allowed: true,
                reason: "no_client_bootstrap_required".to_string(),
                metric_count: 0,
                client_revision: None,
                expected_revision: None,
            };
        }
        return BootstrapEmbedStatus {
            allowed: false,
            reason: "manifest_missing".to_string(),
            metric_count: 0,
            client_revision: None,
            expected_revision: None,
        };
    };
    let registry = crate::mrg::registry::MrgRegistryWriter::load(workspace_root, app_id);
    let data_generation = load_cache_generation(app_root.as_path(), app_id).data_generation;
    bootstrap_embed_status_for_manifest(&registry, &manifest, data_generation.as_str())
}

pub fn bootstrap_embed_allowed(
    registry: &MrgRegistry,
    manifest: &ClientBootstrapManifest,
    data_generation: &str,
) -> bool {
    bootstrap_embed_status_for_manifest(registry, manifest, data_generation).allowed
}

pub fn scene_requires_client_bootstrap(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> bool {
    let registry = crate::mrg::registry::MrgRegistryWriter::load(workspace_root, app_id);
    registry.slots.iter().any(|slot| {
        slot.client_eligible && slot.slot_id.scope_key == scene_id
    })
}

pub fn build_client_bootstrap_head_fragment(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Option<String> {
    let payload = build_client_bootstrap_payload(workspace_root, app_id, scene_id)?;
    let payload_json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    let metric_count = payload
        .bootstrap_scopes
        .iter()
        .map(|scope| scope.metrics.len())
        .sum::<usize>();
    let artifact_url = scene_bootstrap_artifact_public_url(app_id, scene_id, payload.client_revision.as_str());
    let _ = write_scene_bootstrap_artifact(workspace_root, app_id, scene_id, &payload);
    Some(format!(
        r#"<meta name="mei-bootstrap-inlined" content="1" /><meta name="mei-bootstrap-metric-count" content="{metric_count}" /><meta name="mei-bootstrap-artifact-url" content="{artifact_url}" /><script type="application/json" id="mei-client-bootstrap">{payload_json}</script><script>window.__mei=window.__mei||{{}};(function(){{try{{var el=document.getElementById("mei-client-bootstrap");if(!el)return;var p=JSON.parse(el.textContent||"{{}}");if(p.clientRevision)window.__mei.client_revision=p.clientRevision;if(p.bootstrapScope)window.__mei.bootstrap_scope=p.bootstrapScope;if(p.targetFile)window.__mei.bootstrap_target_file=p.targetFile;if(p.compileEpoch)window.__mei.bootstrap_compile_epoch=p.compileEpoch;if(p.dataGeneration)window.__mei.bootstrap_data_generation=p.dataGeneration;if(p.appId)window.__mei.bootstrap_app_id=p.appId;if(Array.isArray(p.metrics))window.__mei.bootstrap_metrics=p.metrics;if(Array.isArray(p.bootstrapScopes))window.__mei.bootstrap_scopes=p.bootstrapScopes;window.__mei.bootstrap_artifact_url="{artifact_url}";window.__meiBootstrapPayloadReady=1;}}catch(e){{window.__meiBootstrapSeedError="bootstrap_parse_failed";}}try{{document.dispatchEvent(new CustomEvent("mei-bootstrap-ready"));}}catch(e){{}}}})();</script>"#
    ))
}

pub fn build_client_bootstrap_payload(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Option<ClientBootstrapPayload> {
    let app_root = resolve_app_root(workspace_root, app_id);
    let config = load_mei_config_for_app(app_root.as_path(), None);
    let client_cfg = config.runtime.client_bootstrap.unwrap_or_default();
    let registry = crate::mrg::registry::MrgRegistryWriter::load(workspace_root, app_id);
    let data_generation = load_cache_generation(app_root.as_path(), app_id).data_generation;
    let mut candidate_scopes = vec![scene_id.to_string()];
    if client_cfg.neighbor_hops > 0 {
        let ctx = HostContext::new(workspace_root.to_path_buf(), app_id.to_string());
        let linked = crate::mrg::frontier::linked_board_scenes_for_scope(
            &ctx,
            scene_id,
            client_cfg.neighbor_hops,
        )
        .unwrap_or_default();
        for scope in linked.into_iter().take(client_cfg.max_neighbor_scopes) {
            if !candidate_scopes.contains(&scope) {
                candidate_scopes.push(scope);
            }
        }
    }
    let mut scope_payloads = Vec::new();
    for scope in candidate_scopes {
        let Some(manifest) = read_client_bootstrap(workspace_root, app_id, scope.as_str()) else {
            continue;
        };
        if !bootstrap_embed_allowed(&registry, &manifest, data_generation.as_str()) {
            continue;
        }
        scope_payloads.push(scope_payload_from_manifest(
            workspace_root,
            app_id,
            &manifest,
        ));
    }
    let primary = scope_payloads
        .iter()
        .find(|scope| scope.bootstrap_scope == scene_id)
        .cloned()
        .or_else(|| scope_payloads.first().cloned())?;
    let layout_budget_manifest =
        layout_budget_manifest_for_scope(workspace_root, app_id, scene_id);
    Some(ClientBootstrapPayload {
        client_revision: primary.client_revision.clone(),
        bootstrap_scope: primary.bootstrap_scope.clone(),
        target_file: primary.target_file.clone(),
        compile_epoch: primary.compile_epoch.clone(),
        data_generation: data_generation.clone(),
        app_id: app_id.to_string(),
        metrics: primary.metrics.clone(),
        bootstrap_scopes: scope_payloads,
        layout_budget_manifest,
    })
}

fn layout_budget_manifest_for_scope(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Option<mei_lang_kernel::LayoutBudgetManifest> {
    let outcome =
        crate::assemble::assemble_scope_from_registry(workspace_root, app_id, scene_id).ok()??;
    if outcome.compiled.ui_layout_index.nodes.is_empty() {
        return None;
    }
    let revision = format!(
        "{}:{}",
        outcome.compile_revision,
        mei_lang_kernel::ops_layout_tuning_revision_digest(
            &load_mei_config_for_app(
                resolve_app_root(workspace_root, app_id).as_path(),
                Some(workspace_root),
            )
            .ops,
        )
    );
    Some(
        outcome
            .compiled
            .ui_layout_index
            .layout_budget_manifest(revision.as_str()),
    )
}

#[allow(dead_code)]
fn layout_budget_manifest_for_pilot(
    workspace_root: &Path,
    app_id: &str,
    scene_id: &str,
) -> Option<mei_lang_kernel::LayoutBudgetManifest> {
    layout_budget_manifest_for_scope(workspace_root, app_id, scene_id)
}

pub fn clear_client_bootstrap_for_scope(app_root: &Path, scope: &str) -> bool {
    let path = client_bootstrap_path(app_root, scope);
    if path.is_file() {
        fs::remove_file(&path).is_ok()
    } else {
        false
    }
}

pub fn clear_client_bootstraps_for_stale_scopes(app_root: &Path, registry: &MrgRegistry) -> usize {
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
    metric_total_rows: &BTreeMap<String, usize>,
    max_metrics: usize,
) -> anyhow::Result<Option<ClientBootstrapManifest>> {
    let mut eligible: Vec<_> = descriptors
        .iter()
        .filter(|descriptor| {
            descriptor.scope_key == scope
                && descriptor.client_eligible
                && descriptor.cache_layers_ready.client
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
            total_rows: metric_total_rows.get(metric_id).copied(),
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
    let metric_ids: BTreeSet<&str> = manifest
        .metrics
        .iter()
        .map(|metric| metric.id.as_str())
        .collect();
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
        let metric_id = slot.slot_id.node.key.rsplit("::").next().unwrap_or("");
        if !metric_ids.contains(metric_id) {
            continue;
        }
        if let Some(payload_ref) = slot.payload_ref.as_ref() {
            hashes.push(payload_ref.content_hash.clone());
        }
    }
    hashes
}

fn scope_payload_from_manifest(
    workspace_root: &Path,
    app_id: &str,
    manifest: &ClientBootstrapManifest,
) -> ClientBootstrapScopePayload {
    let target_file =
        resolve_target_file_for_scope(workspace_root, app_id, manifest.scope.as_str());
    let compile_epoch = format!(
        "{}|{}|{}",
        mei_lang_kernel::scene_payload_cache_epoch(),
        mei_lang_kernel::dataset_materialize_cache_epoch(),
        target_file
    );
    ClientBootstrapScopePayload {
        client_revision: manifest.client_revision.clone(),
        bootstrap_scope: manifest.scope.clone(),
        target_file,
        compile_epoch,
        metrics: manifest.metrics.clone(),
    }
}

fn resolve_target_file_for_scope(workspace_root: &Path, app_id: &str, scope: &str) -> String {
    let app_root = mei_lang_kernel::resolve_app_root(workspace_root, app_id);
    let registry = crate::mcg::registry::McgRegistryWriter::load(workspace_root, app_id);
    if let Ok(routes) = crate::assemble::list_scope_routes(workspace_root, app_id) {
        if let Some(route) = routes.into_iter().find(|route| route.scene_id == scope) {
            return crate::assemble::assembly_target_for_key(
                app_root.as_path(),
                &registry,
                route.assembly_key.as_str(),
            );
        }
    }
    if let Some(node) = registry
        .nodes_of_kind(crate::types::GraphNodeKind::AssemblyView)
        .into_iter()
        .find(|node| {
            node.id.key.contains(&format!("#{scope}")) || node.id.key.contains(&format!("{scope}@"))
        })
    {
        return crate::assemble::assembly_target_for_node(app_root.as_path(), node);
    }
    if let Some(assembly_key) =
        crate::assemble::find_assembly_key_by_scene(app_root.as_path(), &registry, scope)
    {
        return crate::assemble::assembly_target_for_key(
            app_root.as_path(),
            &registry,
            assembly_key.as_str(),
        );
    }
    format!("src/scene/{scope}/assembly.mei")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mrg::registry::{MrgLastEval, MrgRegistry, MrgSlotId, MrgSlotRecord};
    use crate::types::{GraphNodeId, GraphNodeKind, PayloadRef};
    use mei_lang_kernel::MetricShape;

    fn sample_slot(state: MaterialState, slot_key: &str, content_hash: &str) -> MrgSlotRecord {
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
    fn bootstrap_embed_status_rejects_stale_client_slots() {
        let manifest = ClientBootstrapManifest {
            schema_version: "mei-client-bootstrap-v1".to_string(),
            scope: "home".to_string(),
            client_revision: "rev".to_string(),
            workset_id: "workset:home:0".to_string(),
            metrics: vec![ClientBootstrapMetric {
                id: "metric_a".to_string(),
                dataset_id: None,
                total_rows: None,
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
        let status = bootstrap_embed_status_for_manifest(&registry, &manifest, "gen-1");
        assert!(!status.allowed);
        assert_eq!(status.reason, "slots_not_ready");
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
                total_rows: None,
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
                total_rows: None,
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
        assert!(bootstrap_embed_allowed(
            &registry,
            &manifest,
            data_generation
        ));
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
            &BTreeMap::from([("metric_a".to_string(), 42usize)]),
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
        assert!(bootstrap_embed_allowed(
            &registry,
            &manifest,
            data_generation.as_str()
        ));
        assert_eq!(
            manifest.metrics[0].dataset_id.as_deref(),
            Some("__world_metrics__::metrics/demo.bundle.mei")
        );
        assert_eq!(manifest.metrics[0].total_rows, Some(42));
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
            &BTreeMap::new(),
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
