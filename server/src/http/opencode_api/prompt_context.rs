use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Component, Path as FsPath, PathBuf},
};

use axum::http::StatusCode;
use walkdir::WalkDir;

use crate::{
    opencode::{
        bridge::BridgePromptRequest,
        runtime::load_managed_opencode_skill_prompt,
    },
    AppError, AppState, SessionContextSnapshot,
};

use super::super::scene_api::{
    build_world_context_snapshot, query_world_asset, query_world_assets, query_world_runtime,
    WorldScope,
};

pub(crate) fn sanitize_relative_path(value: &str) -> Option<String> {
    let mut parts = Vec::new();
    for component in FsPath::new(value).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

pub(crate) fn resolve_app_root(state: &AppState, request: &BridgePromptRequest) -> Option<(String, PathBuf)> {
    let app_id = request.app_id.as_deref()?.trim();
    if app_id.is_empty() {
        return None;
    }
    let root = state.source_root.join(app_id);
    if !root.exists() {
        return None;
    }
    Some((app_id.to_string(), root))
}

#[derive(Debug, Clone)]
struct MeiFileEntry {
    relative_path: String,
    modified_epoch_ms: u128,
}

fn collect_mei_file_entries(app_root: &FsPath) -> Vec<MeiFileEntry> {
    let mut files = WalkDir::new(app_root)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("mei"))
        .filter_map(|entry| {
            let relative_path = entry
                .path()
                .strip_prefix(app_root)
                .ok()
                .and_then(|path| path.to_str())
                .map(|path| path.replace('\\', "/"))?;
            let modified_epoch_ms = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|value| value.as_millis())
                .unwrap_or(0);
            Some(MeiFileEntry {
                relative_path,
                modified_epoch_ms,
            })
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files
}

fn build_mei_files_revision(entries: &[MeiFileEntry]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for entry in entries {
        entry.relative_path.hash(&mut hasher);
        entry.modified_epoch_ms.hash(&mut hasher);
    }
    hasher.finish()
}

fn append_world_context_lines(
    lines: &mut Vec<String>,
    source_root: &FsPath,
    app_id: &str,
    scope: &WorldScope,
) {
    let snapshot = match build_world_context_snapshot(source_root, app_id, Some(scope)) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(app_id = %app_id, %error, "failed to build world context snapshot");
            return;
        }
    };

    lines.push(String::new());
    lines.push("[World Snapshot]".to_string());
    lines.push(format!("scene_id: {}", snapshot.world_snapshot.scene_id));
    lines.push(format!("entry_target: {}", snapshot.entry_target));
    lines.push(format!(
        "world_id: {}",
        snapshot
            .world_snapshot
            .world_id
            .as_deref()
            .unwrap_or("unknown")
    ));
    lines.push(format!(
        "resource_count: {}",
        snapshot.world_snapshot.world_resource_count
    ));
    lines.push(format!(
        "entity_count: {}",
        snapshot.world_snapshot.world_entity_count
    ));
    if let Some(topology) = snapshot.world_snapshot.world_topology.as_deref() {
        lines.push(format!("topology: {topology}"));
    }
    if !snapshot
        .world_snapshot
        .world_resource_kind_counts
        .is_empty()
    {
        lines.push(format!(
            "resource_kind_counts: {}",
            serde_json::to_string(&snapshot.world_snapshot.world_resource_kind_counts)
                .unwrap_or_else(|_| "{}".to_string())
        ));
    }
    if !snapshot.world_snapshot.world_entity_kind_counts.is_empty() {
        lines.push(format!(
            "entity_kind_counts: {}",
            serde_json::to_string(&snapshot.world_snapshot.world_entity_kind_counts)
                .unwrap_or_else(|_| "{}".to_string())
        ));
    }
    if !snapshot.world_snapshot.world_key_resource_ids.is_empty() {
        lines.push(format!(
            "key_resource_ids: {}",
            snapshot.world_snapshot.world_key_resource_ids.join(", ")
        ));
    }
    if !snapshot.world_snapshot.world_key_entity_ids.is_empty() {
        lines.push(format!(
            "key_entity_ids: {}",
            snapshot.world_snapshot.world_key_entity_ids.join(", ")
        ));
    }

    lines.push(String::new());
    lines.push("[Runtime Summary]".to_string());
    lines.push(format!("phase: {}", snapshot.runtime_summary.phase));
    lines.push(format!("result: {}", snapshot.runtime_summary.result));
    lines.push(format!("countdown: {}", snapshot.runtime_summary.countdown));
    lines.push(format!(
        "scene_view_entities: {}",
        snapshot.runtime_summary.scene_view_entities
    ));
    lines.push(format!(
        "scene_view_cells: {}",
        snapshot.runtime_summary.scene_view_cells
    ));
    if !snapshot.runtime_summary.available_actions.is_empty() {
        lines.push(format!(
            "available_actions: {}",
            snapshot.runtime_summary.available_actions.join(", ")
        ));
    }
    if !snapshot.runtime_summary.recent_trace_messages.is_empty() {
        lines.push(format!(
            "recent_trace_messages: {}",
            snapshot.runtime_summary.recent_trace_messages.join(" | ")
        ));
    }

    lines.push(String::new());
    lines.push("[World Query Capabilities]".to_string());
    for capability in snapshot.query_capabilities {
        lines.push(format!(
            "- id: {} | status: {} | purpose: {}",
            capability.id, capability.status, capability.purpose
        ));
        lines.push(format!("  input: {}", capability.input));
        lines.push(format!("  output: {}", capability.output));
    }

    lines.push(String::new());
    lines.push("[World Query Skill]".to_string());
    lines.push(
        "1) 先基于 world_snapshot 与 runtime_summary 回答；如果信息不足，再选择对应 world 查询能力。"
            .to_string(),
    );
    lines.push(
        "2) 优先围绕 world 核心资产（entity/resource/topology/relation）推理，不要回退到整个工作区源码。"
            .to_string(),
    );
    lines.push(
        "3) 访问侧默认只读，不直接改写正式作者态；涉及结构修改时，先提出 session patch 建议。"
            .to_string(),
    );
    lines.push(
        "4) 如需显式触发查询，可使用 /world 指令：/world context | /world assets [kind] [limit] | /world asset <id> | /world runtime [trace_limit]。"
            .to_string(),
    );
    lines.push(
        "5) world 查询默认绑定当前 scene；当 app 内存在多个 scene 时，不要跨 scene 混合推理。"
            .to_string(),
    );
}

