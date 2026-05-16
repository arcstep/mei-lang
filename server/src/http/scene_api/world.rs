use std::path::Path;
use std::{collections::BTreeMap, fs};

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    compile_app, compile_app_with_options, initial_runtime_state, project_runtime_view,
    CompileOptions, RuntimeState, UiNodeDecl,
};
use serde_json::Value;

use super::types::{
    ResourceInventoryItem, ResourceInventorySnapshot, ResourceQueryToolSpec, WorldAssetGetResponse,
    WorldAssetListItem, WorldAssetListResponse, WorldContextSnapshot, WorldRuntimeBundle,
    WorldRuntimePeekResponse, WorldRuntimeSummary, WorldScope, WorldSnapshotSummary,
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
        .clone()
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
    let entry_target = compiled.entry_target.clone();
    Ok(WorldRuntimeBundle {
        compiled,
        entry_target,
        contract,
        state,
        scene_view,
    })
}

fn normalize_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn file_ref_from_scene_binding(value: Option<&Value>, expected_kind: &str) -> Option<String> {
    let value = value?;
    let map = value.as_object()?;
    let kind = map
        .get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if kind != expected_kind {
        return None;
    }
    let path = map
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    if path.is_empty() {
        return None;
    }
    Some(normalize_path(path))
}

fn related_to_target(source_path: Option<&str>, target_file: Option<&str>) -> bool {
    let Some(target) = target_file
        .map(normalize_path)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    source_path
        .map(normalize_path)
        .is_some_and(|source| source == target)
}

fn collect_refs_from_value(value: &Value, refs: &mut Vec<String>, depth: usize) {
    if depth > 5 {
        return;
    }
    match value {
        Value::Object(map) => {
            if let Some(kind) = map.get("kind").and_then(Value::as_str) {
                if kind.ends_with("_ref") || kind.ends_with("_file_ref") {
                    refs.push(kind.to_string());
                }
            }
            if let Some(raw_ref) = map.get("__ref").and_then(Value::as_str) {
                refs.push(format!("__ref:{raw_ref}"));
            }
            for (key, entry) in map {
                if key.ends_with("_ref") || key.ends_with("_file_ref") {
                    if let Some(text) = entry.as_str() {
                        refs.push(format!("{key}:{text}"));
                    } else {
                        refs.push(key.to_string());
                    }
                }
                collect_refs_from_value(entry, refs, depth + 1);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_refs_from_value(item, refs, depth + 1);
            }
        }
        _ => {}
    }
}

