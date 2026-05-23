use axum::http::StatusCode;

use crate::{
    agent_runtime::bridge::BridgePromptRequest,
    mei_agent::agent_scope_profile::{
        allowed_world_injection_inventory_ids, world_injection_inventory_item_allowed,
    },
    AppError, AppState,
};

use super::scope_bundle::AgentScopeBundle;
use crate::http::scene_api::{
    query_world_asset, query_world_assets, query_world_runtime, WorldContextSnapshot,
};

/// `/world` 注入 JSON 的最大字符数，避免冲掉用户主提示。
const WORLD_DIRECTIVE_MAX_JSON_CHARS: usize = 64_000;

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
\n（默认按当前会话 scene_id / target_file 收敛；受 resource_visibility 约束）"
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

fn filter_world_context_snapshot_for_injection(
    snap: &WorldContextSnapshot,
    vis: crate::mei_agent::resource_tools::ResourceVisibility,
    rs: &crate::mei_agent::resource_tools::AgentResourceScope,
    app_id: &str,
) -> WorldContextSnapshot {
    let allowed = allowed_world_injection_inventory_ids(snap, vis, rs, app_id);
    let mut out = snap.clone();
    out.resource_inventory.items = snap
        .resource_inventory
        .items
        .iter()
        .filter(|it| world_injection_inventory_item_allowed(it, vis, rs, app_id))
        .cloned()
        .collect();
    out.resource_inventory.total_items = out.resource_inventory.items.len();
    out.world_snapshot
        .world_key_resource_ids
        .retain(|id| allowed.contains(id));
    out.world_snapshot
        .world_key_entity_ids
        .retain(|id| allowed.contains(id));
    out
}

pub(crate) fn truncate_world_json(rendered: String) -> String {
    if rendered.len() <= WORLD_DIRECTIVE_MAX_JSON_CHARS {
        return rendered;
    }
    let preview: String = rendered
        .chars()
        .take(WORLD_DIRECTIVE_MAX_JSON_CHARS)
        .collect();
    format!(
        "{{\"truncated\":true,\"original_chars\":{},\"preview\":{}}}",
        rendered.len(),
        serde_json::to_string(&preview).unwrap_or_else(|_| "\"\"".into())
    )
}

