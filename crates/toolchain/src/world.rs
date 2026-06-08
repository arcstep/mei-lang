use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    decode_ref_value, initial_runtime_state, project_runtime_view, CompiledApp, PanelDecl,
    RefKind, ResourceDecl, RuntimeState, UiNodeDecl,
};
use serde_json::{json, Value};

use crate::types::{
    ResourceInventoryItem, ResourceInventorySnapshot, ResourceQueryToolSpec, WorldAssetGetResponse,
    WorldAssetListItem, WorldAssetListResponse, WorldContextSnapshot, WorldRuntimeBundle,
    WorldRuntimePeekResponse, WorldRuntimeSummary, WorldScope, WorldSnapshotSummary,
};

const LLM_RESOURCE_GET_BUDGET_CHARS: usize = 12_000;

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
        target_file: normalize_scope_field(scope.and_then(|item| item.target_file.as_deref())),
    }
}

pub fn normalize_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

fn app_relative_mei_for_preview(app_id: &str, target_file: &str) -> Option<String> {
    let mut target = normalize_path(target_file);
    if !target.ends_with(".mei") {
        return None;
    }
    let prefix = format!("{}/", app_id.trim_end_matches('/'));
    if target.starts_with(&prefix) {
        target = target[prefix.len()..].to_string();
    }
    if target.is_empty() {
        return None;
    }
    Some(target)
}

fn is_mei_target(target: &str) -> bool {
    target.to_ascii_lowercase().ends_with(".mei")
}

fn resolve_preview_target(app_id: &str, target: &str) -> Option<String> {
    if !is_mei_target(target) {
        return None;
    }
    app_relative_mei_for_preview(app_id, target).or_else(|| Some(normalize_path(target)))
}

fn finish_bundle(compiled: CompiledApp, app_id: &str) -> Result<WorldRuntimeBundle> {
    let contract = compiled
        .scene_contract
        .clone()
        .ok_or_else(|| anyhow!("app `{}` does not provide a scene contract", app_id))?;
    let state = initial_runtime_state(&contract, 1);
    let scene_view = project_runtime_view(&contract, &state);
    let active_target_file = compiled.active_target_file.clone();
    Ok(WorldRuntimeBundle {
        compiled,
        active_target_file,
        contract,
        state,
        scene_view,
    })
}

fn log_bundle_loaded(
    app_id: &str,
    requested_scene: Option<&str>,
    requested_target: Option<&str>,
    strategy: &str,
    fallback_compile: bool,
    bundle: &WorldRuntimeBundle,
    started: Instant,
) {
    tracing::info!(
        app_id = %app_id,
        requested_scene = %requested_scene.unwrap_or("-"),
        requested_target = %requested_target.unwrap_or("-"),
        active_scene = %bundle.compiled.active_scene.as_deref().unwrap_or("-"),
        active_target_file = %bundle.active_target_file,
        compile_strategy = %strategy,
        fallback_compile,
        total_ms = started.elapsed().as_millis() as u64,
        "toolchain world runtime bundle loaded"
    );
}

