use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::http::scene_api::resource_query::default_resource_query_tools;
use crate::AppState;

use super::analysis_contract_llm::build_analysis_contract_catalog_lines;
use super::bundle::{load_world_runtime_bundle, load_world_runtime_bundle_cached};
use super::inventory::build_resource_inventory;
use super::runtime_peek::recent_trace_messages_for_snapshot;
use super::util::normalize_path;
use crate::http::scene_api::types::{
    ResourceQueryToolSpec, WorldContextSnapshot, WorldRuntimeBundle, WorldRuntimeSummary,
    WorldScope, WorldSnapshotSummary,
};

pub(super) fn build_prompt_catalog_lines(
    bundle: &WorldRuntimeBundle,
    query_tools: &[ResourceQueryToolSpec],
) -> Vec<String> {
    use std::fmt::Write as _;

    let mut lines: Vec<String> = Vec::new();
    lines.push("[World — catalog (highest-priority context)]".to_string());
    lines.push(
        "Below lists bindable world assets for this scope. When a dataset resource id is known or implied (e.g. `typical_cases`), call `dataset_query` for row/schema questions (includes bounded analysis_contracts_preview when explain metrics exist), or `dataset_metric` for aggregated questions plus matching analysis_contract summaries (same source as host UI popup/route)."
            .to_string(),
    );
    lines.push(
        "Tool-chaining guard: do NOT read_file() `.xlsx/.xls` (binary). `dataset_query` returns schema+filters+metric ids+sample rows+analysis_contracts_preview; `dataset_metric` returns metric values+analysis_contracts. When `contract_hint` is present, do not invent explain/popup/drilldown fields. For dataset facts, do NOT chain `read_file` / `resource_list` / `resource_runtime_peek` after a successful dataset tool call unless the user explicitly asks runtime trace or verbatim DSL edits."
            .to_string(),
    );
    lines.push(format!(
        "scene: id={} target_file={}",
        bundle.contract.scene.id, bundle.active_target_file
    ));
    if let Some(world) = &bundle.contract.world {
        if let Some(wid) = world.id.as_deref().filter(|s| !s.is_empty()) {
            lines.push(format!("world.id: {wid}"));
        }
        lines.push(format!(
            "world.resources (count={}):",
            world.resources.len()
        ));
        const MAX_RES: usize = 96;
        for r in world.resources.iter().take(MAX_RES) {
            let src = r
                .source
                .as_ref()
                .map(|s| normalize_path(&s.path))
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| "-".to_string());
            let metric_ids = r
                .metrics
                .as_ref()
                .map(|m| m.keys().cloned().collect::<Vec<_>>().join(","))
                .unwrap_or_default();
            let mstr = if metric_ids.is_empty() {
                "-".to_string()
            } else {
                metric_ids.clone()
            };
            let ds = if r.dataset.is_some() {
                "dataset:yes"
            } else {
                "dataset:no"
            };
            let title = r.title.as_deref().unwrap_or("");
            let tool_hint = if matches!(r.kind.as_str(), "dataset" | "dataset_view") {
                if metric_ids.is_empty() {
                    "tool=dataset_query"
                } else {
                    "tool=dataset_query,dataset_metric"
                }
            } else {
                "tool=none"
            };
            let mut line = String::new();
            let _ = write!(
                &mut line,
                "  - resource id={} kind={} title={} source={} metrics=[{}] {} {}",
                r.id, r.kind, title, src, mstr, ds, tool_hint
            );
            if line.chars().count() > 260 {
                line = line.chars().take(257).collect::<String>();
                line.push('…');
            }
            lines.push(line);
        }
        if world.resources.len() > MAX_RES {
            lines.push(format!(
                "  ... resources_omitted: {} (dataset resources remain queryable by id via dataset_query)",
                world.resources.len() - MAX_RES
            ));
        }

        lines.push(format!("world.entities (count={}):", world.entities.len()));
        const MAX_ENT: usize = 48;
        for e in world.entities.iter().take(MAX_ENT) {
            lines.push(format!(
                "  - entity id={} kind={} label={} status={}",
                e.id,
                e.kind,
                e.label.as_deref().unwrap_or("-"),
                e.status.as_deref().unwrap_or("-")
            ));
        }
        if world.entities.len() > MAX_ENT {
            lines.push(format!(
                "  ... entities_omitted: {}",
                world.entities.len() - MAX_ENT
            ));
        }

        if let Some(top) = &world.topology {
            lines.push(format!(
                "world.topology: grid {}x{} cells={}",
                top.rows,
                top.cols,
                top.cells.len()
            ));
        }
    } else {
        lines.push("world: (none in scene contract)".to_string());
    }

    lines.push(String::new());
    lines.push("[World — query tools (bounded)]".to_string());
    for t in query_tools {
        lines.push(format!("- {} — {}", t.id, t.purpose));
        lines.push(format!("  input: {}", t.input));
        lines.push(format!("  output: {}", t.output));
    }

    lines.push(String::new());
    lines.push("[World — scene routes]".to_string());
    const MAX_ENTRY: usize = 32;
    for e in bundle.compiled.scene_routes.iter().take(MAX_ENTRY) {
        lines.push(format!(
            "  - scene_id={} target_file={} kind={}",
            e.scene_id, e.target_file, e.kind
        ));
    }
    if bundle.compiled.scene_routes.len() > MAX_ENTRY {
        lines.push(format!(
            "  ... routes_omitted: {}",
            bundle.compiled.scene_routes.len() - MAX_ENTRY
        ));
    }

    lines.extend(build_analysis_contract_catalog_lines(bundle));

    lines
}

fn build_world_context_snapshot_from_bundle(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    bundle: WorldRuntimeBundle,
) -> Result<WorldContextSnapshot> {
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

    let recent_trace_messages = recent_trace_messages_for_snapshot(&bundle.state, 5);
    let query_tools = default_resource_query_tools();
    let prompt_catalog_lines = build_prompt_catalog_lines(&bundle, &query_tools);

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
            recent_trace_messages,
        },
        query_tools,
        prompt_catalog_lines,
    })
}

pub(crate) fn build_world_context_snapshot(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldContextSnapshot> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    build_world_context_snapshot_from_bundle(source_root, app_id, scope, bundle)
}

pub(crate) fn build_world_context_snapshot_cached(
    state: &AppState,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldContextSnapshot> {
    let bundle = load_world_runtime_bundle_cached(state, app_id, scope)?;
    build_world_context_snapshot_from_bundle(&state.source_root, app_id, scope, bundle)
}
