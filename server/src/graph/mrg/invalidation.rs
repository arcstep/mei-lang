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
    scene_only_bump: bool,
    changed_bundle_owners: &[String],
) -> InvalidationOutcome {
    if scene_only_bump && changed_bundle_owners.is_empty() {
        return InvalidationOutcome {
            scene_only_skip: true,
            ..Default::default()
        };
    }
    let mut outcome = InvalidationOutcome::default();
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
    use crate::graph::MaterialState;
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
        let outcome = apply_mcg_invalidation(&mut mrg, true, &[]);
        assert!(outcome.scene_only_skip);
        assert_eq!(mrg.slots[0].state, MaterialState::Ready);
    }
}
