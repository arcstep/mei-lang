use std::collections::BTreeSet;
use std::fs;

use serde_json::Value;

use crate::http::scene_api::{query_resource_dataset_metric, WorldContextSnapshot};
use crate::mei_agent::browser_context::{access_browser_state, effective_filter_intents};
use crate::{agent_runtime::bridge::BridgePromptRequest, AppState};

use super::super::paths::{resolve_app_root, sanitize_relative_path};
use super::super::request_scope::world_scope_from_request;
use super::super::world_snapshot_lines::{
    append_world_context_error_lines, append_world_context_lines,
    append_world_context_snapshot_lines,
};
use super::browser::append_browser_context_lines;
use super::host::{append_host_contract_schema_line, append_host_protocol_lines};

const ASK_INLINE_TARGET_MAX_BYTES: usize = 24 * 1024;
const ACCESS_EVAL_PREVIEW_MAX_CANDIDATES: usize = 4;
const ACCESS_EVAL_PREVIEW_MAX_DATASETS: usize = 2;
const ACCESS_EVAL_PREVIEW_MAX_METRICS: usize = 6;

pub(super) fn request_mode_slug(request: &BridgePromptRequest) -> &'static str {
    let mode = request
        .mode
        .as_deref()
        .or(request.agent.as_deref())
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "build".to_string());
    if mode == "ask" || mode == "plan" {
        "ask"
    } else {
        "build"
    }
}

fn resolve_target_path_for_request(
    state: &AppState,
    app_id: &str,
    request: &BridgePromptRequest,
) -> Option<(String, std::path::PathBuf)> {
    let raw_target = request
        .target_file
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())?;
    let target_rel = sanitize_relative_path(raw_target)?;
    let mut candidates = vec![(target_rel.clone(), state.source_root.join(&target_rel))];
    let app_prefixed = format!("{app_id}/{target_rel}");
    if app_prefixed != target_rel {
        candidates.push((app_prefixed.clone(), state.source_root.join(&app_prefixed)));
    }
    candidates
        .into_iter()
        .find(|(_, full)| full.exists() && full.is_file())
}

fn truncate_value_preview(value: &Value, max_chars: usize) -> String {
    let raw = match value {
        Value::Null => "null".to_string(),
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "<invalid-json>".to_string()),
    };
    if raw.chars().count() <= max_chars {
        raw
    } else {
        format!("{}...", raw.chars().take(max_chars).collect::<String>())
    }
}

fn candidate_eval_dataset_ids(snapshot: &WorldContextSnapshot) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut ids = Vec::new();
    for item in snapshot
        .resource_inventory
        .items
        .iter()
        .filter(|item| item.resource_type == "resource" && item.related_to_target)
    {
        if seen.insert(item.id.clone()) {
            ids.push(item.id.clone());
        }
    }
    for item in snapshot
        .resource_inventory
        .items
        .iter()
        .filter(|item| item.resource_type == "resource")
    {
        if seen.insert(item.id.clone()) {
            ids.push(item.id.clone());
        }
        if ids.len() >= ACCESS_EVAL_PREVIEW_MAX_CANDIDATES {
            break;
        }
    }
    for id in &snapshot.world_snapshot.world_key_resource_ids {
        if seen.insert(id.clone()) {
            ids.push(id.clone());
        }
        if ids.len() >= ACCESS_EVAL_PREVIEW_MAX_CANDIDATES {
            break;
        }
    }
    ids
}

