use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::types::{
    WorldContextSnapshot,
    WorldRuntimeSummary, WorldScope, WorldSnapshotSummary,
};

use super::super::bundle::load_world_runtime_bundle;
use super::super::inventory::build_resource_inventory;
use super::catalog_lines::*;

pub fn build_world_context_snapshot(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldContextSnapshot> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let resource_inventory = build_resource_inventory(source_root, app_id, &bundle, scope);
    let world = bundle.contract.world.clone();

    let (resource_kind_counts, key_resource_ids, world_resource_count) = if let Some(world) = &world
    {
        let mut counts = BTreeMap::new();
        let mut ids = Vec::new();
        for item in &world.resources {
            *counts.entry(item.kind.clone()).or_insert(0) += 1;
            if ids.len() < 20 {
                ids.push(item.id.clone());
            }
        }
        (counts, ids, world.resources.len())
    } else {
        (BTreeMap::new(), Vec::new(), 0)
    };

    let (entity_kind_counts, key_entity_ids, world_entity_count) = if let Some(world) = &world {
        let mut counts = BTreeMap::new();
        let mut ids = Vec::new();
        for item in &world.entities {
            *counts.entry(item.kind.clone()).or_insert(0) += 1;
            if ids.len() < 20 {
                ids.push(item.id.clone());
            }
        }
        (counts, ids, world.entities.len())
    } else {
        (BTreeMap::new(), Vec::new(), 0)
    };

    let world_topology = world.as_ref().and_then(|item| {
        item.topology.as_ref().map(|topology| {
            format!(
                "grid(rows={}, cols={}, cells={})",
                topology.rows,
                topology.cols,
                topology.cells.len()
            )
        })
    });

    let query_tools = super::super::default_resource_query_tools();
    let prompt_catalog_lines = build_prompt_catalog_lines(app_id, &bundle, &query_tools);

    Ok(WorldContextSnapshot {
        app_id: app_id.to_string(),
        active_target_file: bundle.active_target_file,
        world_snapshot: WorldSnapshotSummary {
            scene_id: bundle.contract.scene.id.clone(),
            world_id: world.as_ref().and_then(|item| item.id.clone()),
            world_resource_count,
            world_entity_count,
            world_topology,
            world_resource_kind_counts: resource_kind_counts,
            world_entity_kind_counts: entity_kind_counts,
            world_key_resource_ids: key_resource_ids,
            world_key_entity_ids: key_entity_ids,
        },
        resource_inventory,
        runtime_summary: WorldRuntimeSummary {
            phase: bundle.state.phase.clone(),
            result: bundle.state.result.clone(),
            countdown: bundle.state.countdown,
            scene_view_entities: bundle.scene_view.entities.len(),
            scene_view_cells: bundle.scene_view.cells.len(),
            available_actions: bundle
                .scene_view
                .available_actions
                .iter()
                .take(20)
                .cloned()
                .collect(),
            recent_trace_messages: recent_trace_messages(&bundle.state, 5),
        },
        query_tools,
        prompt_catalog_lines,
    })
}
