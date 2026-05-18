use std::path::Path;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use anyhow::{anyhow, Result};
use mei_lang_kernel::{
    compile_app_with_options, evaluate_runtime_metric_defs, initial_runtime_state,
    project_runtime_view, CompileOptions, CompiledApp, DatasetView, RuntimeState, UiNodeDecl,
};
use serde_json::{json, Value};

use crate::{
    http::{
        compile_cache::compile_app_with_cache,
        datasets::{query_dataset_rows, DatasetQueryOptions},
        pages::resolve_components_root,
    },
    AppState,
};

use super::resource_query::default_resource_query_tools;
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
        target_file: normalize_scope_field(scope.and_then(|item| item.target_file.as_deref())),
    }
}

/// 将请求里的 `target_file` 规范为「相对 app 根」的 `.mei` 路径（供 preview 编译与磁盘探测）。
/// 允许传入 workspace 相对路径 `{app_id}/data/...` 或仅用 `data/...`。
fn app_relative_mei_for_preview(app_id: &str, target_file: &str) -> Option<String> {
    let mut t = normalize_path(target_file);
    if !t.ends_with(".mei") {
        return None;
    }
    let prefix = format!("{}/", app_id.trim_end_matches('/'));
    if t.starts_with(&prefix) {
        t = t[prefix.len()..].to_string();
    }
    if t.is_empty() {
        return None;
    }
    Some(t)
}