fn append_access_eval_preview_lines(
    lines: &mut Vec<String>,
    state: &AppState,
    app_id: &str,
    world_scope: &crate::http::scene_api::WorldScope,
    world_snapshot: Option<&WorldContextSnapshot>,
    request: &BridgePromptRequest,
) {
    let browser_state = access_browser_state(request.browser_context.as_ref());
    let Some(query_state) = browser_state.merged_query_state.as_ref() else {
        return;
    };
    lines.push(String::new());
    lines.push("[Access — default eval scope]".to_string());
    if !browser_state.active_query_state_ids.is_empty() {
        lines.push(format!(
            "active_query_state_ids: {}",
            browser_state.active_query_state_ids.join(", ")
        ));
    }
    if !query_state.filters.is_empty() {
        lines.push(format!(
            "filters: {}",
            serde_json::to_string(&query_state.filters).unwrap_or_else(|_| "{}".to_string())
        ));
    }
    if let Some(search) = query_state.search.as_deref() {
        lines.push(format!("search: {search}"));
    }
    let filter_intents = effective_filter_intents(&browser_state.filter_intents, query_state);
    if !filter_intents.is_empty() {
        let preview = filter_intents
            .iter()
            .take(8)
            .map(|intent| format!("{}={}", intent.dimension, intent.value))
            .collect::<Vec<_>>();
        lines.push(format!("filter_intents: {}", preview.join(", ")));
    }
    let Some(snapshot) = world_snapshot else {
        lines.push("evaluated_metric_previews: unavailable (world snapshot missing)".to_string());
        return;
    };
    let mut preview_lines = Vec::new();
    for dataset_id in candidate_eval_dataset_ids(snapshot)
        .into_iter()
        .take(ACCESS_EVAL_PREVIEW_MAX_CANDIDATES)
    {
        let response = query_resource_dataset_metric(
            state.source_root.as_path(),
            app_id,
            Some(world_scope),
            dataset_id.as_str(),
            &[],
            query_state.search.as_deref(),
            &query_state.filters,
            Some(query_state),
            &filter_intents,
        );
        let Ok(payload) = response else {
            continue;
        };
        let Some(metrics) = payload.get("metrics").and_then(Value::as_array) else {
            continue;
        };
        if metrics.is_empty() {
            continue;
        }
        let total_rows = payload
            .get("total_rows")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let metric_preview = metrics
            .iter()
            .take(ACCESS_EVAL_PREVIEW_MAX_METRICS)
            .filter_map(|metric| {
                let id = metric
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|item| !item.is_empty())?;
                let label = metric
                    .get("label")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|item| !item.is_empty());
                let value = truncate_value_preview(metric.get("value").unwrap_or(&Value::Null), 48);
                Some(match label {
                    Some(label) => format!("{id}({label})={value}"),
                    None => format!("{id}={value}"),
                })
            })
            .collect::<Vec<_>>();
        if metric_preview.is_empty() {
            continue;
        }
        preview_lines.push(format!(
            "dataset={} total_rows={} metrics={}",
            dataset_id,
            total_rows,
            metric_preview.join(", ")
        ));
        if preview_lines.len() >= ACCESS_EVAL_PREVIEW_MAX_DATASETS {
            break;
        }
    }
    if preview_lines.is_empty() {
        lines.push(
            "evaluated_metric_previews: none (current query state did not resolve to visible runtime metrics)"
                .to_string(),
        );
    } else {
        lines.push("evaluated_metric_previews:".to_string());
        for item in preview_lines {
            lines.push(format!("- {item}"));
        }
    }
}

