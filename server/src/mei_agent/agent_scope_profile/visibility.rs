//! `resource_visibility` 默认值与请求字段解析。

use crate::agent_runtime::bridge::BridgePromptRequest;

use crate::mei_agent::mode_policy::{AgentMode, AgentModePolicy, RouteMode};
use crate::mei_agent::resource_tools::ResourceVisibility;

/// 根据路由与模式选择默认的资源可见策略（可被请求体显式覆盖）。
pub(crate) fn default_resource_visibility(policy: AgentModePolicy) -> ResourceVisibility {
    match (policy.route_mode, policy.mode) {
        (RouteMode::Access, AgentMode::Ask) => ResourceVisibility::AllowSceneReachable,
        (RouteMode::Manage, AgentMode::Ask) => ResourceVisibility::AllowDirectRefs,
        (RouteMode::Manage, AgentMode::Build) => ResourceVisibility::AllowDirectRefs,
        (RouteMode::Access, AgentMode::Build) => ResourceVisibility::LocalOnly,
    }
}

/// 解析并收敛 `resource_visibility`：未知值回退到默认值。
pub(crate) fn resolve_resource_visibility(
    request: &BridgePromptRequest,
    policy: AgentModePolicy,
) -> ResourceVisibility {
    ResourceVisibility::parse(request.resource_visibility.as_deref())
        .unwrap_or_else(|| default_resource_visibility(policy))
}
