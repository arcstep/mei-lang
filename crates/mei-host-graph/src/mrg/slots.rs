use std::collections::BTreeSet;
use std::path::Path;

use mei_host_core::{CacheLayersReady, EvalSlotDescriptor};

use crate::content_store::METRIC_RESPONSE;
use crate::mrg::registry::{MrgLastEval, MrgRegistryWriter, MrgSlotId, MrgSlotRecord};
use crate::mrg::tier::compute_client_revision;
use crate::types::{
    current_time_ms, stable_hash, GraphNodeId, GraphNodeKind, MaterialState, PayloadRef,
};

pub fn record_slot_from_descriptor(
    source_root: &Path,
    app_id: &str,
    descriptor: &EvalSlotDescriptor,
) -> anyhow::Result<()> {
    record_mrg_slot_from_descriptor(source_root, app_id, descriptor)
}

pub fn record_slots_from_descriptors(
    source_root: &Path,
    app_id: &str,
    descriptors: &[EvalSlotDescriptor],
) -> anyhow::Result<()> {
    if descriptors.is_empty() {
        let mut registry = MrgRegistryWriter::load(source_root, app_id);
        registry.finalize();
        MrgRegistryWriter::save(source_root, &registry)?;
        return Ok(());
    }
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    for descriptor in descriptors {
        let record = build_slot_record(descriptor);
        registry.upsert_slot(record);
    }
    prune_redundant_jit_slots(&mut registry);
    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)
}

fn record_mrg_slot_from_descriptor(
    source_root: &Path,
    app_id: &str,
    descriptor: &EvalSlotDescriptor,
) -> anyhow::Result<()> {
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    registry.upsert_slot(build_slot_record(descriptor));
    prune_redundant_jit_slots(&mut registry);
    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)
}

fn build_slot_record(descriptor: &EvalSlotDescriptor) -> MrgSlotRecord {
    let slot_revision = compute_slot_revision(
        descriptor.metric_def_bundle_revision.as_str(),
        descriptor.data_source_revision.as_str(),
        descriptor.scope_key.as_str(),
        "json_walk",
    );
    let client_revision = descriptor.client_revision.clone().or_else(|| {
        Some(compute_client_revision(
            slot_revision.as_str(),
            descriptor.content_hash.as_str(),
            descriptor.data_source_revision.as_str(),
        ))
    });
    let cache_layer = if descriptor.cache_layer.is_empty() {
        if descriptor.artifact_hit {
            "disk".to_string()
        } else {
            "compute".to_string()
        }
    } else {
        descriptor.cache_layer.clone()
    };
    MrgSlotRecord {
        slot_id: MrgSlotId {
            node: GraphNodeId::new(GraphNodeKind::MaterialSlot, descriptor.slot_key.clone()),
            scope_key: descriptor.scope_key.clone(),
        },
        slot_revision,
        state: MaterialState::Ready,
        owner_resource_id: descriptor.owner_resource_id.clone(),
        metric_def_bundle_revision: descriptor.metric_def_bundle_revision.clone(),
        data_source_revision: descriptor.data_source_revision.clone(),
        payload_ref: Some(PayloadRef::new(
            descriptor.payload_kind.as_str(),
            descriptor.content_hash.as_str(),
            descriptor.schema_version.as_str(),
        )),
        cache_policy: "artifact_sealed".to_string(),
        eval_engine: "json_walk".to_string(),
        last_eval: Some(MrgLastEval {
            at_ms: current_time_ms(),
            wall_ms: descriptor.wall_ms,
            artifact_hit: descriptor.artifact_hit,
            cache_layer,
        }),
        resident_tier: if descriptor.resident_tier.is_empty() {
            if descriptor.cache_layers_ready.memory {
                "memory_resident".to_string()
            } else {
                "disk_only".to_string()
            }
        } else {
            descriptor.resident_tier.clone()
        },
        client_eligible: descriptor.client_eligible,
        client_revision,
        payload_bytes: descriptor.payload_bytes,
        tiers_ready: Some(descriptor.cache_layers_ready.clone()),
        access_count: 0,
        last_access_ms: None,
        workset_id: if descriptor.workset_id.is_empty() {
            None
        } else {
            Some(descriptor.workset_id.clone())
        },
    }
}

fn prune_redundant_jit_slots(registry: &mut crate::mrg::registry::MrgRegistry) {
    let canonical_slots = registry
        .slots
        .iter()
        .filter(|slot| {
            slot.state == MaterialState::Ready
                && !slot
                    .workset_id
                    .as_deref()
                    .is_some_and(|workset| workset.starts_with("jit:"))
        })
        .filter_map(|slot| {
            let metric_id = slot.slot_id.node.key.rsplit("::").next()?.to_string();
            Some((
                slot.slot_id.scope_key.clone(),
                slot.owner_resource_id.clone(),
                metric_id,
                slot.slot_revision.clone(),
            ))
        })
        .collect::<BTreeSet<_>>();
    registry.slots.retain(|slot| {
        if !slot
            .workset_id
            .as_deref()
            .is_some_and(|workset| workset.starts_with("jit:"))
        {
            return true;
        }
        let Some(metric_id) = slot.slot_id.node.key.rsplit("::").next() else {
            return true;
        };
        !canonical_slots.contains(&(
            slot.slot_id.scope_key.clone(),
            slot.owner_resource_id.clone(),
            metric_id.to_string(),
            slot.slot_revision.clone(),
        ))
    });
}