pub(super) fn build_dynamic_mei_context(
    state: &AppState,
    request: &BridgePromptRequest,
    world_snapshot: Option<&WorldContextSnapshot>,
    world_snapshot_error: Option<&str>,
) -> Option<String> {
    let (app_id, _app_root) = resolve_app_root(state, request)?;
    let ask_mode = request_mode_slug(request) == "ask";
    let world_scope = world_scope_from_request(request);
    let scene_id = world_scope.scene_id.as_deref().unwrap_or("unknown");
    let mut lines = vec![
        "[MeiLang Runtime Context]".to_string(),
        format!("app: {app_id}"),
        format!("scene: {scene_id}"),
    ];
    if let Some(target) = request
        .target_file
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        if sanitize_relative_path(target).is_some() {
            lines.push(format!("target: {target}"));
        } else {
            lines.push(format!("target: {target} (invalid relative path)"));
        }
    }
    append_host_protocol_lines(&mut lines, request.host_protocol.as_ref());
    append_host_contract_schema_line(&mut lines, request.host_contract_schema.as_deref());
    append_browser_context_lines(&mut lines, request.browser_context.as_ref());
    if let Some(snapshot) = world_snapshot {
        append_world_context_snapshot_lines(&mut lines, snapshot);
    } else if let Some(message) = world_snapshot_error {
        append_world_context_error_lines(&mut lines, &app_id, message);
    } else {
        append_world_context_lines(&mut lines, &state.source_root, &app_id, &world_scope);
    }
    lines.push(String::new());
    if ask_mode {
        append_access_eval_preview_lines(
            &mut lines,
            state,
            &app_id,
            &world_scope,
            world_snapshot,
            request,
        );
        lines.push(String::new());
        lines.push(
            concat!(
                "[Ask mode — world-first]\n",
                "The active target `.mei` source is not inlined here so the prompt stays focused on the injected world/runtime catalog.\n",
                "Use `dataset_query(dataset_id)` / `dataset_metric(dataset_id)` for tabular data and metrics; use `resource_list`, `resource_get(resource_id)`, `resource_business_summary`, `resource_runtime_peek`, and `resource_runtime_trace_export` for world/runtime facts; when browser query_state is present, current prompt context already carries the default eval scope and a bounded metric preview.\n",
                "Use `read_file` only when you need verbatim DSL from a workspace path that is allowed by the current resource visibility (typically under `<app_id>/...`).",
            )
            .to_string(),
        );
    } else {
        lines.push(format!(
            "[Build mode — scene anchor] scene_id={scene_id} (source-focus file body inlined below when available)"
        ));
        lines.push(String::new());
        if let Some((target_rel, full_path)) =
            resolve_target_path_for_request(state, &app_id, request)
        {
            match fs::read_to_string(&full_path) {
                Ok(content) => {
                    let bytes = content.as_bytes();
                    let (inlined, truncated) = if bytes.len() > ASK_INLINE_TARGET_MAX_BYTES {
                        (
                            String::from_utf8_lossy(&bytes[..ASK_INLINE_TARGET_MAX_BYTES])
                                .to_string(),
                            true,
                        )
                    } else {
                        (content, false)
                    };
                    lines.push("[Build mode — current target .mei snapshot]".to_string());
                    lines.push(format!("path: {target_rel}"));
                    lines.push(format!(
                        "truncated: {} (max {} bytes)",
                        if truncated { "yes" } else { "no" },
                        ASK_INLINE_TARGET_MAX_BYTES
                    ));
                    lines.push("---".to_string());
                    lines.push(inlined);
                }
                Err(error) => {
                    lines.push(format!(
                    "[Build mode — current target .mei snapshot]\npath: {target_rel}\nerror: failed to read target file ({error})"
                ));
                }
            }
            lines.push(String::new());
            lines.push(
            "Other scene/world/frame files are indexed in the injected world/runtime catalog above, not inlined; use `read_file` within allowed paths for source-focus edits."
                .to_string(),
        );
        } else {
            lines.push(
                "[Build mode — current target .mei snapshot]\nunavailable: no valid target `.mei` in current request scope"
                    .to_string(),
            );
            lines.push(String::new());
            lines.push(
                concat!(
                    "`.mei` source is not inlined above. `read_file` paths are relative to the workspace root (parent of each app folder). ",
                    "For app-owned files use `<app_id>/...` (e.g. `examples/ds/01-dataset-baseline/scenes/...`); a bare `scenes/...` or `data/...` without app id resolves next to the workspace root and is usually wrong."
                )
                .to_string(),
            );
        }
    }
    Some(lines.join("\n"))
}
