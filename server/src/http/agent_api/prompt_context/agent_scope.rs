//! 统一的 Agent scope / profile 视图：把 `WorldScope`、模式策略、资源可见性与可达性收口到单一结构，
//! 供上下文预览、签名与（间接）执行期 resource scope 对齐。

use crate::agent_runtime::bridge::BridgePromptRequest;
use crate::http::scene_api::{WorldContextSnapshot, WorldScope};
use crate::mei_agent::agent_scope_profile::{resolve_resource_visibility, ScopeReachabilitySets};
use crate::mei_agent::mode_policy::{AgentModePolicy, RouteMode};
use crate::mei_agent::resource_tools::ResourceVisibility;

use super::request_scope::world_scope_from_request;

/// 与 UI `manage` / `access` 路由对应的「绑定语义」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindingKind {
    /// 管理页：以当前编辑文件为中心。
    File,
    /// 访问页：以当前场景为中心。
    Scene,
}

/// 规范化后的 agent scope profile（单次请求视角）。
#[derive(Debug, Clone)]
pub(crate) struct AgentScopeProfile {
    pub world_scope: WorldScope,
    pub mode_policy: AgentModePolicy,
    pub resource_visibility: ResourceVisibility,
    pub binding_kind: BindingKind,
    pub reachability: ScopeReachabilitySets,
}

impl AgentScopeProfile {
    pub(crate) fn from_request_and_snapshot(
        request: &BridgePromptRequest,
        snapshot: Option<&WorldContextSnapshot>,
        app_id: &str,
    ) -> Self {
        let mode_policy = AgentModePolicy::from_request(request);
        let resource_visibility = resolve_resource_visibility(request, mode_policy);
        let world_scope = world_scope_from_request(request);
        let binding_kind = match mode_policy.route_mode {
            RouteMode::Manage => BindingKind::File,
            RouteMode::Access => BindingKind::Scene,
        };
        let reachability = match snapshot {
            Some(s) => ScopeReachabilitySets::from_world_snapshot(s, app_id),
            None => ScopeReachabilitySets::fallback_from_request_target(request, app_id),
        };
        Self {
            world_scope,
            mode_policy,
            resource_visibility,
            binding_kind,
            reachability,
        }
    }

    pub(crate) fn reach_digest(&self) -> String {
        self.reachability.digest_short()
    }

    /// 供 UI 展示的短摘要（单行）。
    pub(crate) fn summary_line(&self) -> String {
        let bind = match self.binding_kind {
            BindingKind::File => "file(manage)",
            BindingKind::Scene => "scene(access)",
        };
        let scene = self.world_scope.scene_id.as_deref().unwrap_or("-");
        let tgt = self.world_scope.target_file.as_deref().unwrap_or("-");
        format!(
            "profile: binding={bind} | mode={} | route={} | visibility={} | reach={} | scene={scene} | file={tgt}",
            self.mode_policy.mode.as_str(),
            self.mode_policy.route_mode.as_str(),
            self.resource_visibility.as_slug(),
            self.reach_digest()
        )
    }
}