/// 将 `/world ...` 首行展开为内联 JSON 提示；与工具链共用 canonical `AgentScopeBundle`。
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

    let bundle = AgentScopeBundle::resolve(state, request).ok_or_else(|| {
        AppError::status(
            StatusCode::BAD_REQUEST,
            "使用 /world 指令时需要有效的 app_id 且工作区存在对应应用目录",
        )
    })?;

    if bundle.profile.resource_visibility
        == crate::mei_agent::resource_tools::ResourceVisibility::LocalOnly
    {
        return Err(AppError::status(
            StatusCode::FORBIDDEN,
            "当前 resource_visibility=local_only，不允许使用 /world 注入大块 world 数据（与 read_file/dataset 的收敛策略一致）。\
请切换到「直接引用」或「场景可达」，或改用 dataset_query / dataset_metric / read_file。",
        ));
    }

    let app_id = bundle.app_id.as_str();
    let scope_ref = Some(&bundle.world_scope);

    let vis = bundle.profile.resource_visibility;
    let rs = &bundle.resource_scope;

    let rendered = match directive {
        WorldDirective::Context => {
            let snap = bundle.snapshot.as_ref().ok_or_else(|| {
                let msg = bundle
                    .snapshot_error
                    .clone()
                    .unwrap_or_else(|| "world 上下文不可用".to_string());
                AppError::status(
                    StatusCode::BAD_REQUEST,
                    format!("world context 不可用：{msg}"),
                )
            })?;
            let filtered = filter_world_context_snapshot_for_injection(snap, vis, rs, app_id);
            serde_json::to_string_pretty(&filtered).unwrap_or_else(|_| "{}".to_string())
        }
        WorldDirective::Assets { kind, limit } => {
            let snap = bundle.snapshot.as_ref().ok_or_else(|| {
                AppError::status(
                    StatusCode::FORBIDDEN,
                    "scope_denied: 缺少 world snapshot，无法按 resource_visibility 过滤 world assets 列表",
                )
            })?;
            let allowed = allowed_world_injection_inventory_ids(snap, vis, rs, app_id);
            let mut response = query_world_assets(
                &state.source_root,
                app_id,
                scope_ref,
                kind.as_deref(),
                limit,
            )
            .map_err(|error| {
                AppError::status(
                    StatusCode::BAD_REQUEST,
                    format!("world assets 查询失败：{}", error),
                )
            })?;
            response.items.retain(|it| allowed.contains(&it.id));
            response.total = response.items.len();
            serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
        }
        WorldDirective::Asset { id } => {
            let snap = bundle.snapshot.as_ref().ok_or_else(|| {
                AppError::status(
                    StatusCode::FORBIDDEN,
                    "scope_denied: 缺少 world snapshot，无法按 resource_visibility 校验 world asset",
                )
            })?;
            let allowed = allowed_world_injection_inventory_ids(snap, vis, rs, app_id);
            let tid = id.trim();
            if !allowed.contains(tid) {
                return Err(AppError::status(
                    StatusCode::FORBIDDEN,
                    format!(
                        "scope_denied: world asset `{tid}` 不在当前 `{}` 可达 inventory 中；请使用 dataset_query / dataset_metric / read_file",
                        vis.as_slug()
                    ),
                ));
            }
            let response = query_world_asset(&state.source_root, app_id, scope_ref, &id).map_err(
                |e: anyhow::Error| {
                    let text = e.to_string();
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
                },
            )?;
            serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
        }
        WorldDirective::Runtime { trace_limit } => {
            bundle.snapshot.as_ref().ok_or_else(|| {
                AppError::status(
                    StatusCode::FORBIDDEN,
                    "scope_denied: 缺少 world snapshot，无法按 resource_visibility 验证 world runtime 摘要",
                )
            })?;
            let response = query_world_runtime(&state.source_root, app_id, scope_ref, trace_limit)
                .map_err(|error| {
                    AppError::status(
                        StatusCode::BAD_REQUEST,
                        format!("world runtime 查询失败：{}", error),
                    )
                })?;
            serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
        }
    };

    let rendered = truncate_world_json(rendered);

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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn parse_world_none_for_normal_chat() {
        assert!(parse_world_directive("hello").unwrap().is_none());
    }

    #[test]
    fn world_directive_blocked_for_local_only() {
        let state = crate::test_support::test_app_state().expect("app state");
        let mut request = BridgePromptRequest {
            text: "/world context\n".into(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("local_only".into()),
        };
        let err = apply_world_directive_to_prompt(&state, &mut request).unwrap_err();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn world_runtime_scope_denied_without_snapshot() {
        let state = crate::test_support::test_app_state().expect("app state");
        let mut request = BridgePromptRequest {
            text: "/world runtime\n".into(),
            app_id: Some("examples/core/_invalid/07-app-missing-scene".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
        };
        let err = apply_world_directive_to_prompt(&state, &mut request).unwrap_err();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn world_runtime_inlines_bounded_json_for_valid_app() {
        let state = crate::test_support::test_app_state().expect("app state");
        let mut request = BridgePromptRequest {
            text: "/world runtime\n".into(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
        };
        apply_world_directive_to_prompt(&state, &mut request).expect("runtime directive");
        assert!(request.text.contains("[World Query Result]"));
        assert!(request.text.contains("app_id"));
    }

    #[test]
    fn world_assets_list_respects_inventory_filter() {
        let state = crate::test_support::test_app_state().expect("app state");
        let mut request = BridgePromptRequest {
            text: "/world assets all 200\n".into(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
        };
        apply_world_directive_to_prompt(&state, &mut request).expect("assets directive");
        let marker = "[World Query Result]";
        let idx = request.text.find(marker).expect("marker");
        let json_block = &request.text[idx + marker.len()..];
        assert!(
            json_block.contains("\"total\":") && json_block.contains("\"items\""),
            "expected JSON list shape"
        );
    }

    #[test]
    fn truncate_world_json_inserts_truncation_marker() {
        let huge = "x".repeat(WORLD_DIRECTIVE_MAX_JSON_CHARS + 500);
        let out = truncate_world_json(huge.clone());
        assert!(out.contains("\"truncated\":true"));
        assert!(out.len() < huge.len());
    }

    #[test]
    fn world_asset_scope_denied_when_id_not_in_allowed_inventory() {
        let state = crate::test_support::test_app_state().expect("app state");
        let mut request = BridgePromptRequest {
            text: "/world asset __definitely_not_allowed_id__\n".into(),
            app_id: Some("examples/core/01-single-file-doc".into()),
            scene_id: None,
            target_file: Some("main.mei".into()),
            system: None,
            mode: Some("ask".into()),
            route_mode: Some("manage".into()),
            agent: None,
            model: None,
            resource_visibility: Some("allow_direct_refs".into()),
        };
        let err = apply_world_directive_to_prompt(&state, &mut request).unwrap_err();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn parse_world_context() {
        let (d, follow) = parse_world_directive("/world context\nhi")
            .unwrap()
            .unwrap();
        assert!(follow.contains("hi"));
        match d {
            WorldDirective::Context => {}
            _ => panic!("expected context"),
        }
    }
}
