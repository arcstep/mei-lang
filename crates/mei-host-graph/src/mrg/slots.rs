use std::path::Path;

use mei_host_core::EvalSlotDescriptor;

use crate::content_store::METRIC_RESPONSE;
use crate::mrg::registry::{MrgLastEval, MrgRegistryWriter, MrgSlotId, MrgSlotRecord};
use crate::types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef, current_time_ms, stable_hash};

pub fn record_slot_from_descriptor(
    source_root: &Path,
    app_id: &str,
    descriptor: &EvalSlotDescriptor,
) -> anyhow::Result<()> {
    record_mrg_slot(
        source_root,
        app_id,
        &descriptor.slot_key,
        &descriptor.scope_key,
        &descriptor.owner_resource_id,
        &descriptor.metric_def_bundle_revision,
        &descriptor.data_source_revision,
        &descriptor.payload_kind,
        &descriptor.content_hash,
        &descriptor.schema_version,
        descriptor.wall_ms,
        descriptor.artifact_hit,
    )
}

pub fn record_slots_from_descriptors(
    source_root: &Path,
    app_id: &str,
    descriptors: &[EvalSlotDescriptor],
) -> anyhow::Result<()> {
    for descriptor in descriptors {
        record_slot_from_descriptor(source_root, app_id, descriptor)?;
    }
    if descriptors.is_empty() {
        let mut registry = MrgRegistryWriter::load(source_root, app_id);
        registry.finalize();
        MrgRegistryWriter::save(source_root, &registry)?;
    }
    Ok(())
}

fn record_mrg_slot(
    source_root: &Path,
    app_id: &str,
    slot_node_key: &str,
    scope_key: &str,
    owner_resource_id: &str,
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    payload_kind: &str,
    content_hash: &str,
    schema_version: &str,
    wall_ms: u64,
    artifact_hit: bool,
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
            node: GraphNodeId::new(GraphNodeKind::MaterialSlot, slot_node_key.to_string()),
            scope_key: scope_key.to_string(),
        },
        slot_revision,
        state: MaterialState::Ready,
        owner_resource_id: owner_resource_id.to_string(),
        metric_def_bundle_revision: metric_def_bundle_revision.to_string(),
        data_source_revision: data_source_revision.to_string(),
        payload_ref: Some(PayloadRef::new(
            payload_kind,
            content_hash,
            schema_version,
        )),
        cache_policy: "artifact_sealed".to_string(),
        eval_engine: "json_walk".to_string(),
        last_eval: Some(MrgLastEval {
            at_ms: current_time_ms(),
            wall_ms,
            artifact_hit,
            cache_layer: if artifact_hit { "disk" } else { "compute" }.to_string(),
        }),
    });
    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)
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
    }
}
