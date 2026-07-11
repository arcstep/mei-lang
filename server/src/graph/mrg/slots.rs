use std::path::Path;

use mei_lang_kernel::resolve_app_root;

use crate::graph::content_store::{self, METRIC_DATAFRAME, METRIC_RESPONSE};
use crate::graph::feature::graph_registry_dedup_enabled;
use crate::graph::mrg::registry::{MrgLastEval, MrgRegistryWriter, MrgSlotId, MrgSlotRecord};
use crate::graph::mrg::slot_revision::compute_slot_revision;
use crate::graph::types::{GraphNodeId, GraphNodeKind, MaterialState, PayloadRef};

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
    if !graph_registry_dedup_enabled() {
        return Ok(());
    }
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
        payload_ref: Some(PayloadRef::new(payload_kind, content_hash, schema_version)),
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

/// Resolve slot artifact bytes from Content Store CAS only.
pub fn resolve_slot_payload_path(
    source_root: &Path,
    app_id: &str,
    slot: &MrgSlotRecord,
) -> Option<std::path::PathBuf> {
    let app_root = resolve_app_root(source_root, app_id);
    slot.payload_ref
        .as_ref()
        .and_then(|pref| content_store::resolve_payload_ref(app_root.as_path(), pref))
}

pub fn record_mrg_slot_after_eval(
    source_root: &Path,
    app_id: &str,
    workset_id: &str,
    scope_key: &str,
    owner_resource_id: &str,
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    response_cache_key: &str,
    _artifact_relative_path: &str,
    wall_ms: u64,
    artifact_hit: bool,
) -> anyhow::Result<()> {
    record_mrg_slot(
        source_root,
        app_id,
        workset_id,
        scope_key,
        owner_resource_id,
        metric_def_bundle_revision,
        data_source_revision,
        METRIC_RESPONSE,
        response_cache_key,
        "mei-metric-response-result-artifact-v1",
        wall_ms,
        artifact_hit,
    )
}

pub fn record_mrg_dataframe_slot_after_eval(
    source_root: &Path,
    app_id: &str,
    workset_id: &str,
    scope_key: &str,
    owner_resource_id: &str,
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    shared_artifact_key: &str,
    _artifact_relative_path: &str,
    wall_ms: u64,
    artifact_hit: bool,
) -> anyhow::Result<()> {
    record_mrg_slot(
        source_root,
        app_id,
        workset_id,
        scope_key,
        owner_resource_id,
        metric_def_bundle_revision,
        data_source_revision,
        METRIC_DATAFRAME,
        shared_artifact_key,
        "mei-metric-dataframe-result-artifact-v1",
        wall_ms,
        artifact_hit,
    )
}

pub fn record_mrg_slot_failed(
    source_root: &Path,
    app_id: &str,
    slot_node_key: &str,
    scope_key: &str,
    owner_resource_id: &str,
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    error_message: &str,
) -> anyhow::Result<()> {
    if !graph_registry_dedup_enabled() {
        return Ok(());
    }
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
            cache_layer: error_message.chars().take(32).collect(),
        }),
    });
    registry.finalize();
    MrgRegistryWriter::save(source_root, &registry)
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Prefer MRG registry slot count for cold-start index hint (Phase 2).
pub fn mrg_slot_count(source_root: &Path, app_id: &str) -> usize {
    if !graph_registry_dedup_enabled() {
        return 0;
    }
    MrgRegistryWriter::load(source_root, app_id).slots.len()
}