pub fn load_world_runtime_bundle_with<F>(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    mut compile: F,
) -> Result<WorldRuntimeBundle>
where
    F: FnMut(mei_lang_kernel::CompileOptions) -> Result<CompiledApp>,
{
    let load_started = Instant::now();
    let scope = normalize_world_scope(scope);
    let requested_scene = scope.scene_id.as_deref();
    let requested_target = scope.target_file.clone();
    let app_root = mei_lang_kernel::resolve_app_root(source_root, app_id);
    let mut fallback_compile = false;

    if requested_scene.is_none() {
        if let Some(target) = requested_target.as_deref().filter(|target| is_mei_target(target)) {
            let compiled = compile(mei_lang_kernel::CompileOptions {
                scene: None,
                preview_target: resolve_preview_target(app_id, target),
            })?;
            let bundle = finish_bundle(compiled, app_id)?;
            log_bundle_loaded(
                app_id,
                requested_scene,
                requested_target.as_deref(),
                "target_preview_single_compile",
                fallback_compile,
                &bundle,
                load_started,
            );
            return Ok(bundle);
        }
    }

    if let Some(scene_id) = requested_scene {
        let preview_target = requested_target
            .as_deref()
            .filter(|target| is_mei_target(target))
            .and_then(|target| resolve_preview_target(app_id, target));
        let compiled = compile(mei_lang_kernel::CompileOptions {
            scene: Some(scene_id.to_string()),
            preview_target,
        })?;
        if compiled.active_scene.as_deref() != Some(scene_id) {
            fallback_compile = true;
            let baseline = compile(mei_lang_kernel::CompileOptions::default())?;
            let route = baseline
                .scene_routes
                .iter()
                .find(|route| route.scene_id == scene_id)
                .ok_or_else(|| anyhow!("scene `{scene_id}` not found in app `{app_id}`"))?;
            let preview_target = requested_target
                .as_deref()
                .filter(|target| is_mei_target(target))
                .and_then(|target| {
                    let normalized = normalize_path(target);
                    if normalized == normalize_path(route.target_file.as_str()) {
                        None
                    } else if app_root
                        .join(app_relative_mei_for_preview(app_id, target).unwrap_or(normalized))
                        .is_file()
                    {
                        resolve_preview_target(app_id, target)
                    } else {
                        None
                    }
                });
            let compiled = compile(mei_lang_kernel::CompileOptions {
                scene: Some(scene_id.to_string()),
                preview_target,
            })?;
            if compiled.active_scene.as_deref() != Some(scene_id) {
                return Err(anyhow!("scene `{scene_id}` not found in app `{app_id}`"));
            }
            let bundle = finish_bundle(compiled, app_id)?;
            log_bundle_loaded(
                app_id,
                requested_scene,
                requested_target.as_deref(),
                "scene_route_fallback_compile",
                fallback_compile,
                &bundle,
                load_started,
            );
            return Ok(bundle);
        }
        let bundle = finish_bundle(compiled, app_id)?;
        log_bundle_loaded(
            app_id,
            requested_scene,
            requested_target.as_deref(),
            "scene_single_compile",
            fallback_compile,
            &bundle,
            load_started,
        );
        return Ok(bundle);
    }

    let compiled = compile(mei_lang_kernel::CompileOptions::default())?;
    let bundle = finish_bundle(compiled, app_id)?;
    log_bundle_loaded(
        app_id,
        requested_scene,
        requested_target.as_deref(),
        "default_compile",
        fallback_compile,
        &bundle,
        load_started,
    );
    Ok(bundle)
}

pub fn load_world_runtime_bundle(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldRuntimeBundle> {
    let components_root = crate::resolve_components_root(source_root);
    load_world_runtime_bundle_with(source_root, app_id, scope, |options| {
        crate::compile_app_with_cache(source_root, app_id, options, components_root.as_path())
            .map(|outcome| outcome.compiled)
            .map_err(|failure| failure.error)
    })
}

pub fn default_resource_query_tools() -> Vec<ResourceQueryToolSpec> {
    vec![
        ResourceQueryToolSpec {
            id: "dataset_query".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose:
                "按 dataset 资源 id 查询有界结果（schema+filters+metric ids+sample rows+analysis_contracts_preview）；对应 LLM 工具名 dataset_query"
                    .to_string(),
            input: "{id: string, search?: string, filters?: object, columns?: string[], limit?: number, scene_id?, target_file?}"
                .to_string(),
            output:
                "bounded: {dataset{schema_preview,filters,metric_ids,analysis_contracts_preview}, sample_rows, truncation, usage_hint}; defaults: first 10 rows + first 10 columns + cell text truncation."
                    .to_string(),
        },
        ResourceQueryToolSpec {
            id: "dataset_metric".to_string(),
            status: "phase2_api_ready".to_string(),
            purpose:
                "按 dataset 资源 id 查询运行时指标值（count/rate/trend 等聚合）及 analysis_contract 摘要；对应 LLM 工具名 dataset_metric"
                    .to_string(),
            input: "{id: string, metric_ids?: string[], search?: string, filters?: object, scene_id?, target_file?}"
                .to_string(),
            output:
                "bounded: {dataset_id, total_rows, metrics, analysis_contracts}; when metric_ids omitted returns all runtime metrics for the dataset. analysis_contracts mirrors host UI explain/popup contract."
                    .to_string(),
        },
        ResourceQueryToolSpec {
            id: "resource_list".to_string(),
            status: "phase3_native_ready".to_string(),
            purpose: "列出当前 world 下的 assets（与 LLM 工具 resource_list 一致）".to_string(),
            input: "{kind?: string, limit?: number, scene_id?, target_file?}".to_string(),
            output: "bounded: WorldAssetListResponse JSON".to_string(),
        },
        ResourceQueryToolSpec {
            id: "resource_get".to_string(),
            status: "phase3_native_ready".to_string(),
            purpose: "按 id 获取单个 world asset/entity（与 LLM 工具 resource_get 一致）".to_string(),
            input: "{id: string, scene_id?, target_file?}".to_string(),
            output: "bounded: WorldAssetGetResponse JSON".to_string(),
        },
        ResourceQueryToolSpec {
            id: "resource_runtime_peek".to_string(),
            status: "phase3_native_ready".to_string(),
            purpose: "窥视 world runtime 状态（与 LLM 工具 resource_runtime_peek 一致）".to_string(),
            input: "{trace_limit?: number, scene_id?, target_file?}".to_string(),
            output: "bounded: WorldRuntimePeekResponse JSON".to_string(),
        },
    ]
}

fn json_serialized_len(value: &Value) -> usize {
    serde_json::to_string(value).map(|s| s.len()).unwrap_or(0)
}

fn shrink_json_for_llm(value: &Value, max_total: usize) -> Value {
    let len = json_serialized_len(value);
    if len <= max_total {
        return value.clone();
    }
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, item) in map.iter().take(48) {
                let entry_len = json_serialized_len(item);
                if entry_len > 2_000 {
                    out.insert(
                        key.clone(),
                        json!({
                            "_omitted": true,
                            "approx_chars": entry_len,
                        }),
                    );
                } else {
                    out.insert(key.clone(), item.clone());
                }
            }
            out.insert(
                "_truncated".to_string(),
                json!({
                    "reason": "payload too large for tool output",
                    "approx_original_chars": len,
                }),
            );
            Value::Object(out)
        }
        Value::Array(items) => json!({
            "type": "array",
            "len": items.len(),
            "head": items.iter().take(5).cloned().collect::<Vec<_>>(),
        }),
        Value::String(text) => {
            let cap = 1_000usize;
            if text.len() <= cap {
                Value::String(text.clone())
            } else {
                Value::String(format!("{}…", text.chars().take(cap).collect::<String>()))
            }
        }
        other => other.clone(),
    }
}

