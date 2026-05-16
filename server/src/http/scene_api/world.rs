use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    compile_app, compile_app_with_options, initial_runtime_state, project_runtime_view,
    CompileOptions, RuntimeState,
};
use serde_json::Value;

use super::types::{
    WorldAssetGetResponse, WorldAssetListItem, WorldAssetListResponse, WorldContextSnapshot,
    WorldQueryCapabilitySummary, WorldRuntimeBundle, WorldRuntimePeekResponse,
    WorldRuntimeSummary, WorldScope, WorldSnapshotSummary,
};

fn normalize_asset_kind(kind: Option<&str>) -> String {
    match kind.map(|value| value.trim().to_ascii_lowercase()) {
        Some(value) if value == "entity" => "entity".to_string(),
        Some(value) if value == "resource" => "resource".to_string(),
        Some(value) if value == "cell" => "cell".to_string(),
        _ => "all".to_string(),
    }
}

fn normalize_limit(limit: Option<usize>, default: usize, max: usize) -> usize {
    limit.unwrap_or(default).clamp(1, max)
}

fn normalize_scope_field(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

fn normalize_world_scope(scope: Option<&WorldScope>) -> WorldScope {
    WorldScope {
        scene_id: normalize_scope_field(scope.and_then(|item| item.scene_id.as_deref())),
        entry_id: normalize_scope_field(scope.and_then(|item| item.entry_id.as_deref())),
        target_file: normalize_scope_field(scope.and_then(|item| item.target_file.as_deref())),
    }
}

fn load_world_runtime_bundle(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldRuntimeBundle> {
    let scope = normalize_world_scope(scope);
    let requested_scene = scope.scene_id.as_deref();
    let requested_entry = scope.entry_id.as_deref();
    let requested_target = scope.target_file.clone();

    let mut selected_entry = requested_entry.map(str::to_string);
    if let Some(scene_id) = requested_scene {
        let base_compiled = compile_app(source_root, app_id)?;
        let scene_entry = base_compiled
            .entries
            .iter()
            .find(|item| item.scene_id == scene_id || item.entry_id == scene_id)
            .ok_or_else(|| anyhow!("scene `{scene_id}` not found in app `{app_id}`"))?;
        if let Some(entry_id) = requested_entry {
            if entry_id != scene_entry.entry_id {
                return Err(anyhow!(
                    "scene `{scene_id}` does not match entry `{entry_id}`"
                ));
            }
        }
        if let Some(target_file) = requested_target.as_deref() {
            if target_file != scene_entry.target_file {
                return Err(anyhow!(
                    "scene `{scene_id}` is not bound to target `{target_file}`"
                ));
            }
        }
        selected_entry = Some(scene_entry.entry_id.clone());
    }

    let compiled = compile_app_with_options(
        source_root,
        app_id,
        CompileOptions {
            entry: selected_entry.clone(),
            preview_target: if selected_entry.is_some() {
                None
            } else {
                requested_target.clone()
            },
        },
    )?;
    if let Some(entry_id) = selected_entry.as_deref() {
        if compiled.active_entry.as_deref() != Some(entry_id) {
            return Err(anyhow!("entry `{entry_id}` not found in app `{app_id}`"));
        }
    }
    let contract = compiled
        .scene_contract
        .ok_or_else(|| anyhow!("app `{}` does not provide a scene contract", app_id))?;
    if let Some(scene_id) = requested_scene {
        if contract.scene.id != scene_id {
            return Err(anyhow!(
                "requested scene `{scene_id}` but active scene is `{}`",
                contract.scene.id
            ));
        }
    }
    let state = initial_runtime_state(&contract, 1);
    let scene_view = project_runtime_view(&contract, &state);
    Ok(WorldRuntimeBundle {
        entry_target: compiled.entry_target,
        contract,
        state,
        scene_view,
    })
}

fn collect_world_asset_items(
    bundle: &WorldRuntimeBundle,
    kind: &str,
    limit: usize,
) -> (usize, Vec<WorldAssetListItem>) {
    let mut items = Vec::new();
    let mut total = 0usize;

    if matches!(kind, "all" | "resource") {
        if let Some(world) = &bundle.contract.world {
            total += world.resources.len();
            for item in &world.resources {
                if items.len() >= limit {
                    break;
                }
                items.push(WorldAssetListItem {
                    id: item.id.clone(),
                    kind: "resource".to_string(),
                    label: None,
                    title: item.title.clone(),
                    status: None,
                    tags: Vec::new(),
                });
            }
        }
    }

    if matches!(kind, "all" | "entity") {
        if let Some(world) = &bundle.contract.world {
            total += world.entities.len();
            for item in &world.entities {
                if items.len() >= limit {
                    break;
                }
                items.push(WorldAssetListItem {
                    id: item.id.clone(),
                    kind: "entity".to_string(),
                    label: item.label.clone(),
                    title: None,
                    status: item.status.clone(),
                    tags: Vec::new(),
                });
            }
        }
    }

    if matches!(kind, "all" | "cell") {
        if let Some(world) = &bundle.contract.world {
            if let Some(topology) = &world.topology {
                total += topology.cells.len();
                for cell in &topology.cells {
                    if items.len() >= limit {
                        break;
                    }
                    items.push(WorldAssetListItem {
                        id: cell.id.clone(),
                        kind: "cell".to_string(),
                        label: None,
                        title: cell.surface_kind.clone(),
                        status: cell.hazard_state.clone(),
                        tags: cell.tags.clone(),
                    });
                }
            }
        }
    }

    (total, items)
}

fn recent_trace_messages(state: &RuntimeState, trace_limit: usize) -> Vec<String> {
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

pub(crate) fn query_world_assets(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    kind: Option<&str>,
    limit: Option<usize>,
) -> Result<WorldAssetListResponse> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let normalized_kind = normalize_asset_kind(kind);
    let normalized_limit = normalize_limit(limit, 20, 200);
    let (total, items) = collect_world_asset_items(&bundle, &normalized_kind, normalized_limit);
    Ok(WorldAssetListResponse {
        app_id: app_id.to_string(),
        scene_id: bundle.contract.scene.id.clone(),
        query_kind: normalized_kind,
        total,
        items,
    })
}

pub(crate) fn query_world_asset(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
) -> Result<WorldAssetGetResponse> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let target_id = id.trim();
    if target_id.is_empty() {
        return Err(anyhow!("query parameter `id` is required"));
    }

    if let Some(world) = &bundle.contract.world {
        if let Some(item) = world.resources.iter().find(|item| item.id == target_id) {
            return Ok(WorldAssetGetResponse {
                app_id: app_id.to_string(),
                scene_id: bundle.contract.scene.id.clone(),
                id: item.id.clone(),
                kind: "resource".to_string(),
                payload: serde_json::to_value(item).unwrap_or(Value::Null),
            });
        }
        if let Some(item) = world.entities.iter().find(|item| item.id == target_id) {
            return Ok(WorldAssetGetResponse {
                app_id: app_id.to_string(),
                scene_id: bundle.contract.scene.id.clone(),
                id: item.id.clone(),
                kind: "entity".to_string(),
                payload: serde_json::to_value(item).unwrap_or(Value::Null),
            });
        }
        if let Some(topology) = &world.topology {
            if let Some(item) = topology.cells.iter().find(|item| item.id == target_id) {
                return Ok(WorldAssetGetResponse {
                    app_id: app_id.to_string(),
                    scene_id: bundle.contract.scene.id.clone(),
                    id: item.id.clone(),
                    kind: "cell".to_string(),
                    payload: serde_json::to_value(item).unwrap_or(Value::Null),
                });
            }
        }
    }

    Err(anyhow!("world asset `{target_id}` not found"))
}

pub(crate) fn query_world_runtime(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    trace_limit: Option<usize>,
) -> Result<WorldRuntimePeekResponse> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let normalized_trace_limit = normalize_limit(trace_limit, 5, 50);
    Ok(WorldRuntimePeekResponse {
        app_id: app_id.to_string(),
        scene_id: bundle.contract.scene.id.clone(),
        phase: bundle.state.phase.clone(),
        result: bundle.state.result.clone(),
        countdown: bundle.state.countdown,
        available_actions: bundle
            .scene_view
            .available_actions
            .iter()
            .take(20)
            .cloned()
            .collect(),
        recent_trace_messages: recent_trace_messages(&bundle.state, normalized_trace_limit),
    })
}

