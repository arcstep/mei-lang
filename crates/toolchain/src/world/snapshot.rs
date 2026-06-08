use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::RuntimeState;

use crate::types::{
    ResourceQueryToolSpec, WorldContextSnapshot, WorldRuntimeBundle, WorldRuntimeSummary,
    WorldScope, WorldSnapshotSummary,
};

use super::bundle::{load_world_runtime_bundle, normalize_path};
use super::inventory::build_resource_inventory;

pub(crate) fn recent_trace_messages(state: &RuntimeState, trace_limit: usize) -> Vec<String> {
    state
        .trace_events
        .iter()
        .rev()
        .take(trace_limit)
        .map(|item| item.message.clone())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn build_prompt_catalog_lines(
    app_id: &str,
    bundle: &WorldRuntimeBundle,
    query_tools: &[ResourceQueryToolSpec],
) -> Vec<String> {
    const MAX_RESOURCES: usize = 40;
    const MAX_METRIC_IDS_PER_RESOURCE: usize = 12;
    let mut lines = Vec::new();
    lines.push("[World — catalog]".to_string());
    lines.push(format!("app_id: {app_id}"));
    lines.push(format!("scene_id: {}", bundle.contract.scene.id));
    lines.push(format!("target_file: {}", bundle.active_target_file));
    let world_id = bundle
        .contract
        .world
        .as_ref()
        .and_then(|item| item.id.as_deref())
        .unwrap_or("unknown");
    let world_resource_count = bundle
        .contract
        .world
        .as_ref()
        .map(|item| item.resources.len())
        .unwrap_or(0);
    let world_entity_count = bundle
        .contract
        .world
        .as_ref()
        .map(|item| item.entities.len())
        .unwrap_or(0);
    lines.push(format!(
        "world: id={world_id} resources={} entities={}",
        world_resource_count, world_entity_count
    ));
    lines.push("[World — resources]".to_string());
    if let Some(world) = &bundle.contract.world {
        for item in world.resources.iter().take(MAX_RESOURCES) {
            let source = item
                .source
                .as_ref()
                .map(|source| normalize_path(&source.path))
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| "-".to_string());
            let mut row = format!(
                "- id={} kind={} dataset={} source={}",
                item.id,
                item.kind,
                if item.dataset.is_some() { "yes" } else { "no" },
                source
            );
            if let Some(metrics) = item.metrics.as_ref() {
                if !metrics.is_empty() {
                    let metric_ids = metrics
                        .keys()
                        .take(MAX_METRIC_IDS_PER_RESOURCE)
                        .cloned()
                        .collect::<Vec<_>>();
                    row.push_str(&format!(" metric_ids={}", metric_ids.join(",")));
                }
            }
            lines.push(row);
        }
        if world.resources.len() > MAX_RESOURCES {
            lines.push(format!(
                "- ... {} more resources omitted",
                world.resources.len() - MAX_RESOURCES
            ));
        }
    } else {
        lines.push("- (no world resources)".to_string());
    }
    lines.push("[World — runtime]".to_string());
    lines.push(format!(
        "phase={} result={} actions={}",
        bundle.state.phase,
        bundle.state.result,
        bundle.scene_view.available_actions.join(", ")
    ));
    lines.push("[World — query tooling]".to_string());
    if query_tools.is_empty() {
        lines.push("- (none)".to_string());
    } else {
        for tool in query_tools {
            lines.push(format!(
                "- {}: {} | input={} | output={}",
                tool.id, tool.purpose, tool.input, tool.output
            ));
        }
    }
    lines
}

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

    let query_tools = super::default_resource_query_tools();
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

