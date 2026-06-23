use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{RuntimeState, Severity};

use crate::semantic_summary::summarize_compiled_app_semantics;
use crate::types::{
    CompiledRouteSummary, ComponentAssetSummary, DiagnosticCountSummary, LoadedResourceSummary,
    ResourceInventoryItem, ResourceQueryToolSpec, WorldBusinessEntitySummary,
    WorldBusinessResourceSummary, WorldBusinessSummary, WorldContextSnapshot, WorldRuntimeBundle,
    WorldRuntimeSummary, WorldScope, WorldSnapshotSummary,
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

fn top_kind_lines(counts: &BTreeMap<String, usize>, label: &str) -> Option<String> {
    if counts.is_empty() {
        return None;
    }
    let top = counts
        .iter()
        .take(4)
        .map(|(kind, count)| format!("{kind}={count}"))
        .collect::<Vec<_>>();
    Some(format!("{label}: {}", top.join(", ")))
}

fn resource_inventory_map<'a>(
    items: &'a [ResourceInventoryItem],
) -> BTreeMap<&'a str, &'a ResourceInventoryItem> {
    let mut out = BTreeMap::new();
    for item in items {
        out.insert(item.id.as_str(), item);
    }
    out
}

fn summarize_diagnostics(bundle: &WorldRuntimeBundle) -> DiagnosticCountSummary {
    let mut summary = DiagnosticCountSummary::default();
    for item in &bundle.compiled.diagnostics {
        match item.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warning => summary.warnings += 1,
            Severity::Info => summary.infos += 1,
        }
    }
    summary
}