pub fn record_slot_failed(
    source_root: &Path,
    app_id: &str,
    slot_key: &str,
    scope_key: &str,
    owner_resource_id: &str,
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    error_message: &str,
) -> anyhow::Result<()> {
    let slot_revision = compute_slot_revision(
        metric_def_bundle_revision,
        data_source_revision,
        scope_key,
        "json_walk",
    );
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    registry.upsert_slot(MrgSlotRecord {
        slot_id: MrgSlotId {
            node: GraphNodeId::new(GraphNodeKind::MaterialSlot, slot_key.to_string()),
            scope_key: scope_key.to_string(),
        },
        slot_revision,
        state: MaterialState::Failed,
        owner_resource_id: owner_resource_id.to_string(),
        metric_def_bundle_revision: metric_def_bundle_revision.to_string(),
        data_source_revision: data_source_revision.to_string(),
        payload_ref: None,
        cache_policy: "artifact_sealed".to_string(),
        eval_engine: "json_walk".to_string(),
        last_eval: Some(MrgLastEval {
            at_ms: current_time_ms(),
            wall_ms: 0,
            artifact_hit: false,
            cache_layer: format!("failed:{error_message}"),
        }),
        resident_tier: "disk_only".to_string(),
        client_eligible: false,
        client_revision: None,
        payload_bytes: None,
        tiers_ready: None,
        access_count: 0,
        last_access_ms: None,
        workset_id: None,
    });
    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)
}

pub fn mark_slots_stale_for_bundles(
    source_root: &Path,
    app_id: &str,
    bundle_owner_ids: &[String],
) -> anyhow::Result<usize> {
    let mut registry = MrgRegistryWriter::load(source_root, app_id);
    let mut count = 0usize;
    for slot in registry.slots.iter_mut() {
        if bundle_owner_ids
            .iter()
            .any(|owner| slot.owner_resource_id == *owner)
        {
            slot.state = MaterialState::Stale;
            count += 1;
        }
    }
    if count > 0 {
        registry.finalize();
        MrgRegistryWriter::save(source_root, &registry)?;
    }
    Ok(count)
}

fn compute_slot_revision(
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    scope_key: &str,
    eval_engine: &str,
) -> String {
    stable_hash(&format!(
        "{metric_def_bundle_revision}\n{data_source_revision}\n{scope_key}\n{eval_engine}"
    ))
}

pub fn default_metric_response_descriptor(
    slot_key: &str,
    scope_key: &str,
    owner_resource_id: &str,
    bundle_revision: &str,
    data_source_revision: &str,
    content_hash: &str,
    wall_ms: u64,
    artifact_hit: bool,
) -> EvalSlotDescriptor {
    EvalSlotDescriptor {
        slot_key: slot_key.to_string(),
        scope_key: scope_key.to_string(),
        owner_resource_id: owner_resource_id.to_string(),
        metric_def_bundle_revision: bundle_revision.to_string(),
        data_source_revision: data_source_revision.to_string(),
        payload_kind: METRIC_RESPONSE.to_string(),
        content_hash: content_hash.to_string(),
        schema_version: "mei-metric-response-result-artifact-v1".to_string(),
        wall_ms,
        artifact_hit,
        workset_id: String::new(),
        cache_layer: String::new(),
        cache_layers_ready: CacheLayersReady::default(),
        client_revision: None,
        resident_tier: String::new(),
        client_eligible: false,
        payload_bytes: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mrg::registry::MrgRegistry;

    fn descriptor(workset: &str, slot_key: &str, content_hash: &str) -> EvalSlotDescriptor {
        let mut descriptor = default_metric_response_descriptor(
            slot_key,
            "home",
            "__world_metrics__::metrics/demo.bundle.mei",
            "demo.bundle.mei",
            "data-rev-1",
            content_hash,
            1,
            true,
        );
        descriptor.workset_id = workset.to_string();
        descriptor
    }

    #[test]
    fn canonical_warmup_slot_prunes_equivalent_jit_slot() {
        let mut registry = MrgRegistry::empty("demo");
        registry.upsert_slot(build_slot_record(&descriptor(
            "jit:home:demo",
            "jit:home:demo::count",
            "target-specific-key",
        )));
        registry.upsert_slot(build_slot_record(&descriptor(
            "workset:home:0",
            "workset:home:0::count",
            "canonical-key",
        )));

        prune_redundant_jit_slots(&mut registry);

        assert_eq!(registry.slots.len(), 1);
        assert_eq!(
            registry.slots[0].workset_id.as_deref(),
            Some("workset:home:0")
        );
    }

    #[test]
    fn jit_slot_with_distinct_revision_is_retained() {
        let mut registry = MrgRegistry::empty("demo");
        registry.upsert_slot(build_slot_record(&descriptor(
            "workset:home:0",
            "workset:home:0::count",
            "canonical-key",
        )));
        let mut jit = descriptor(
            "jit:home:demo",
            "jit:home:demo::count",
            "target-specific-key",
        );
        jit.data_source_revision = "data-rev-2".to_string();
        registry.upsert_slot(build_slot_record(&jit));

        prune_redundant_jit_slots(&mut registry);

        assert_eq!(registry.slots.len(), 2);
    }
}