fn world_scope_from_request(request: &BridgePromptRequest) -> WorldScope {
    WorldScope {
        scene_id: request
            .scene_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        entry_id: request
            .entry_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        target_file: request
            .target_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    }
}

fn current_target_path(
    app_root: &FsPath,
    request: &BridgePromptRequest,
) -> Option<(String, PathBuf)> {
    let target = sanitize_relative_path(request.target_file.as_deref()?.trim())?;
    let path = app_root.join(&target);
    if !path.exists() || !path.is_file() {
        return None;
    }
    Some((target, path))
}

fn build_dynamic_mei_context(state: &AppState, request: &BridgePromptRequest) -> Option<String> {
    let (app_id, app_root) = resolve_app_root(state, request)?;
    let world_scope = world_scope_from_request(request);
    let scene_id = world_scope.scene_id.as_deref().unwrap_or("unknown");
    let entry_id = request
        .entry_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");
    let mei_entries = collect_mei_file_entries(&app_root);
    let mei_files = mei_entries
        .iter()
        .map(|item| item.relative_path.clone())
        .collect::<Vec<_>>();
    let target_context = current_target_path(&app_root, request).and_then(|(target, path)| {
        fs::read_to_string(&path)
            .ok()
            .map(|content| (target, content))
    });
    let mut lines = vec![
        "[MeiLang Runtime Context]".to_string(),
        format!("app: {app_id}"),
        format!("scene: {scene_id}"),
        format!("entry: {entry_id}"),
    ];
    if let Some((target, _)) = &target_context {
        lines.push(format!("target: {target}"));
    } else if let Some(target) = request.target_file.as_deref() {
        lines.push(format!("target: {}", target.trim()));
    }
    lines.push(
        "language: MeiLang .mei (Starlark-hosted DSL), not Music Encoding Initiative XML"
            .to_string(),
    );
    lines.push(String::new());
    lines.push("[Application Mei Files]".to_string());
    if mei_files.is_empty() {
        lines.push("- (none)".to_string());
    } else {
        lines.extend(mei_files.iter().map(|item| format!("- {item}")));
    }
    if let Some((target, content)) = target_context {
        lines.push(String::new());
        lines.push(format!("[Current File: {target}]"));
        lines.push("```mei".to_string());
        lines.push(content);
        lines.push("```".to_string());
    }
    append_world_context_lines(&mut lines, &state.source_root, &app_id, &world_scope);
    Some(lines.join("\n"))
}

fn build_context_signature(state: &AppState, request: &BridgePromptRequest) -> Option<String> {
    let (app_id, app_root) = resolve_app_root(state, request)?;
    let scene_id = request.scene_id.as_deref().map(str::trim).unwrap_or("");
    let entry_id = request.entry_id.as_deref().map(str::trim).unwrap_or("");
    let target_file = request.target_file.as_deref().map(str::trim).unwrap_or("");
    let mei_entries = collect_mei_file_entries(&app_root);
    let revision = build_mei_files_revision(&mei_entries);
    Some(format!(
        "v=world-context-v2|app={app_id}|scene={scene_id}|entry={entry_id}|target={target_file}|mei_revision={revision}"
    ))
}