fn default_world_query_capabilities() -> Vec<WorldQueryCapabilitySummary> {
    vec![
        WorldQueryCapabilitySummary {
            id: "world.asset.list".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose: "按类型查看 world 里的核心资产清单（entity/resource/cell）".to_string(),
            input: "{kind?: entity|resource|cell, limit?: number}".to_string(),
            output:
                "{items: [{id, kind, label_or_title, tags?}], total}; endpoint: GET /api/world/assets/*app_id?scene_id=..."
                    .to_string(),
        },
        WorldQueryCapabilitySummary {
            id: "world.asset.get".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose: "按资产 id 查看单个对象详情".to_string(),
            input: "{id: string}".to_string(),
            output:
                "{id, kind, fields, relations?}; endpoint: GET /api/world/asset/*app_id?id=...&scene_id=..."
                .to_string(),
        },
        WorldQueryCapabilitySummary {
            id: "world.runtime.peek".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose: "查看运行态关键信息（phase/result/actions/trace）".to_string(),
            input: "{include?: [state|actions|trace], trace_limit?: number}".to_string(),
            output:
                "{phase, result, available_actions, recent_trace_messages}; endpoint: GET /api/world/runtime/*app_id?scene_id=..."
                    .to_string(),
        },
    ]
}

pub(crate) fn build_world_context_snapshot(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldContextSnapshot> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
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

    let recent_trace_messages = recent_trace_messages(&bundle.state, 5);

    Ok(WorldContextSnapshot {
        app_id: app_id.to_string(),
        entry_target: bundle.entry_target,
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
        query_capabilities: default_world_query_capabilities(),
    })
}
