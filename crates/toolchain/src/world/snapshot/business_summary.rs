use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use crate::semantic_summary::summarize_compiled_app_semantics;
use crate::types::{
    CompiledRouteSummary, ComponentAssetSummary, LoadedResourceSummary, WorldBusinessEntitySummary,
    WorldBusinessResourceSummary, WorldBusinessSummary,
    WorldRuntimeSummary, WorldScope,
};

use super::super::bundle::{load_world_runtime_bundle, normalize_path};
use super::super::inventory::build_resource_inventory;
use super::catalog_lines::*;
use super::helpers::*;

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
    let query_tools = super::super::default_resource_query_tools();
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

