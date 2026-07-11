use std::path::Path;

use anyhow::Result;

use crate::graph::load_mrg_registry;
use crate::graph::types::MaterialState;

use super::types::{BlockListEntry, BlockListReport};

pub fn block_list(source_root: &Path, app_id: &str, states: &[&str]) -> Result<BlockListReport> {
    let mrg = load_mrg_registry(source_root, app_id);
    let want_all = states.is_empty();
    let mut blocks = Vec::new();
    for slot in &mrg.slots {
        let state_slug = match slot.state {
            MaterialState::Ready => "ready",
            MaterialState::Stale => "stale",
            MaterialState::Missing => "missing",
            MaterialState::Failed => "failed",
            MaterialState::Warming => "warming",
        };
        if !want_all && !states.contains(&state_slug) {
            continue;
        }
        blocks.push(BlockListEntry {
            block_id: format!(
                "material_slot:{}@{}",
                slot.slot_id.node.key, slot.slot_id.scope_key
            ),
            kind: "material_slot".to_string(),
            key: slot.slot_id.node.key.clone(),
            scope_key: Some(slot.slot_id.scope_key.clone()),
            state: state_slug.to_string(),
            last_error: if slot.state == MaterialState::Failed {
                Some(format!("owner={}", slot.owner_resource_id))
            } else {
                None
            },
        });
    }
    Ok(BlockListReport {
        app_id: app_id.to_string(),
        blocks,
    })
}