fn extract_dataset_schema_preview(dataset: &Value) -> Option<Value> {
    let columns = dataset.get("columns")?.as_array()?;
    const MAX_COLS: usize = 72;
    let mut preview = Vec::new();
    for column in columns.iter().take(MAX_COLS) {
        let Some(map) = column.as_object() else {
            continue;
        };
        let name = map.get("name").and_then(Value::as_str).unwrap_or("?");
        let ty = map
            .get("type")
            .or_else(|| map.get("type_name"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        let mut row = serde_json::Map::new();
        row.insert("name".to_string(), json!(name));
        row.insert("type".to_string(), json!(ty));
        if let Some(source) = map.get("source").and_then(Value::as_str) {
            row.insert("source".to_string(), json!(source));
        }
        if map
            .get("optional")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            row.insert("optional".to_string(), json!(true));
        }
        preview.push(Value::Object(row));
    }
    Some(json!({
        "column_count": columns.len(),
        "columns_preview": preview,
        "columns_preview_truncated": columns.len() > MAX_COLS,
    }))
}

fn summarize_dataset_decl(dataset: &Value) -> Value {
    let len = json_serialized_len(dataset);
    let schema = extract_dataset_schema_preview(dataset);
    match dataset {
        Value::Object(map) => {
            let keys: Vec<&str> = map.keys().map(String::as_str).take(32).collect();
            let kind = map.get("kind").and_then(Value::as_str);
            let key = map.get("key").and_then(Value::as_str);
            let normalize_n = map
                .get("normalize")
                .and_then(Value::as_object)
                .map(|object| object.len())
                .unwrap_or(0);
            json!({
                "present": true,
                "approx_decl_chars": len,
                "kind": kind,
                "key": key,
                "top_level_keys_sample": keys,
                "top_level_key_count": map.len(),
                "normalize_field_count": normalize_n,
                "schema": schema,
                "note": "full dataset body omitted; `schema.columns_preview` lists declared columns (bounded)."
            })
        }
        _ => json!({
            "present": true,
            "approx_decl_chars": len,
            "note": "dataset value is non-object; omitted for size."
        }),
    }
}

fn summarize_filters_decl(filters: &Value) -> Value {
    let len = json_serialized_len(filters);
    if len <= 1_200 {
        return filters.clone();
    }
    match filters {
        Value::Object(map) => json!({
            "object_key_count": map.len(),
            "keys": map.keys().take(40).cloned().collect::<Vec<_>>(),
            "approx_chars": len,
            "note": "filters object truncated to keys only.",
        }),
        _ => json!({
            "approx_chars": len,
            "note": "filters omitted (too large).",
        }),
    }
}

fn summarize_metrics_decl(metrics: &BTreeMap<String, Value>) -> Value {
    let keys: Vec<&str> = metrics.keys().map(String::as_str).take(48).collect();
    json!({
        "metric_ids_sample": keys,
        "metric_id_count": metrics.len(),
        "note": "metric bodies omitted; ids are enough to reason about bindings before read_file.",
    })
}

fn summarize_resource_decl(item: &ResourceDecl) -> Value {
    let content_note = item.content.as_ref().map(|content| {
        if content.len() <= 800 {
            json!(content.as_str())
        } else {
            json!({
                "prefix": content.chars().take(400).collect::<String>(),
                "truncated_chars": content.len().saturating_sub(400),
            })
        }
    });
    json!({
        "_payload_shape": "resource_summary_v1",
        "id": item.id,
        "kind": item.kind,
        "title": item.title,
        "purpose": item.purpose,
        "source": item.source.as_ref().map(|source| json!({ "path": normalize_path(&source.path) })).unwrap_or(Value::Null),
        "dataset": item.dataset.as_ref().map_or(json!({ "present": false }), summarize_dataset_decl),
        "metrics": item.metrics.as_ref().map_or(Value::Null, summarize_metrics_decl),
        "filters": item.filters.as_ref().map_or(Value::Null, summarize_filters_decl),
        "content": content_note.unwrap_or(Value::Null),
    })
}

fn external_slot_binding(value: Option<&Value>, slot: RefKind) -> Option<(String, &'static str)> {
    let value = value?;
    if let Some(map) = value.as_object() {
        let legacy_kind = match slot {
            RefKind::World => Some("world_file_ref"),
            RefKind::Frame => Some("frame_file_ref"),
            RefKind::Scene => Some("scene_file_ref"),
            _ => None,
        };
        if let Some(expected) = legacy_kind {
            let kind = map
                .get("kind")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if kind == expected {
                let path = map
                    .get("path")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or_default();
                if !path.is_empty() {
                    return Some((normalize_path(path), expected));
                }
            }
        }
    }
    if let Some(expr) = decode_ref_value(value) {
        if expr.kind == slot {
            let path = expr
                .locator
                .scene_file
                .as_deref()
                .map(str::trim)
                .filter(|path| !path.is_empty())
                .map(normalize_path)?;
            let resource_type = match slot {
                RefKind::World => "world_ref",
                RefKind::Frame => "frame_ref",
                RefKind::Scene => "scene_ref",
                _ => return None,
            };
            return Some((path, resource_type));
        }
    }
    None
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
            "scene_ref(",
            "world_ref(",
            "flow_ref(",
            "frame_ref(",
            "panel_ref(",
            "dataset_ref(",
            "metric_ref(",
            "resource_ref(",
            "scene_file_ref(",
            "world_file_ref(",
            "frame_file_ref(",
        ] {
            if trimmed.contains(token) {
                refs.push(token.trim_end_matches('(').to_string());
            }
        }
    }
    refs.sort();
    refs.dedup();
    refs
}

