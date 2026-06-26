
use mei_lang_kernel::RuntimeState;

use crate::types::{
    ResourceQueryToolSpec, WorldRuntimeBundle,
};

use super::super::bundle::normalize_path;
pub fn recent_trace_messages(state: &RuntimeState, trace_limit: usize) -> Vec<String> {
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

pub(super) fn build_prompt_catalog_lines(
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