pub fn build_world_business_summary(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldBusinessSummary> {
    const MAX_KEY_RESOURCES: usize = 8;
    const MAX_KEY_ENTITIES: usize = 8;
    const MAX_METRIC_IDS: usize = 8;

    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let resource_inventory = build_resource_inventory(source_root, app_id, &bundle, scope);
    let inventory_map = resource_inventory_map(&resource_inventory.items);
    let query_tools = super::default_resource_query_tools();
    let mut resource_kind_counts = BTreeMap::<String, usize>::new();
    let mut entity_kind_counts = BTreeMap::<String, usize>::new();
    let mut loaded_resource_kind_counts = BTreeMap::<String, usize>::new();
    let mut key_resources = Vec::new();
    let mut key_entities = Vec::new();
    let diagnostics = summarize_diagnostics(&bundle);
    let scene_routes = bundle
        .compiled
        .scene_routes
        .iter()
        .take(12)
        .map(|route| CompiledRouteSummary {
            scene_id: route.scene_id.clone(),
            target_file: normalize_path(&route.target_file),
            kind: route.kind.clone(),
            title: route.title.clone(),
            is_default: route.is_default,
            access_export: route.access_export,
        })
        .collect::<Vec<_>>();
    let loaded_resources = bundle
        .compiled
        .resources
        .iter()
        .take(12)
        .map(|item| {
            *loaded_resource_kind_counts
                .entry(item.kind.clone())
                .or_insert(0) += 1;
            LoadedResourceSummary {
                id: item.id.clone(),
                kind: item.kind.clone(),
                title: item.title.clone(),
                has_dataset: item.dataset.is_some(),
                has_document: item.document.is_some(),
            }
        })
        .collect::<Vec<_>>();
    for item in bundle.compiled.resources.iter().skip(12) {
        *loaded_resource_kind_counts
            .entry(item.kind.clone())
            .or_insert(0) += 1;
    }
    let component_assets = bundle
        .compiled
        .component_assets
        .iter()
        .take(12)
        .map(|asset| ComponentAssetSummary {
            key: asset.key.clone(),
            tag: asset.tag.clone(),
            script: normalize_path(&asset.script),
        })
        .collect::<Vec<_>>();
    let default_scene_id = bundle
        .compiled
        .scene_routes
        .iter()
        .find(|route| route.is_default)
        .map(|route| route.scene_id.clone());

    if let Some(world) = &bundle.contract.world {
        for item in &world.resources {
            *resource_kind_counts.entry(item.kind.clone()).or_insert(0) += 1;
            if key_resources.len() >= MAX_KEY_RESOURCES {
                continue;
            }
            let inventory = inventory_map.get(item.id.as_str()).copied();
            let metric_ids = item
                .metrics
                .as_ref()
                .map(|metrics| metrics.keys().take(MAX_METRIC_IDS).cloned().collect())
                .unwrap_or_default();
            key_resources.push(WorldBusinessResourceSummary {
                id: item.id.clone(),
                kind: item.kind.clone(),
                title: item.title.clone(),
                source_path: inventory.and_then(|entry| entry.source_path.clone()),
                metric_ids,
                summary: inventory.and_then(|entry| entry.summary.clone()),
                related_to_target: inventory.is_some_and(|entry| entry.related_to_target),
            });
        }
        for item in &world.entities {
            *entity_kind_counts.entry(item.kind.clone()).or_insert(0) += 1;
            if key_entities.len() >= MAX_KEY_ENTITIES {
                continue;
            }
            key_entities.push(WorldBusinessEntitySummary {
                id: item.id.clone(),
                kind: item.kind.clone(),
                label: item.label.clone(),
                status: item.status.clone(),
            });
        }
    }

    let runtime_summary = WorldRuntimeSummary {
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
    };
    let total_resources = resource_kind_counts.values().sum::<usize>();
    let total_entities = entity_kind_counts.values().sum::<usize>();
    let semantic = summarize_compiled_app_semantics(&bundle.compiled);
    let business_focus = format!(
        "app `{}`（title=`{}`）当前聚焦 scene `{}`，active_target_file=`{}`；routes={} world resources={} entities={}，runtime phase=`{}` result=`{}`。",
        app_id,
        bundle.compiled.title,
        bundle.contract.scene.id,
        bundle.active_target_file,
        bundle.compiled.scene_routes.len(),
        total_resources,
        total_entities,
        runtime_summary.phase,
        runtime_summary.result
    );
    let mut narrative = vec![
        format!("app_title: {}", bundle.compiled.title),
        format!("app_kind: {}", semantic.app_kind),
        format!("scene_id: {}", bundle.contract.scene.id),
        format!("active_target_file: {}", bundle.active_target_file),
    ];
    if let Some(active_scene) = bundle.compiled.active_scene.as_deref() {
        narrative.push(format!("active_scene: {active_scene}"));
    }
    if let Some(default_scene) = default_scene_id.as_deref() {
        narrative.push(format!("default_scene: {default_scene}"));
    }
    if let Some(scene_profile) = semantic.scene_profile.as_deref() {
        narrative.push(format!("scene_profile: {scene_profile}"));
    }
    if let Some(scene_summary) = semantic.scene_summary.as_deref() {
        narrative.push(format!("scene_summary: {scene_summary}"));
    }
    if let Some(scene_goal) = semantic.scene_goal.as_deref() {
        narrative.push(format!("scene_goal: {scene_goal}"));
    }
    if !scene_routes.is_empty() {
        let route_ids = scene_routes
            .iter()
            .take(6)
            .map(|route| route.scene_id.as_str())
            .collect::<Vec<_>>();
        narrative.push(format!(
            "scene_routes: {} (count={})",
            route_ids.join(", "),
            bundle.compiled.scene_routes.len()
        ));
    }
    if let Some(world_id) = bundle
        .contract
        .world
        .as_ref()
        .and_then(|world| world.id.clone())
    {
        narrative.push(format!("world_id: {world_id}"));
    }
    narrative.push(format!("panel_count: {}", semantic.panel_count));
    if let Some(layout_type) = semantic.frame_layout_type.as_deref() {
        narrative.push(format!("frame_layout_type: {layout_type}"));
    }
    if semantic.world_has_topology {
        narrative.push("world_topology: grid".to_string());
    }
    if semantic.flow_interaction_count > 0
        || semantic.flow_subject_timer_count > 0
        || semantic.has_timer
    {
        narrative.push(format!(
            "flow: interactions={} subject_timers={} timer={}",
            semantic.flow_interaction_count, semantic.flow_subject_timer_count, semantic.has_timer
        ));
    }
    if let Some(line) = top_kind_lines(&resource_kind_counts, "resource_kinds") {
        narrative.push(line);
    }
    if let Some(line) = top_kind_lines(&entity_kind_counts, "entity_kinds") {
        narrative.push(line);
    }
    if let Some(line) = top_kind_lines(&loaded_resource_kind_counts, "loaded_resource_kinds") {
        narrative.push(line);
    }
    if !component_assets.is_empty() {
        let tags = component_assets
            .iter()
            .take(6)
            .map(|item| item.tag.as_str())
            .collect::<Vec<_>>();
        narrative.push(format!(
            "component_assets: {} (count={})",
            tags.join(", "),
            bundle.compiled.component_assets.len()
        ));
    }
    if diagnostics.errors > 0 || diagnostics.warnings > 0 || diagnostics.infos > 0 {
        narrative.push(format!(
            "diagnostics: errors={} warnings={} infos={}",
            diagnostics.errors, diagnostics.warnings, diagnostics.infos
        ));
    }
    if !runtime_summary.available_actions.is_empty() {
        narrative.push(format!(
            "available_actions: {}",
            runtime_summary.available_actions.join(", ")
        ));
    }
    if !runtime_summary.recent_trace_messages.is_empty() {
        narrative.push(format!(
            "recent_trace: {}",
            runtime_summary.recent_trace_messages.join(" | ")
        ));
    }
    narrative.push(format!(
        "business_explanation: {}",
        semantic.business_explanation
    ));
    let mut semantic_hints = Vec::new();
    semantic_hints.push(format!(
        "app_kind=`{}`；semantic_tags={}",
        semantic.app_kind,
        semantic.semantic_tags.join(", ")
    ));
    if default_scene_id.as_deref() != bundle.compiled.active_scene.as_deref() {
        if let (Some(default_scene), Some(active_scene)) = (
            default_scene_id.as_deref(),
            bundle.compiled.active_scene.as_deref(),
        ) {
            semantic_hints.push(format!(
                "当前 active_scene=`{active_scene}`，默认入口是 `{default_scene}`。"
            ));
        }
    }
    if bundle
        .compiled
        .resources
        .iter()
        .any(|item| item.dataset.is_some())
    {
        semantic_hints.push(
            "应用包含 dataset 资源，可继续用 dataset_query / dataset_metric 下钻。".to_string(),
        );
    }
    if !bundle.compiled.component_assets.is_empty() {
        semantic_hints.push("应用依赖 component assets / shared platform assets，适合结合 components/templates 继续理解承载层。".to_string());
    }
    if diagnostics.errors > 0 {
        semantic_hints.push(
            "当前编译产物包含 error diagnostics；摘要可用于理解结构，但不等于语义完全健康。"
                .to_string(),
        );
    }

    Ok(WorldBusinessSummary {
        app_id: app_id.to_string(),
        app_title: bundle.compiled.title.clone(),
        app_kind: semantic.app_kind,
        scene_id: bundle.contract.scene.id.clone(),
        world_id: bundle
            .contract
            .world
            .as_ref()
            .and_then(|world| world.id.clone()),
        active_scene: bundle.compiled.active_scene.clone(),
        default_scene_id,
        scene_profile: semantic.scene_profile,
        scene_summary: semantic.scene_summary,
        scene_goal: semantic.scene_goal,
        active_target_file: bundle.active_target_file,
        business_focus,
        business_explanation: semantic.business_explanation,
        panel_count: semantic.panel_count,
        flow_interaction_count: semantic.flow_interaction_count,
        flow_subject_timer_count: semantic.flow_subject_timer_count,
        has_timer: semantic.has_timer,
        world_has_topology: semantic.world_has_topology,
        frame_layout_type: semantic.frame_layout_type,
        narrative,
        semantic_hints,
        semantic_tags: semantic.semantic_tags,
        resource_kind_counts,
        entity_kind_counts,
        loaded_resource_kind_counts,
        key_resources,
        key_entities,
        scene_routes,
        loaded_resources,
        component_assets,
        diagnostics,
        runtime_summary,
        query_tools,
    })
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
