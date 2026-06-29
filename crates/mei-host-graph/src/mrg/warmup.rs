use std::collections::BTreeSet;

use crate::mrg::registry::{MrgEdgeRecord, MrgRegistry};
use crate::types::GraphNodeKind;

#[derive(Debug, Clone, Default)]
pub struct WarmupFrontierOutcome {
    pub scheduled_slots: Vec<String>,
    pub navigation_edges_added: usize,
}

pub fn record_navigation_edge(mrg: &mut MrgRegistry, from_scene: &str, to_scene: &str) -> usize {
    let from = format!("navigation:{from_scene}");
    let to = format!("navigation:{to_scene}");
    if mrg
        .edges
        .iter()
        .any(|edge| edge.from == from && edge.to == to)
    {
        return 0;
    }
    mrg.upsert_edge(MrgEdgeRecord {
        from,
        to,
        kind: "navigates_to".to_string(),
    });
    1
}

pub fn warm_frontier_slots(
    mrg: &MrgRegistry,
    node_key: &str,
    k_hops: usize,
) -> WarmupFrontierOutcome {
    let mut visited = BTreeSet::new();
    let mut frontier = vec![node_key.to_string()];
    let mut scheduled = Vec::new();
    for _ in 0..=k_hops {
        let mut next = Vec::new();
        for key in frontier {
            if !visited.insert(key.clone()) {
                continue;
            }
            for slot in &mrg.slots {
                if slot.slot_id.node.key.contains(key.as_str())
                    && matches!(
                        slot.state,
                        crate::types::MaterialState::Missing | crate::types::MaterialState::Stale
                    )
                {
                    scheduled.push(slot.slot_id.node.stable_key());
                }
            }
            for edge in &mrg.edges {
                if edge.from.ends_with(&key) && edge.kind == "navigates_to" {
                    next.push(edge.to.clone());
                }
            }
        }
        frontier = next;
    }
    let _ = GraphNodeKind::Navigation;
    WarmupFrontierOutcome {
        scheduled_slots: scheduled,
        navigation_edges_added: 0,
    }
}
