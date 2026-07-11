//! MCG/MRG dedup helpers — always read disk registry when dedup is enabled.

use std::collections::BTreeMap;
use std::path::Path;

use crate::graph::feature::graph_registry_dedup_enabled;
use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::mrg::registry::{MrgRegistry, MrgRegistryWriter};
use crate::graph::mrg::slot_revision::compute_slot_revision;
use crate::graph::types::{GraphNodeKind, MaterialState};

pub fn mrg_eval_scope_key(scene_id: &str, scene_path: Option<&str>) -> String {
    let scene_id = scene_id.trim();
    if let Some(path) = scene_path.map(str::trim).filter(|value| !value.is_empty()) {
        if scene_id.is_empty() {
            path.to_string()
        } else {
            format!("{scene_id}@{path}")
        }
    } else if scene_id.is_empty() {
        "default".to_string()
    } else {
        scene_id.to_string()
    }
}

pub fn load_mcg_bundle_revisions(source_root: &Path, app_id: &str) -> BTreeMap<String, String> {
    if !graph_registry_dedup_enabled() {
        return BTreeMap::new();
    }
    McgRegistryWriter::load(source_root, app_id)
        .nodes
        .into_iter()
        .filter(|node| node.id.kind == GraphNodeKind::MetricDefBundle)
        .map(|node| (node.id.key, node.revision))
        .collect()
}

pub fn load_mrg_registry(source_root: &Path, app_id: &str) -> MrgRegistry {
    if !graph_registry_dedup_enabled() {
        return MrgRegistry::empty(app_id);
    }
    MrgRegistryWriter::load(source_root, app_id)
}

pub fn metric_bundle_revision_unchanged(
    pre_revisions: &BTreeMap<String, String>,
    owner_resource_id: &str,
    current_revision: &str,
) -> bool {
    pre_revisions
        .get(owner_resource_id)
        .is_some_and(|prev| prev == current_revision)
}

/// Read MetricDefBundle revision from MCG registry (1.3.0 canonical source).
pub fn mcg_metric_bundle_revision(
    mcg_revisions: &BTreeMap<String, String>,
    owner_resource_id: &str,
) -> Option<String> {
    mcg_revisions
        .get(owner_resource_id)
        .cloned()
        .filter(|rev| !rev.trim().is_empty())
}

static LOCAL_BUNDLE_REV_WARNED: std::sync::OnceLock<std::sync::Mutex<BTreeMap<String, bool>>> =
    std::sync::OnceLock::new();

/// Resolve bundle revision: MCG first, local defs hash fallback (warn once per owner).
pub fn resolve_metric_bundle_revision(
    mcg_revisions: &BTreeMap<String, String>,
    owner_resource_id: &str,
    defs_for_hydrate: &BTreeMap<String, serde_json::Value>,
) -> Option<String> {
    if let Some(rev) = mcg_metric_bundle_revision(mcg_revisions, owner_resource_id) {
        return Some(rev);
    }
    if defs_for_hydrate.is_empty() {
        return None;
    }
    let warned = LOCAL_BUNDLE_REV_WARNED.get_or_init(|| std::sync::Mutex::new(BTreeMap::new()));
    let mut guard = warned.lock().ok()?;
    if !guard.contains_key(owner_resource_id) {
        tracing::warn!(
            owner = %owner_resource_id,
            "MCG MetricDefBundle revision missing; falling back to local defs hash"
        );
        guard.insert(owner_resource_id.to_string(), true);
    }
    let serialized = serde_json::to_string(defs_for_hydrate).ok()?;
    Some(format!(
        "mdb:{}",
        crate::graph::types::stable_hash(&serialized)
    ))
}

/// Canonical MRG slot cache key for a metric workset plan.
pub fn canonical_slot_cache_key_for_workset(
    owner_resource_id: &str,
    scene_id: &str,
    scene_path: Option<&str>,
    bundle_revision: &str,
    dependency_revision_key: &str,
) -> String {
    let scope_key = mrg_eval_scope_key(scene_id, scene_path);
    let slot_revision = compute_slot_revision(
        bundle_revision,
        dependency_revision_key,
        scope_key.as_str(),
        "json_walk",
    );
    crate::graph::mrg::cache_key::slot_cache_key(
        owner_resource_id,
        scope_key.as_str(),
        slot_revision.as_str(),
        dependency_revision_key,
    )
}

