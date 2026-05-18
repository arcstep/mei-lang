//! 从请求与可选 world 快照构造 `AgentResourceScope`。

use std::sync::Arc;

use crate::agent_runtime::bridge::BridgePromptRequest;
use crate::http::scene_api::WorldContextSnapshot;
use crate::mei_agent::mode_policy::AgentModePolicy;
use crate::mei_agent::resource_tools::AgentResourceScope;

use super::reachability::ScopeReachabilitySets;
use super::visibility::resolve_resource_visibility;
use super::world_inventory_scope::allowed_world_injection_inventory_ids;

/// 从 HTTP 请求构造 Native 侧资源 scope（无 inventory 时可达集为空，read_file 在非 local 模式下会偏保守）。
pub(crate) fn agent_resource_scope_from_request(
    request: &BridgePromptRequest,
    policy: AgentModePolicy,
) -> AgentResourceScope {
    let vis = resolve_resource_visibility(request, policy);
    let app_id = request.app_id.as_deref().unwrap_or("").trim();
    let reach = if app_id.is_empty() {
        ScopeReachabilitySets::default()
    } else {
        ScopeReachabilitySets::fallback_from_request_target(request, app_id)
    };
    let (d, s) = reach.to_arc_pair();
    AgentResourceScope {
        scene_id: request.scene_id.clone(),
        target_file: request.target_file.clone(),
        resource_visibility: vis,
        direct_ref_paths: d,
        scene_reachable_paths: s,
        world_injection_allowed_ids: None,
    }
}

/// 结合 world 快照构造完整执行期 scope（推荐路径：由 HTTP 分发层在 `send_prompt` 前构建）。
pub(crate) fn agent_resource_scope_from_request_with_snapshot(
    request: &BridgePromptRequest,
    policy: AgentModePolicy,
    snapshot: Option<&WorldContextSnapshot>,
    app_id: &str,
) -> AgentResourceScope {
    let vis = resolve_resource_visibility(request, policy);
    let reach = match snapshot {
        Some(snap) => ScopeReachabilitySets::from_world_snapshot(snap, app_id),
        None => ScopeReachabilitySets::fallback_from_request_target(request, app_id),
    };
    let (d, s) = reach.to_arc_pair();
    let rs_core = AgentResourceScope {
        scene_id: request.scene_id.clone(),
        target_file: request.target_file.clone(),
        resource_visibility: vis,
        direct_ref_paths: d,
        scene_reachable_paths: s,
        world_injection_allowed_ids: None,
    };
    let world_injection_allowed_ids = snapshot.map(|snap| {
        Arc::new(allowed_world_injection_inventory_ids(
            snap, vis, &rs_core, app_id,
        ))
    });
    AgentResourceScope {
        world_injection_allowed_ids,
        ..rs_core
    }
}
