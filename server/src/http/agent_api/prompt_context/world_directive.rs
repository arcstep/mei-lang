use axum::http::StatusCode;

use crate::{
    agent_runtime::bridge::BridgePromptRequest,
    AppError, AppState,
};

use super::request_scope::world_scope_from_request;
use super::super::super::scene_api::{
    build_world_context_snapshot, query_world_asset, query_world_assets, query_world_runtime,
};

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
\n（默认按当前会话 scene_id / target_file 收敛）"
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