/// MRG slot covers eval when bundle revision matches and artifact cache key is recorded.
pub fn mrg_slot_covers_eval(
    registry: &MrgRegistry,
    owner_resource_id: &str,
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    scope_key: &str,
    cache_key: &str,
) -> bool {
    mrg_slot_covers_payload(
        registry,
        owner_resource_id,
        metric_def_bundle_revision,
        data_source_revision,
        scope_key,
        cache_key,
        "metric_response",
    )
}

/// MRG slot covers dataframe eval when shared artifact key matches.
pub fn mrg_slot_covers_dataframe_eval(
    registry: &MrgRegistry,
    owner_resource_id: &str,
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    scope_key: &str,
    shared_artifact_key: &str,
) -> bool {
    mrg_slot_covers_payload(
        registry,
        owner_resource_id,
        metric_def_bundle_revision,
        data_source_revision,
        scope_key,
        shared_artifact_key,
        "metric_dataframe",
    )
}

fn mrg_slot_covers_payload(
    registry: &MrgRegistry,
    owner_resource_id: &str,
    metric_def_bundle_revision: &str,
    data_source_revision: &str,
    scope_key: &str,
    cache_key: &str,
    payload_kind: &str,
) -> bool {
    let expected_revision = compute_slot_revision(
        metric_def_bundle_revision,
        data_source_revision,
        scope_key,
        "json_walk",
    );
    let canonical_key = crate::graph::mrg::cache_key::slot_cache_key(
        owner_resource_id,
        scope_key,
        expected_revision.as_str(),
        cache_key,
    );
    registry.slots.iter().any(|slot| {
        slot.state == MaterialState::Ready
            && slot.owner_resource_id == owner_resource_id
            && slot.metric_def_bundle_revision == metric_def_bundle_revision
            && slot.data_source_revision == data_source_revision
            && slot.slot_revision == expected_revision
            && slot
                .payload_ref
                .as_ref()
                .is_some_and(|payload| payload.kind == payload_kind)
            && slot.payload_ref.as_ref().is_some_and(|payload| {
                payload.content_hash == cache_key || payload.content_hash == canonical_key
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::mrg::registry::{MrgSlotId, MrgSlotRecord};
    use crate::graph::types::GraphNodeId;

    #[test]
    fn mrg_slot_covers_eval_when_revision_and_key_match() {
        let bundle_rev = "mdb:abc";
        let ds_rev = "ds:parquet1";
        let cache_key = "shared-key";
        let slot_revision = compute_slot_revision(bundle_rev, ds_rev, "default", "json_walk");
        let registry = MrgRegistry {
            schema_version: "test".to_string(),
            app_id: "demo".to_string(),
            registry_revision: String::new(),
            updated_at_ms: 0,
            nodes: Vec::new(),
            slots: vec![MrgSlotRecord {
                slot_id: MrgSlotId {
                    node: GraphNodeId::new(GraphNodeKind::MaterialSlot, "m1".to_string()),
                    scope_key: "default".to_string(),
                },
                slot_revision,
                state: MaterialState::Ready,
                owner_resource_id: "owner1".to_string(),
                metric_def_bundle_revision: bundle_rev.to_string(),
                data_source_revision: ds_rev.to_string(),
                payload_ref: Some(crate::graph::types::PayloadRef::new(
                    "metric_response",
                    cache_key,
                    "v1",
                )),
                cache_policy: "artifact_sealed".to_string(),
                eval_engine: "json_walk".to_string(),
                last_eval: None,
            }],
            edges: Vec::new(),
        };
        assert!(mrg_slot_covers_eval(
            &registry, "owner1", bundle_rev, ds_rev, "default", cache_key,
        ));
    }
}