fn load_world_runtime_bundle_with<F>(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    mut compile: F,
) -> Result<WorldRuntimeBundle>
where
    F: FnMut(CompileOptions) -> Result<CompiledApp>,
{
    let scope = normalize_world_scope(scope);
    let requested_scene = scope.scene_id.as_deref();
    let requested_target = scope.target_file.clone();

    let base_compiled = compile(CompileOptions {
        scene: None,
        preview_target: None,
    })?;
    let app_root = source_root.join(app_id);
    let mut selected_scene: Option<String> = None;

    if let Some(scene_id) = requested_scene {
        let by_scene = base_compiled
            .scene_routes
            .iter()
            .find(|item| item.scene_id == scene_id)
            .ok_or_else(|| anyhow!("scene `{scene_id}` not found in app `{app_id}`"))?;
        if let Some(target_file) = requested_target.as_deref() {
            let nt = normalize_path(target_file);
            let matches_target = nt == normalize_path(by_scene.target_file.as_str());
            if matches_target {
                selected_scene = Some(by_scene.scene_id.clone());
            } else if let Some(by_target) = base_compiled
                .scene_routes
                .iter()
                .find(|e| normalize_path(e.target_file.as_str()) == nt)
            {
                if by_target.scene_id != by_scene.scene_id {
                    return Err(anyhow!(
                        "scene `{scene_id}` is not bound to target `{target_file}`"
                    ));
                }
                selected_scene = Some(by_target.scene_id.clone());
            } else if let Some(rel) = app_relative_mei_for_preview(app_id, target_file) {
                if let Some(by_target) = base_compiled
                    .scene_routes
                    .iter()
                    .find(|e| normalize_path(e.target_file.as_str()) == normalize_path(&rel))
                {
                    if by_target.scene_id != by_scene.scene_id {
                        return Err(anyhow!(
                            "scene `{scene_id}` is not bound to target `{target_file}`"
                        ));
                    }
                    selected_scene = Some(by_target.scene_id.clone());
                } else if app_root.join(&rel).is_file() {
                    selected_scene = None;
                } else {
                    return Err(anyhow!(
                        "scene `{scene_id}` is not bound to target `{target_file}`"
                    ));
                }
            } else {
                return Err(anyhow!(
                    "scene `{scene_id}` is not bound to target `{target_file}`"
                ));
            }
        } else {
            selected_scene = Some(by_scene.scene_id.clone());
        }
    } else if let Some(target_only) = requested_target.as_deref() {
        let nt = normalize_path(target_only);
        if let Some(found) = base_compiled
            .scene_routes
            .iter()
            .find(|item| normalize_path(item.target_file.as_str()) == nt)
        {
            selected_scene = Some(found.scene_id.clone());
        } else if let Some(rel) = app_relative_mei_for_preview(app_id, target_only) {
            if let Some(found) = base_compiled
                .scene_routes
                .iter()
                .find(|e| normalize_path(e.target_file.as_str()) == normalize_path(&rel))
            {
                selected_scene = Some(found.scene_id.clone());
            }
        }
    }

    let preview_path = if selected_scene.is_some() {
        None
    } else {
        requested_target.as_deref().and_then(|target| {
            if !target.to_lowercase().ends_with(".mei") {
                return None;
            }
            app_relative_mei_for_preview(app_id, target).or_else(|| Some(normalize_path(target)))
        })
    };

    let compiled = compile(CompileOptions {
        scene: selected_scene.clone(),
        preview_target: preview_path,
    })?;
    if let Some(sid) = selected_scene.as_deref() {
        if compiled.active_scene.as_deref() != Some(sid) {
            return Err(anyhow!("scene `{sid}` not found in app `{app_id}`"));
        }
    }
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

fn load_world_runtime_bundle(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldRuntimeBundle> {
    load_world_runtime_bundle_with(source_root, app_id, scope, |options| {
        compile_app_with_options(source_root, app_id, options)
    })
}

fn load_world_runtime_bundle_cached(
    state: &AppState,
    app_id: &str,
    scope: Option<&WorldScope>,
) -> Result<WorldRuntimeBundle> {
    let components_root = resolve_components_root(&state.source_root);
    load_world_runtime_bundle_with(&state.source_root, app_id, scope, |options| {
        compile_app_with_cache(state, app_id, options, components_root.as_path())
            .map(|outcome| outcome.compiled)
            .map_err(|failure| failure.error)
    })
}

fn normalize_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

const LLM_RESOURCE_GET_BUDGET_CHARS: usize = 12_000;
const DATASET_QUERY_DEFAULT_LIMIT: usize = 10;
const DATASET_QUERY_MAX_LIMIT: usize = 50;
const DATASET_QUERY_DEFAULT_COLUMNS: usize = 10;
const DATASET_QUERY_MAX_COLUMNS: usize = 10;
const DATASET_QUERY_MAX_CELL_CHARS: usize = 50;
const DATASET_QUERY_TOTAL_CHAR_BUDGET: usize = 12_000;

fn json_serialized_len(v: &Value) -> usize {
    serde_json::to_string(v).map(|s| s.len()).unwrap_or(0)
}

fn normalize_dataset_limit(limit: Option<usize>) -> usize {
    normalize_limit(limit, DATASET_QUERY_DEFAULT_LIMIT, DATASET_QUERY_MAX_LIMIT)
}

fn dataset_available_columns(dataset: &DatasetView) -> Vec<String> {
    if !dataset.columns.is_empty() {
        return dataset.columns.clone();
    }
    dataset.schema.iter().map(|c| c.name.clone()).collect()
}

fn normalize_dataset_columns(dataset: &DatasetView, requested: Option<&[String]>) -> Vec<String> {
    let available = dataset_available_columns(dataset);
    let available_set = available.iter().cloned().collect::<BTreeSet<_>>();
    let mut selected = Vec::new();

    if let Some(req) = requested {
        for col in req {
            let name = col.trim();
            if name.is_empty() {
                continue;
            }
            if available_set.contains(name) && !selected.iter().any(|v| v == name) {
                selected.push(name.to_string());
            }
            if selected.len() >= DATASET_QUERY_MAX_COLUMNS {
                break;
            }
        }
    }

    if selected.is_empty() {
        selected = available
            .into_iter()
            .take(DATASET_QUERY_DEFAULT_COLUMNS)
            .collect();
    }
    selected
}

fn truncate_text_chars(input: &str, max_chars: usize) -> (String, bool) {
    if input.chars().count() <= max_chars {
        return (input.to_string(), false);
    }
    let mut out = input.chars().take(max_chars).collect::<String>();
    out.push('…');
    (out, true)
}

fn bounded_cell_value(value: &Value, truncated_cells: &mut usize) -> Value {
    match value {
        Value::String(s) => {
            let (text, changed) = truncate_text_chars(s, DATASET_QUERY_MAX_CELL_CHARS);
            if changed {
                *truncated_cells += 1;
            }
            Value::String(text)
        }
        Value::Array(_) | Value::Object(_) => {
            let raw = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
            let (text, changed) = truncate_text_chars(&raw, DATASET_QUERY_MAX_CELL_CHARS);
            if changed {
                *truncated_cells += 1;
            }
            Value::String(text)
        }
        other => other.clone(),
    }
}

fn project_dataset_row(
    row: &Value,
    selected_columns: &[String],
    truncated_cells: &mut usize,
) -> Value {
    let mut out = serde_json::Map::new();
    if let Some(obj) = row.as_object() {
        for col in selected_columns {
            let value = obj
                .get(col)
                .map(|v| bounded_cell_value(v, truncated_cells))
                .unwrap_or(Value::Null);
            out.insert(col.clone(), value);
        }
        return Value::Object(out);
    }
    out.insert("_raw".to_string(), bounded_cell_value(row, truncated_cells));
    Value::Object(out)
}

fn build_schema_preview(dataset: &DatasetView, selected_columns: &[String]) -> Vec<Value> {
    let schema_map = dataset
        .schema
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect::<BTreeMap<_, _>>();
    selected_columns
        .iter()
        .map(|name| {
            if let Some(col) = schema_map.get(name.as_str()) {
                json!({
                    "name": col.name,
                    "type": col.type_name,
                    "source": col.source,
                    "optional": col.optional,
                })
            } else {
                json!({
                    "name": name,
                    "type": "unknown",
                })
            }
        })
        .collect()
}

/// 从已物化的 `dataset` JSON 中提取列名/类型（有界），避免模型为「有哪些字段」再去 read_file `.mei`。
fn extract_dataset_schema_preview(dataset: &Value) -> Option<Value> {
    let cols = dataset.get("columns")?.as_array()?;
    const MAX_COLS: usize = 72;
    let mut preview = Vec::new();
    for c in cols.iter().take(MAX_COLS) {
        let Some(co) = c.as_object() else {
            continue;
        };
        let name = co.get("name").and_then(Value::as_str).unwrap_or("?");
        let typ = co
            .get("type")
            .or_else(|| co.get("type_name"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        let mut row = serde_json::Map::new();
        row.insert("name".to_string(), json!(name));
        row.insert("type".to_string(), json!(typ));
        if let Some(s) = co.get("source").and_then(Value::as_str) {
            row.insert("source".to_string(), json!(s));
        }
        if let Some(o) = co.get("optional").and_then(Value::as_bool) {
            if o {
                row.insert("optional".to_string(), json!(true));
            }
        }
        preview.push(Value::Object(row));
    }
    Some(json!({
        "column_count": cols.len(),
        "columns_preview": preview,
        "columns_preview_truncated": cols.len() > MAX_COLS,
    }))
}

fn summarize_dataset_decl(dataset: &Value) -> Value {
    let len = json_serialized_len(dataset);
    let schema = extract_dataset_schema_preview(dataset);
    match dataset {
        Value::Object(m) => {
            let keys: Vec<&str> = m.keys().map(String::as_str).take(32).collect();
            let kind = m.get("kind").and_then(Value::as_str);
            let key = m.get("key").and_then(Value::as_str);
            let normalize_n = m
                .get("normalize")
                .and_then(Value::as_object)
                .map(|o| o.len())
                .unwrap_or(0);
            json!({
                "present": true,
                "approx_decl_chars": len,
                "kind": kind,
                "key": key,
                "top_level_keys_sample": keys,
                "top_level_key_count": m.len(),
                "normalize_field_count": normalize_n,
                "schema": schema,
                "note": "full dataset body omitted; `schema.columns_preview` lists declared columns (bounded). Use read_file on the entry `.mei` only when the user needs exact DSL quotes or edits — not for routine field lists."
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
        Value::Object(m) => {
            let keys: Vec<&str> = m.keys().map(String::as_str).collect();
            json!({
                "object_key_count": keys.len(),
                "keys": keys.iter().take(40).copied().collect::<Vec<_>>(),
                "approx_chars": len,
                "note": "filters object truncated to keys only."
            })
        }
        _ => json!({ "approx_chars": len, "note": "filters omitted (too large)." }),
    }
}

fn summarize_metrics_decl(metrics: &BTreeMap<String, Value>) -> Value {
    let count = metrics.len();
    let keys: Vec<&str> = metrics.keys().map(String::as_str).take(48).collect();
    json!({
        "metric_ids_sample": keys,
        "metric_id_count": count,
        "note": "metric bodies omitted; ids are enough to reason about bindings before read_file."
    })
}

/// 供 `resource_get` 与 HTTP API 使用：避免把 dataset / metrics 等大 JSON 原样塞进模型上下文。
fn summarize_resource_decl(item: &mei_lang_kernel::ResourceDecl) -> Value {
    let content_note = item.content.as_ref().map(|c| {
        if c.len() <= 800 {
            json!(c.as_str())
        } else {
            json!({
                "prefix": c.chars().take(400).collect::<String>(),
                "truncated_chars": c.len().saturating_sub(400),
            })
        }
    });
    json!({
        "_payload_shape": "resource_summary_v1",
        "id": item.id,
        "kind": item.kind,
        "title": item.title,
        "purpose": item.purpose,
        "source": item.source.as_ref().map(|s| json!({ "path": normalize_path(&s.path) })).unwrap_or(Value::Null),
        "dataset": item.dataset.as_ref().map_or(json!({ "present": false }), summarize_dataset_decl),
        "metrics": item.metrics.as_ref().map_or(Value::Null, summarize_metrics_decl),
        "filters": item.filters.as_ref().map_or(Value::Null, summarize_filters_decl),
        "content": content_note.unwrap_or(Value::Null),
    })
}

fn shrink_json_for_llm(v: &Value, max_total: usize) -> Value {
    let len = json_serialized_len(v);
    if len <= max_total {
        return v.clone();
    }
    match v {
        Value::Object(m) => {
            let mut out = serde_json::Map::new();
            for (k, val) in m.iter().take(48) {
                let elen = json_serialized_len(val);
                if elen > 2_000 {
                    out.insert(k.clone(), json!({ "_omitted": true, "approx_chars": elen }));
                } else {
                    out.insert(k.clone(), val.clone());
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
        Value::Array(a) => json!({
            "type": "array",
            "len": a.len(),
            "head": a.iter().take(5).cloned().collect::<Vec<_>>(),
        }),
        Value::String(s) => {
            let cap = 1_000usize;
            if s.len() <= cap {
                Value::String(s.clone())
            } else {
                Value::String(format!("{}…", s.chars().take(cap).collect::<String>()))
            }
        }
        other => other.clone(),
    }
}

fn build_prompt_catalog_lines(
    bundle: &WorldRuntimeBundle,
    query_tools: &[ResourceQueryToolSpec],
) -> Vec<String> {
    use std::fmt::Write as _;

    let mut lines: Vec<String> = Vec::new();
    lines.push("[World — catalog (highest-priority context)]".to_string());
    lines.push(
        "Below lists bindable world assets for this scope. When a dataset resource id is known or implied (e.g. `typical_cases`), call `dataset_query` for row/schema questions, or `dataset_metric` for aggregated questions whose metric id is already listed below (count/rate/trend/summary card asks)."
            .to_string(),
    );
    lines.push(
        "Tool-chaining guard: do NOT read_file() `.xlsx/.xls` (binary). `dataset_query` returns schema+filters+metric ids+sample rows, while `dataset_metric` returns metric values. For dataset facts, do NOT chain `read_file` / `resource_list` / `resource_runtime_peek` after a successful dataset tool call unless the user explicitly asks runtime trace or verbatim DSL edits."
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

    lines
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
        .or_else(|| Some(bundle.active_target_file.clone()));
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
        Some(bundle.active_target_file.clone()),
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
                    .or_else(|| scene_world_file_ref.clone())
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
                scene_world_file_ref
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
                    scene_world_file_ref
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
            scene_frame_file_ref
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
            scene_frame_file_ref
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

pub(crate) fn query_world_dataset(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
    columns: Option<&[String]>,
    limit: Option<usize>,
) -> Result<Value> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let dataset_id = id.trim();
    if dataset_id.is_empty() {
        return Err(anyhow!("query parameter `id` is required"));
    }
    let loaded = bundle
        .compiled
        .resources
        .iter()
        .find(|item| item.id == dataset_id)
        .ok_or_else(|| anyhow!("dataset resource `{dataset_id}` not found"))?;
    let dataset = loaded
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_id}` is not a dataset"))?;

    let row_limit = normalize_dataset_limit(limit);
    let selected_columns = normalize_dataset_columns(dataset, columns);
    let app_root = source_root.join(app_id);
    let query_options = DatasetQueryOptions {
        page: 1,
        page_size: row_limit,
        search: search
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        filters: filters.clone(),
        collect_all: false,
    };
    let query_result = query_dataset_rows(&app_root, dataset, query_options)?;

    let mut truncated_cells = 0usize;
    let sample_rows = query_result
        .rows
        .iter()
        .map(|row| project_dataset_row(row, &selected_columns, &mut truncated_cells))
        .collect::<Vec<_>>();

    let world_resource = bundle
        .contract
        .world
        .as_ref()
        .and_then(|w| w.resources.iter().find(|item| item.id == dataset_id));
    let metric_ids = world_resource
        .and_then(|item| item.metrics.as_ref())
        .map(|m| m.keys().take(64).cloned().collect::<Vec<_>>())
        .unwrap_or_else(|| dataset.metrics.keys().take(64).cloned().collect::<Vec<_>>());
    let filters_preview = world_resource
        .and_then(|item| item.filters.as_ref())
        .map(summarize_filters_decl)
        .unwrap_or(Value::Null);
    let schema_preview = build_schema_preview(dataset, &selected_columns);
    let schema_total_columns = if !dataset.schema.is_empty() {
        dataset.schema.len()
    } else {
        dataset.columns.len()
    };

    let mut payload = json!({
        "app_id": app_id,
        "scene_id": bundle.contract.scene.id,
        "id": dataset_id,
        "dataset": {
            "id": dataset.id.clone(),
            "title": dataset.title.clone(),
            "purpose": dataset.purpose.clone(),
            "source": {
                "kind": dataset.source.kind.clone(),
                "path": normalize_path(&dataset.source.path),
                "sheet": dataset.source.sheet.clone(),
            },
            "schema_preview": schema_preview,
            "schema_column_count": schema_total_columns,
            "filters": filters_preview,
            "metric_ids": metric_ids,
        },
        "sample_rows": sample_rows,
        "truncation": {
            "row_limit": row_limit,
            "column_limit": DATASET_QUERY_MAX_COLUMNS,
            "cell_char_limit": DATASET_QUERY_MAX_CELL_CHARS,
            "rows_returned": query_result.rows.len(),
            "columns_returned": selected_columns.len(),
            "cells_truncated": truncated_cells,
            "total_char_budget": DATASET_QUERY_TOTAL_CHAR_BUDGET,
            "total_chars_before_budget": 0,
            "total_chars_after_budget": 0,
        },
        "usage_hint": "若需更多数据，请在 dataset_query 中追加 filters/search/columns/limit；默认仅返回前10行与前10列的有界样例。",
    });
    let before = json_serialized_len(&payload);
    if let Some(v) = payload.pointer_mut("/truncation/total_chars_before_budget") {
        *v = json!(before);
    }
    if before > DATASET_QUERY_TOTAL_CHAR_BUDGET {
        payload = shrink_json_for_llm(&payload, DATASET_QUERY_TOTAL_CHAR_BUDGET);
    }
    let after = json_serialized_len(&payload);
    if let Some(v) = payload.pointer_mut("/truncation/total_chars_after_budget") {
        *v = json!(after);
    }
    Ok(payload)
}

pub(crate) fn query_world_dataset_metrics(
    source_root: &Path,
    app_id: &str,
    scope: Option<&WorldScope>,
    id: &str,
    metric_ids: &[String],
    search: Option<&str>,
    filters: &BTreeMap<String, String>,
) -> Result<Value> {
    let bundle = load_world_runtime_bundle(source_root, app_id, scope)?;
    let dataset_id = id.trim();
    if dataset_id.is_empty() {
        return Err(anyhow!("query parameter `id` is required"));
    }
    let loaded = bundle
        .compiled
        .resources
        .iter()
        .find(|item| item.id == dataset_id)
        .ok_or_else(|| anyhow!("dataset resource `{dataset_id}` not found"))?;
    let dataset = loaded
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow!("resource `{dataset_id}` is not a dataset"))?;
    if dataset.runtime_metric_defs.is_empty() {
        return Err(anyhow!("dataset `{dataset_id}` has no runtime metric defs"));
    }

    let app_root = source_root.join(app_id);
    let query_options = DatasetQueryOptions {
        page: 1,
        page_size: 0,
        search: search
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        filters: filters.clone(),
        collect_all: true,
    };
    let filtered_rows = query_dataset_rows(&app_root, dataset, query_options)?;

    let mut runtime_dataset = dataset.clone();
    runtime_dataset.rows = filtered_rows.rows.clone();
    if !filtered_rows.columns.is_empty() {
        runtime_dataset.columns = filtered_rows.columns.clone();
    }

    let mut datasets = bundle
        .compiled
        .resources
        .iter()
        .filter_map(|resource| {
            resource
                .dataset
                .clone()
                .map(|dataset| (resource.id.clone(), dataset))
        })
        .collect::<BTreeMap<_, _>>();
    datasets.insert(dataset_id.to_string(), runtime_dataset.clone());

    let metric_filter = if metric_ids.is_empty() {
        None
    } else {
        Some(metric_ids)
    };
    let metrics_map = evaluate_runtime_metric_defs(
        &dataset.runtime_metric_defs,
        &runtime_dataset.rows,
        &datasets,
        metric_filter,
    )?;
    let metrics = if metric_ids.is_empty() {
        metrics_map.into_values().collect::<Vec<_>>()
    } else {
        metric_ids
            .iter()
            .filter_map(|metric_id| metrics_map.get(metric_id).cloned())
            .collect::<Vec<_>>()
    };

    Ok(json!({
        "app_id": app_id,
        "scene_id": bundle.contract.scene.id,
        "dataset_id": dataset_id,
        "total_rows": runtime_dataset.rows.len(),
        "metrics": metrics,
        "usage_hint": "指标问答优先使用 dataset_metric；若要查看明细行，再改用 dataset_query。"
    }))
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

    let recent_trace_messages = recent_trace_messages(&bundle.state, 5);
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(related_to_target(
            Some("./apps/demo/main.mei"),
            Some("apps/demo/main.mei")
        ));
        assert!(!related_to_target(
            Some("apps/demo/other.mei"),
            Some("apps/demo/main.mei")
        ));
    }

    #[test]
    fn resource_summary_includes_column_preview() {
        use mei_lang_kernel::{ResourceDecl, SourceDecl};

        let dataset = json!({
            "key": "ds1",
            "kind": "dataframe",
            "columns": [
                {"name": "a", "type": "string", "optional": false},
                {"name": "b", "type": "number", "source": "B"},
            ],
            "normalize": {}
        });
        let item = ResourceDecl {
            id: "ds1".into(),
            kind: "dataset".into(),
            title: None,
            purpose: None,
            source: Some(SourceDecl {
                kind: "xlsx".into(),
                path: "data/x.xlsx".into(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            }),
            content: None,
            dataset: Some(dataset),
            metrics: None,
            filters: None,
        };
        let v = summarize_resource_decl(&item);
        let preview = v
            .pointer("/dataset/schema/columns_preview")
            .expect("columns_preview");
        let arr = preview.as_array().expect("array");
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn app_relative_mei_strips_workspace_app_prefix() {
        assert_eq!(
            app_relative_mei_for_preview("spbjw", "spbjw/data/dataset/x.mei").as_deref(),
            Some("data/dataset/x.mei")
        );
        assert_eq!(
            app_relative_mei_for_preview("spbjw", "data/dataset/x.mei").as_deref(),
            Some("data/dataset/x.mei")
        );
        assert_eq!(app_relative_mei_for_preview("spbjw", "data/x.txt"), None);
    }

    #[test]
    fn resource_get_summary_omits_huge_dataset_blob() {
        use mei_lang_kernel::{ResourceDecl, SourceDecl};

        let huge_rows: Value = json!((0..4000).map(|i| json!({"id": i})).collect::<Vec<_>>());
        let huge = json!({ "kind": "tabular", "rows": huge_rows });
        let item = ResourceDecl {
            id: "ds1".into(),
            kind: "dataset".into(),
            title: Some("Demo".into()),
            purpose: None,
            source: Some(SourceDecl {
                kind: "xlsx".into(),
                path: "data/raw/x.xlsx".into(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            }),
            content: None,
            dataset: Some(huge),
            metrics: None,
            filters: None,
        };
        let v = summarize_resource_decl(&item);
        let s = serde_json::to_string(&v).expect("json");
        assert!(
            s.len() < 4_000,
            "summary unexpectedly large: {} chars",
            s.len()
        );
        assert!(
            s.contains("approx_decl_chars"),
            "expected size metadata in summary: {s}"
        );
        assert!(
            !s.contains("\"id\":3999"),
            "expected row bodies not to be inlined: {s}"
        );
    }

    #[test]
    fn dataset_query_default_columns_cap_to_ten() {
        let dataset = DatasetView {
            id: "ds".to_string(),
            title: None,
            purpose: None,
            schema: (0..20)
                .map(|i| mei_lang_kernel::ColumnSchema {
                    name: format!("c{i}"),
                    type_name: "string".to_string(),
                    source: None,
                    optional: false,
                    unit: None,
                })
                .collect(),
            stage_schema: Vec::new(),
            columns: (0..20).map(|i| format!("c{i}")).collect(),
            rows: Vec::new(),
            source: mei_lang_kernel::SourceDecl {
                kind: "derived".to_string(),
                path: "dataset_view:ds".to_string(),
                sheet: None,
                header_row: None,
                preview_rows: None,
                page_size: None,
                max_page_size: None,
                table: None,
                query: None,
                connection: None,
                content: None,
            },
            sources: Vec::new(),
            metrics: BTreeMap::new(),
            runtime_metric_defs: BTreeMap::new(),
        };
        let cols = normalize_dataset_columns(&dataset, None);
        assert_eq!(cols.len(), 10);
        assert_eq!(cols.first().map(String::as_str), Some("c0"));
        assert_eq!(cols.last().map(String::as_str), Some("c9"));
    }

    #[test]
    fn dataset_row_projection_truncates_long_text() {
        let row = json!({
            "name": "alice",
            "long_text": "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
        });
        let mut truncated = 0usize;
        let out = project_dataset_row(
            &row,
            &["name".to_string(), "long_text".to_string()],
            &mut truncated,
        );
        assert_eq!(out.pointer("/name").and_then(Value::as_str), Some("alice"));
        let long = out
            .pointer("/long_text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(long.chars().count() <= DATASET_QUERY_MAX_CELL_CHARS + 1);
        assert!(truncated >= 1);
    }
}
