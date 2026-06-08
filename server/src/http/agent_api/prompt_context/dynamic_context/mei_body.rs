use std::fs;

use crate::http::scene_api::WorldContextSnapshot;
use crate::{agent_runtime::bridge::BridgePromptRequest, AppState};

use super::super::paths::{resolve_app_root, sanitize_relative_path};
use super::super::request_scope::world_scope_from_request;
use super::super::world_snapshot_lines::{append_world_context_error_lines, append_world_context_lines, append_world_context_snapshot_lines};
use super::browser::append_browser_context_lines;
use super::host::{append_host_contract_schema_line, append_host_protocol_lines};

const ASK_INLINE_TARGET_MAX_BYTES: usize = 24 * 1024;

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
        lines.push(
            concat!(
                "[Ask mode — world-first]\n",
                "The active target `.mei` source is not inlined here so the prompt stays focused on the injected world/runtime catalog.\n",
                "Use `dataset_query` / `dataset_metric` for tabular data and metrics.\n",
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
                    "For app-owned files use `<app_id>/...` (e.g. `spbjw/scenes/行政检查/datasets/...`); a bare `scenes/...` or `data/...` without app id resolves next to the workspace root and is usually wrong."
                )
                .to_string(),
            );
        }
    }
    Some(lines.join("\n"))
}