pub(crate) fn load_or_refresh_session_context(
    state: &AppState,
    session_id: &str,
    request: &BridgePromptRequest,
) -> Option<String> {
    let signature = build_context_signature(state, request)?;
    {
        let Ok(cache) = state.opencode_session_context.lock() else {
            tracing::warn!("opencode session context cache lock poisoned; fallback to rebuild");
            return build_dynamic_mei_context(state, request);
        };
        if let Some(snapshot) = cache.get(session_id) {
            if snapshot.signature == signature {
                return Some(snapshot.context.clone());
            }
        }
    }
    let context = build_dynamic_mei_context(state, request)?;
    let Ok(mut cache) = state.opencode_session_context.lock() else {
        tracing::warn!("opencode session context cache lock poisoned; skip cache write");
        return Some(context);
    };
    cache.insert(
        session_id.to_string(),
        SessionContextSnapshot {
            signature,
            context: context.clone(),
        },
    );
    Some(context)
}

fn build_meilang_system_prompt(
    state: &AppState,
    existing: Option<&str>,
    session_context: Option<&str>,
) -> Option<String> {
    let mut blocks = Vec::new();
    if let Some(system) = existing.map(str::trim).filter(|value| !value.is_empty()) {
        blocks.push(system.to_string());
    }
    blocks.push(
        "You are a MeiLang authoring assistant. Treat `.mei` as MeiLang scene-first DSL hosted on restricted Starlark, not Music Encoding Initiative XML.".to_string(),
    );
    blocks.push(
        "Prefer declarative bindings: app(entries=[entry(...)]), scene(world/flow/frame=...), world(id=...), flow(id=...), frame(id=...), frame.add_panel(...).".to_string(),
    );
    blocks.push(
        "Default to Chinese (Simplified Chinese) for all responses, plans, progress updates, and explanations unless the user explicitly requests another language.".to_string(),
    );
    blocks.push(
        "When presenting a plan, keep the execution-oriented content in Chinese and avoid switching to English by default.".to_string(),
    );
    match load_managed_opencode_skill_prompt(state) {
        Ok(Some(skill_prompt)) => {
            let mut block = String::new();
            block.push_str("[MeiLang Claude Skill Entry]\n");
            block.push_str(&skill_prompt.entry_markdown);
            block.push_str("\n\n[Skill Home]\n");
            block.push_str(&format!(
                "source_kind: {}\npath: {}",
                skill_prompt.source_kind, skill_prompt.skill_home
            ));
            block.push_str(
                "\n\n[Important]\nCompanion files are relative to skill_home. Resolve them as `skill_home/<file>` before reading.",
            );
            if !skill_prompt.companion_files.is_empty() {
                block.push_str("\n\n[Companion Files]\n");
                for item in skill_prompt.companion_files {
                    block.push_str(&format!("- rel: {item}\n"));
                }
            }
            blocks.push(block.trim().to_string());
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(%error, "failed to load mei-lang skill prompt");
        }
    }
    if let Some(context) = session_context {
        blocks.push(format!("[MeiLang Session Context]\n{context}"));
    }
    if blocks.is_empty() {
        None
    } else {
        Some(blocks.join("\n\n"))
    }
}

#[derive(Debug, Clone)]
enum WorldDirective {
    Context,
    Assets {
        kind: Option<String>,
        limit: Option<usize>,
    },
    Asset {
        id: String,
    },
    Runtime {
        trace_limit: Option<usize>,
    },
}

fn world_directive_usage() -> &'static str {
    "支持的 world 指令：\
\n/world context\
\n/world assets [entity|resource|cell] [limit]\
\n/world asset <id>\
\n/world runtime [trace_limit]\
\n（默认按当前会话 scene_id / entry_id / target_file 收敛）"
}

