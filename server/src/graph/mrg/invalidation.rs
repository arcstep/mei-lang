use std::collections::BTreeMap;

use crate::graph::mrg::registry::MrgRegistry;

#[derive(Debug, Clone, Default)]
pub struct InvalidationOutcome {
    pub slots_marked_stale: usize,
    pub scene_only_skip: bool,
}

/// Apply bridge invalidation policies after MCG update.
pub fn apply_mcg_invalidation(
    mrg: &mut MrgRegistry,
    bridge: &crate::graph::bridge::BridgeExport,
    scene_only_bump: bool,
    changed_bundle_owners: &[String],
) -> InvalidationOutcome {
    if scene_only_bump && changed_bundle_owners.is_empty() {
        let scene_policy = bridge
            .invalidation_policies
            .iter()
            .find(|policy| policy.mcg_kind == "scene_payload");
        if scene_policy.is_some_and(|policy| !policy.mrg_propagate) {
            return InvalidationOutcome {
                scene_only_skip: true,
                ..Default::default()
            };
        }
    }
    let mut outcome = InvalidationOutcome::default();
    let bundle_policy = bridge
        .invalidation_policies
        .iter()
        .find(|policy| policy.mcg_kind == "metric_def_bundle");
    if bundle_policy.is_some_and(|policy| !policy.mrg_propagate) {
        return outcome;
    }
    for owner_id in changed_bundle_owners {
        mrg.mark_owner_slots_stale(owner_id.as_str(), "stale");
        outcome.slots_marked_stale += 1;
    }
    outcome
}

pub fn changed_bundle_owners(
    previous: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut changed = Vec::new();
    for (owner, revision) in current {
        match previous.get(owner) {
            Some(prev) if prev == revision => {}
            _ => changed.push(owner.clone()),
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::types::MaterialState;
    use crate::graph::mrg::registry::{MrgRegistry, MrgSlotId, MrgSlotRecord};
    use crate::graph::types::{GraphNodeId, GraphNodeKind};

    #[test]
    fn scene_only_does_not_stale_slots() {
        let mut mrg = MrgRegistry::empty("zhifa");
        mrg.slots.push(MrgSlotRecord {
            slot_id: MrgSlotId {
                node: GraphNodeId::new(GraphNodeKind::MaterialSlot, "m1"),
                scope_key: "default".to_string(),
            },
            slot_revision: "sr:1".to_string(),
            state: MaterialState::Ready,
            owner_resource_id: "ds1".to_string(),
            metric_def_bundle_revision: "mdb:1".to_string(),
            data_source_revision: String::new(),
            payload_ref: None,
            cache_policy: "artifact_sealed".to_string(),
            eval_engine: "json_walk".to_string(),
            last_eval: None,
        });
        let bridge = crate::graph::bridge::export_bridge("zhifa", &BTreeMap::new());
        let outcome = apply_mcg_invalidation(&mut mrg, &bridge, true, &[]);
        assert!(outcome.scene_only_skip);
        assert_eq!(mrg.slots[0].state, MaterialState::Ready);
    }
}