fn extract_ref_tokens_from_source(source: &str) -> Vec<String> {
    let mut refs = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        for token in [
            "world_ref(",
            "frame_ref(",
            "panel_ref(",
            "data_ref(",
            "metric_ref(",
        ] {
            if trimmed.contains(token) {
                refs.push(token.trim_end_matches('(').to_string());
            }
        }
        for token in ["scene_file_ref(", "world_file_ref(", "frame_file_ref("] {
            if trimmed.contains(token) {
                refs.push(token.trim_end_matches('(').to_string());
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn collect_panel_references(panel: &mei_lang_kernel::PanelDecl) -> Vec<String> {
    let mut refs = Vec::new();
    for node in &panel.blocks {
        match node {
            UiNodeDecl::Panel(child) => {
                refs.push(format!("panel:{}", child.id));
                refs.extend(collect_panel_references(child));
            }
            UiNodeDecl::Block(block) => {
                refs.push(format!("use_key:{}", block.use_key));
                collect_refs_from_value(&block.props, &mut refs, 0);
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn push_inventory_item(
    out: &mut Vec<ResourceInventoryItem>,
    id: String,
    resource_type: &str,
    title: Option<String>,
    summary: Option<String>,
    source_path: Option<String>,
    references: Vec<String>,
    target_file: Option<&str>,
) {
    out.push(ResourceInventoryItem {
        id,
        resource_type: resource_type.to_string(),
        title,
        summary,
        source_path: source_path.clone(),
        references,
        related_to_target: related_to_target(source_path.as_deref(), target_file),
    });
}

fn build_resource_inventory(
    source_root: &Path,
    app_id: &str,
    bundle: &WorldRuntimeBundle,
    scope: Option<&WorldScope>,
) -> ResourceInventorySnapshot {
    let target_file = scope
        .and_then(|item| item.target_file.as_deref())
        .map(normalize_path)
        .or_else(|| Some(bundle.entry_target.clone()));
    let target_ref = target_file.as_deref();
    let scene_world_file_ref =
        file_ref_from_scene_binding(bundle.contract.scene.world.as_ref(), "world_file_ref");
    let scene_frame_file_ref =
        file_ref_from_scene_binding(bundle.contract.scene.frame.as_ref(), "frame_file_ref");
    let mut items = Vec::new();

    push_inventory_item(
        &mut items,
        bundle.contract.scene.id.clone(),
        "scene",
        Some(bundle.contract.scene.id.clone()),
        bundle.contract.scene.summary.clone(),
        Some(bundle.entry_target.clone()),
        Vec::new(),
        target_ref,
    );
    if let Some(path) = scene_world_file_ref.clone() {
        push_inventory_item(
            &mut items,
            format!("world_file_ref:{path}"),
            "world_file_ref",
            Some(path.clone()),
            Some("scene 绑定的外部 world 文件".to_string()),
            Some(path),
            Vec::new(),
            target_ref,
        );
    }
    if let Some(path) = scene_frame_file_ref.clone() {
        push_inventory_item(
            &mut items,
            format!("frame_file_ref:{path}"),
            "frame_file_ref",
            Some(path.clone()),
            Some("scene 绑定的外部 frame 文件".to_string()),
            Some(path),
            Vec::new(),
            target_ref,
        );
    }

    if let Some(world) = &bundle.contract.world {
        push_inventory_item(
            &mut items,
            world.id.clone().unwrap_or_else(|| "world".to_string()),
            "world",
            world.id.clone(),
            Some(format!(
                "resources={} entities={} cells={}",
                world.resources.len(),
                world.entities.len(),
                world
                    .topology
                    .as_ref()
                    .map(|item| item.cells.len())
                    .unwrap_or(0)
            )),
            scene_world_file_ref
                .clone()
                .or_else(|| Some(bundle.entry_target.clone())),
            Vec::new(),
            target_ref,
        );
        for item in &world.resources {
            let mut references = Vec::new();
            if let Some(source) = item.source.as_ref() {
                if !source.path.trim().is_empty() {
                    references.push(format!("source_path:{}", normalize_path(&source.path)));
                }
            }
            push_inventory_item(
                &mut items,
                item.id.clone(),
                "resource",
                item.title.clone(),
                Some(format!("kind={}", item.kind)),
                item.source
                    .as_ref()
                    .map(|source| normalize_path(&source.path))
                    .or_else(|| scene_world_file_ref.clone())
                    .or_else(|| Some(bundle.entry_target.clone())),
                references,
                target_ref,
            );
        }
        for item in &world.entities {
            push_inventory_item(
                &mut items,
                item.id.clone(),
                "entity",
                item.label.clone(),
                Some(format!(
                    "kind={} status={}",
                    item.kind,
                    item.status.as_deref().unwrap_or("unknown")
                )),
                scene_world_file_ref
                    .clone()
                    .or_else(|| Some(bundle.entry_target.clone())),
                item.spawns.clone(),
                target_ref,
            );
        }
        if let Some(topology) = &world.topology {
            for cell in &topology.cells {
                push_inventory_item(
                    &mut items,
                    cell.id.clone(),
                    "cell",
                    cell.surface_kind.clone(),
                    Some(format!(
                        "hazard={} row={:?} col={:?}",
                        cell.hazard_state.as_deref().unwrap_or("none"),
                        cell.row,
                        cell.col
                    )),
                    scene_world_file_ref
                        .clone()
                        .or_else(|| Some(bundle.entry_target.clone())),
                    cell.tags.clone(),
                    target_ref,
                );
            }
        }
    }

    if let Some(frame) = &bundle.contract.frame {
        push_inventory_item(
            &mut items,
            frame.id.clone().unwrap_or_else(|| "frame".to_string()),
            "frame",
            frame.title.clone(),
            Some("scene 主 frame".to_string()),
            scene_frame_file_ref
                .clone()
                .or_else(|| Some(bundle.entry_target.clone())),
            Vec::new(),
            target_ref,
        );
    }
    if let Some(flow) = &bundle.contract.flow {
        push_inventory_item(
            &mut items,
            flow.id.clone().unwrap_or_else(|| "flow".to_string()),
            "flow",
            flow.id.clone(),
            Some(format!(
                "interactions={} subject_timers={}",
                flow.interactions.len(),
                flow.subject_timers.len()
            )),
            Some(bundle.entry_target.clone()),
            Vec::new(),
            target_ref,
        );
    }
    for panel in &bundle.contract.panels {
        push_inventory_item(
            &mut items,
            panel.id.clone(),
            "panel",
            panel.title.clone(),
            Some(format!("blocks={}", panel.blocks.len())),
            scene_frame_file_ref
                .clone()
                .or_else(|| Some(bundle.entry_target.clone())),
            collect_panel_references(panel),
            target_ref,
        );
    }

    for entry in &bundle.compiled.entries {
        push_inventory_item(
            &mut items,
            entry.entry_id.clone(),
            "entry",
            entry.title.clone(),
            Some(format!("scene_id={} kind={}", entry.scene_id, entry.kind)),
            Some(normalize_path(&entry.target_file)),
            vec![format!("scene:{}", entry.scene_id)],
            target_ref,
        );
    }
    for resource in &bundle.compiled.resources {
        push_inventory_item(
            &mut items,
            resource.id.clone(),
            "loaded_resource",
            resource.title.clone(),
            Some(format!(
                "kind={} dataset={}",
                resource.kind,
                if resource.dataset.is_some() {
                    "yes"
                } else {
                    "no"
                }
            )),
            None,
            Vec::new(),
            target_ref,
        );
    }
    for asset in &bundle.compiled.component_assets {
        push_inventory_item(
            &mut items,
            asset.key.clone(),
            "component_asset",
            Some(asset.tag.clone()),
            Some(format!("script={}", asset.script)),
            Some(normalize_path(&asset.script)),
            Vec::new(),
            target_ref,
        );
    }

    if let Some(target) = target_ref {
        let source_path = source_root.join(app_id).join(target);
        if let Ok(source) = fs::read_to_string(&source_path) {
            let refs = extract_ref_tokens_from_source(&source);
            if !refs.is_empty() {
                push_inventory_item(
                    &mut items,
                    format!("source_refs:{target}"),
                    "source_refs",
                    Some(target.to_string()),
                    Some("当前文件中检测到的 *_ref/*_file_ref 引用提示".to_string()),
                    Some(target.to_string()),
                    refs,
                    target_ref,
                );
            }
        }
    }

    ResourceInventorySnapshot {
        target_file,
        total_items: items.len(),
        items,
    }
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

pub(crate) fn query_resource_list(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    kind: Option<&str>,
    limit: Option<usize>,
) -> Result<WorldAssetListResponse> {
    query_world_assets(source_root, app_id, scope, kind, limit)
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

pub(crate) fn query_resource_get(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
) -> Result<WorldAssetGetResponse> {
    query_world_asset(source_root, app_id, scope, id)
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

pub(crate) fn query_resource_runtime_peek(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    trace_limit: Option<usize>,
) -> Result<WorldRuntimePeekResponse> {
    query_world_runtime(source_root, app_id, scope, trace_limit)
}

pub(crate) fn default_resource_query_tools() -> Vec<ResourceQueryToolSpec> {
    vec![
        ResourceQueryToolSpec {
            id: "resource.list".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose: "按 scope 与类型查看资源清单（entity/resource/cell）".to_string(),
            input: "{scope: {scene_id, entry_id, target_file}, kind?: entity|resource|cell, limit?: number}"
                .to_string(),
            output:
                "{items: [{id, kind, label_or_title, tags?}], total}; endpoint: GET /api/world/assets/*app_id?scene_id=..."
                    .to_string(),
        },
        ResourceQueryToolSpec {
            id: "resource.get".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose: "按资源 id 查看单个对象详情".to_string(),
            input: "{scope: {scene_id, entry_id, target_file}, id: string}".to_string(),
            output:
                "{id, kind, fields, relations?}; endpoint: GET /api/world/asset/*app_id?id=...&scene_id=..."
                .to_string(),
        },
        ResourceQueryToolSpec {
            id: "resource.runtime.peek".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose: "查看运行态关键信息（phase/result/actions/trace）".to_string(),
            input: "{scope: {scene_id, entry_id, target_file}, trace_limit?: number}".to_string(),
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
        query_tools: default_resource_query_tools(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_query_tool_ids_are_stable() {
        let ids = default_resource_query_tools()
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "resource.list".to_string(),
                "resource.get".to_string(),
                "resource.runtime.peek".to_string()
            ]
        );
    }

    #[test]
    fn extract_ref_tokens_collects_common_refs() {
        let source = r#"
scene(kind="scene", id="s1", world=world_file_ref(path="worlds/s1-world.mei"))
panel_ref("overview")
metric_ref("sales_growth")
"#;
        let refs = extract_ref_tokens_from_source(source);
        assert!(refs.contains(&"world_file_ref".to_string()));
        assert!(refs.contains(&"panel_ref".to_string()));
        assert!(refs.contains(&"metric_ref".to_string()));
    }

    #[test]
    fn related_target_normalizes_relative_prefix() {
        assert!(related_to_target(Some("./apps/demo/main.mei"), Some("apps/demo/main.mei")));
        assert!(!related_to_target(Some("apps/demo/other.mei"), Some("apps/demo/main.mei")));
    }
}