fn parse_world_directive(text: &str) -> Result<Option<(WorldDirective, String)>, String> {
    let trimmed = text.trim();
    if !trimmed.starts_with("/world") {
        return Ok(None);
    }
    let mut lines = trimmed.lines();
    let first_line = lines.next().unwrap_or_default().trim().to_string();
    let followup = lines.collect::<Vec<_>>().join("\n").trim().to_string();
    let tokens = first_line
        .split_whitespace()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() || tokens[0] != "/world" {
        return Ok(None);
    }
    if tokens.len() < 2 {
        return Err(world_directive_usage().to_string());
    }
    let directive = match tokens[1] {
        "context" => WorldDirective::Context,
        "assets" => {
            let mut kind: Option<String> = None;
            let mut limit: Option<usize> = None;
            if let Some(arg) = tokens.get(2) {
                if let Ok(parsed) = arg.parse::<usize>() {
                    limit = Some(parsed);
                } else {
                    kind = Some((*arg).to_string());
                }
            }
            if let Some(arg) = tokens.get(3) {
                match arg.parse::<usize>() {
                    Ok(parsed) => limit = Some(parsed),
                    Err(_) => {
                        return Err(format!(
                            "无法解析 assets limit 参数 `{}`。\n{}",
                            arg,
                            world_directive_usage()
                        ));
                    }
                }
            }
            WorldDirective::Assets { kind, limit }
        }
        "asset" => {
            let id = tokens
                .get(2)
                .map(|value| value.to_string())
                .unwrap_or_default();
            if id.trim().is_empty() {
                return Err(format!(
                    "world asset 指令缺少 id 参数。\n{}",
                    world_directive_usage()
                ));
            }
            WorldDirective::Asset { id }
        }
        "runtime" => {
            let trace_limit = match tokens.get(2) {
                Some(arg) => match arg.parse::<usize>() {
                    Ok(parsed) => Some(parsed),
                    Err(_) => {
                        return Err(format!(
                            "无法解析 runtime trace_limit 参数 `{}`。\n{}",
                            arg,
                            world_directive_usage()
                        ));
                    }
                },
                None => None,
            };
            WorldDirective::Runtime { trace_limit }
        }
        _ => {
            return Err(format!(
                "不支持的 world 指令 `{}`。\n{}",
                tokens[1],
                world_directive_usage()
            ));
        }
    };
    Ok(Some((directive, followup)))
}

pub(crate) fn apply_world_directive_to_prompt(
    state: &AppState,
    request: &mut BridgePromptRequest,
) -> Result<(), AppError> {
    let parsed = parse_world_directive(&request.text).map_err(|message| {
        AppError::status(
            StatusCode::BAD_REQUEST,
            format!("world 指令解析失败：{message}"),
        )
    })?;
    let Some((directive, followup)) = parsed else {
        return Ok(());
    };

    let app_id = request
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::status(
                StatusCode::BAD_REQUEST,
                "使用 /world 指令时必须提供 app_id 上下文",
            )
        })?;
    let world_scope = world_scope_from_request(request);

    let rendered = match directive {
        WorldDirective::Context => {
            let snapshot =
                build_world_context_snapshot(&state.source_root, app_id, Some(&world_scope))
                    .map_err(|error| {
                        AppError::status(
                            StatusCode::BAD_REQUEST,
                            format!("world context 查询失败：{}", error),
                        )
                    })?;
            serde_json::to_string_pretty(&snapshot).unwrap_or_else(|_| "{}".to_string())
        }
        WorldDirective::Assets { kind, limit } => {
            let response = query_world_assets(
                &state.source_root,
                app_id,
                Some(&world_scope),
                kind.as_deref(),
                limit,
            )
            .map_err(|error| {
                AppError::status(
                    StatusCode::BAD_REQUEST,
                    format!("world assets 查询失败：{}", error),
                )
            })?;
            serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
        }
        WorldDirective::Asset { id } => {
            let response = query_world_asset(&state.source_root, app_id, Some(&world_scope), &id)
                .map_err(|error| {
                let text = error.to_string();
                if text.contains("not found") {
                    AppError::status(
                        StatusCode::NOT_FOUND,
                        format!("world asset 查询失败：{}", text),
                    )
                } else {
                    AppError::status(
                        StatusCode::BAD_REQUEST,
                        format!("world asset 查询失败：{}", text),
                    )
                }
            })?;
            serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
        }
        WorldDirective::Runtime { trace_limit } => {
            let response =
                query_world_runtime(&state.source_root, app_id, Some(&world_scope), trace_limit)
                    .map_err(|error| {
                        AppError::status(
                            StatusCode::BAD_REQUEST,
                            format!("world runtime 查询失败：{}", error),
                        )
                    })?;
            serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
        }
    };

    let mut merged = String::new();
    if !followup.is_empty() {
        merged.push_str(&followup);
        merged.push_str("\n\n");
    } else {
        merged.push_str("请基于下面的 world 查询结果回答，并给出下一步建议。\n\n");
    }
    merged.push_str("[World Query Result]\n```json\n");
    merged.push_str(&rendered);
    merged.push_str("\n```");
    request.text = merged;
    Ok(())
}

pub(crate) fn enrich_prompt_request(
    state: &AppState,
    session_context: Option<&str>,
    mut request: BridgePromptRequest,
) -> BridgePromptRequest {
    let user_text = request.text.trim().to_string();
    request.text = user_text;
    request.system =
        build_meilang_system_prompt(state, request.system.as_deref(), session_context);
    request
}
