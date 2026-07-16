//! Thin re-export: MRG registry schema ownership lives in `mei-host-graph`.

pub use mei_host_graph::{
    MrgEdgeRecord, MrgLastEval, MrgRegistry, MrgRegistryWriter, MrgSlotId, MrgSlotRecord,
    MRG_REGISTRY_SCHEMA_VERSION,
};

use crate::graph::mrg::navigation::types::NavigationEntry;
use crate::graph::types::MaterialState;
use mei_host_graph::MrgNodeRecord;

/// Project typed navigation nodes into the server navigation contract.
pub fn navigation_entries(registry: &MrgRegistry) -> Vec<NavigationEntry> {
    registry
        .nodes
        .iter()
        .filter_map(|node| match node {
            MrgNodeRecord::Navigation {
                id,
                url,
                scene_id,
                target_file,
                state,
            } => Some(NavigationEntry {
                key: id.key.clone(),
                url: url.clone(),
                scene_id: scene_id.clone(),
                target_file: target_file.clone(),
                state: state.clone(),
            }),
            _ => None,
        })
        .collect()
}

pub fn navigation_by_key(registry: &MrgRegistry, key: &str) -> Option<NavigationEntry> {
    navigation_entries(registry)
        .into_iter()
        .find(|entry| entry.key == key)
}

pub fn upsert_navigation_node(
    registry: &mut MrgRegistry,
    key: &str,
    url: &str,
    scene_id: &str,
    target_file: &str,
    state: MaterialState,
) {
    registry.upsert_navigation_node(key, url, scene_id, target_file, state);
}