fn collect_panel_references(panel: &PanelDecl) -> Vec<String> {
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
            UiNodeDecl::PanelRefEmbed(_) => {}
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
        .or_else(|| Some(bundle.active_target_file.clone()));
    let target_ref = target_file.as_deref();
    let scene_world_binding =
        external_slot_binding(bundle.contract.scene.world.as_ref(), RefKind::World);
    let scene_frame_binding =
        external_slot_binding(bundle.contract.scene.frame.as_ref(), RefKind::Frame);
    let scene_world_path = scene_world_binding.as_ref().map(|(path, _)| path.clone());
    let scene_frame_path = scene_frame_binding.as_ref().map(|(path, _)| path.clone());
    let mut items = Vec::new();

    push_inventory_item(
        &mut items,
        bundle.contract.scene.id.clone(),
        "scene",
        Some(bundle.contract.scene.id.clone()),
        bundle.contract.scene.summary.clone(),
        Some(bundle.active_target_file.clone()),
        Vec::new(),
        target_ref,
    );
    if let Some((path, resource_type)) = scene_world_binding.clone() {
        push_inventory_item(
            &mut items,
            format!("{resource_type}:{path}"),
            resource_type,
            Some(path.clone()),
            Some("scene 绑定的外部 world capsule".to_string()),
            Some(path),
            Vec::new(),
            target_ref,
        );
    }
    if let Some((path, resource_type)) = scene_frame_binding.clone() {
        push_inventory_item(
            &mut items,
            format!("{resource_type}:{path}"),
            resource_type,
            Some(path.clone()),
            Some("scene 绑定的外部 frame capsule".to_string()),
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
            scene_world_path
                .clone()
                .or_else(|| Some(bundle.active_target_file.clone())),
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
                    .or_else(|| scene_world_path.clone())
                    .or_else(|| Some(bundle.active_target_file.clone())),
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
                scene_world_path
                    .clone()
                    .or_else(|| Some(bundle.active_target_file.clone())),
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
                    scene_world_path
                        .clone()
                        .or_else(|| Some(bundle.active_target_file.clone())),
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
            scene_frame_path
                .clone()
                .or_else(|| Some(bundle.active_target_file.clone())),
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
            Some(bundle.active_target_file.clone()),
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
            scene_frame_path
                .clone()
                .or_else(|| Some(bundle.active_target_file.clone())),
            collect_panel_references(panel),
            target_ref,
        );
    }

    for route in &bundle.compiled.scene_routes {
        push_inventory_item(
            &mut items,
            route.scene_id.clone(),
            "scene_route",
            route.title.clone(),
            Some(format!("kind={}", route.kind)),
            Some(normalize_path(&route.target_file)),
            vec![format!("scene:{}", route.scene_id)],
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
                if resource.dataset.is_some() { "yes" } else { "no" }
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
        let source_path = mei_lang_kernel::resolve_app_root(source_root, app_id).join(target);
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

    let query_tools = default_resource_query_tools();
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

pub fn query_world_assets(
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

pub fn query_world_asset(
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
            let mut payload = summarize_resource_decl(item);
            if json_serialized_len(&payload) > LLM_RESOURCE_GET_BUDGET_CHARS {
                payload = shrink_json_for_llm(&payload, LLM_RESOURCE_GET_BUDGET_CHARS);
            }
            return Ok(WorldAssetGetResponse {
                app_id: app_id.to_string(),
                scene_id: bundle.contract.scene.id.clone(),
                id: item.id.clone(),
                kind: "resource".to_string(),
                payload,
            });
        }
        if let Some(item) = world.entities.iter().find(|item| item.id == target_id) {
            let raw = serde_json::to_value(item).unwrap_or(Value::Null);
            let payload = shrink_json_for_llm(&raw, LLM_RESOURCE_GET_BUDGET_CHARS);
            return Ok(WorldAssetGetResponse {
                app_id: app_id.to_string(),
                scene_id: bundle.contract.scene.id.clone(),
                id: item.id.clone(),
                kind: "entity".to_string(),
                payload,
            });
        }
        if let Some(topology) = &world.topology {
            if let Some(item) = topology.cells.iter().find(|item| item.id == target_id) {
                let raw = serde_json::to_value(item).unwrap_or(Value::Null);
                let payload = shrink_json_for_llm(&raw, LLM_RESOURCE_GET_BUDGET_CHARS);
                return Ok(WorldAssetGetResponse {
                    app_id: app_id.to_string(),
                    scene_id: bundle.contract.scene.id.clone(),
                    id: item.id.clone(),
                    kind: "cell".to_string(),
                    payload,
                });
            }
        }
    }

    Err(anyhow!("world asset `{target_id}` not found"))
}

pub fn query_world_runtime(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    trace_limit: Option<usize>,
) -> Result<WorldRuntimePeekResponse> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let trace_limit = normalize_limit(trace_limit, 5, 50);
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
        recent_trace_messages: recent_trace_messages(&bundle.state, trace_limit),
    })
}

#[cfg(test)]
mod tests {
    use super::{app_relative_mei_for_preview, extract_ref_tokens_from_source, normalize_path};

    #[test]
    fn app_relative_preview_target_keeps_app_relative_mei() {
        assert_eq!(
            app_relative_mei_for_preview("demo", "demo/scenes/home.mei").as_deref(),
            Some("scenes/home.mei")
        );
    }

    #[test]
    fn normalize_path_strips_relative_prefix() {
        assert_eq!(normalize_path("./foo\\bar.mei"), "foo/bar.mei");
    }

    #[test]
    fn extract_ref_tokens_collects_typed_and_legacy_refs() {
        let source = r#"
scene(id="s1", world = world_ref(scene_file = "worlds/home.mei"))
panel_ref("overview")
world_file_ref(path = "legacy.mei")
"#;
        let refs = extract_ref_tokens_from_source(source);
        assert!(refs.contains(&"world_ref".to_string()));
        assert!(refs.contains(&"panel_ref".to_string()));
        assert!(refs.contains(&"world_file_ref".to_string()));
    }
}
