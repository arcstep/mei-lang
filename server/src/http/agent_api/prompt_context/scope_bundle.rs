//! 单次 Agent 请求的 canonical scope/snapshot 收口：`WorldContextSnapshot`、`AgentScopeProfile`、`AgentResourceScope` 同源构建。

use std::hash::{Hash, Hasher};

use crate::agent_runtime::bridge::BridgePromptRequest;
use crate::http::scene_api::{build_world_context_snapshot, WorldContextSnapshot, WorldScope};
use crate::mei_agent::agent_scope_profile::agent_resource_scope_from_request_with_snapshot;
use crate::mei_agent::mode_policy::AgentModePolicy;
use crate::mei_agent::resource_tools::AgentResourceScope;
use crate::AppState;

use super::agent_scope::AgentScopeProfile;
use super::paths::resolve_app_root;
use super::request_scope::world_scope_from_request;

/// Preview / send / session context / world directive 共用的 scope 包。
#[derive(Debug, Clone)]
pub(crate) struct AgentScopeBundle {
    pub app_id: String,
    pub world_scope: WorldScope,
    pub snapshot: Option<WorldContextSnapshot>,
    pub snapshot_error: Option<String>,
    pub profile: AgentScopeProfile,
    pub resource_scope: AgentResourceScope,
    /// 与 `dynamic_context` 缓存签名中的 reach 段一致（无快照时为 `na`）。
    pub reach_digest: String,
    /// 浏览器访问态上下文摘要（无浏览器上下文时为 `na`）。
    pub browser_digest: String,
    /// 宿主 runtime protocol 摘要（无协议元数据时为 `na`）。
    pub host_protocol_digest: String,
    /// 宿主 contract schema 摘要（无声明时为 `na`）。
    pub host_contract_schema_digest: String,
}

fn browser_context_digest(request: &BridgePromptRequest) -> String {
    let Some(value) = request.browser_context.as_ref() else {
        return "na".to_string();
    };
    let Ok(raw) = serde_json::to_vec(value) else {
        return "invalid".to_string();
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn host_protocol_digest(request: &BridgePromptRequest) -> String {
    let Some(value) = request.host_protocol.as_ref() else {
        return "na".to_string();
    };
    let Ok(raw) = serde_json::to_vec(value) else {
        return "invalid".to_string();
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    raw.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn host_contract_schema_digest(request: &BridgePromptRequest) -> String {
    let schema = request
        .host_contract_schema
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("na")
        .to_string();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    schema.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

impl AgentScopeBundle {
    /// 构建 canonical bundle；无有效 `app_id` 或工作区无该 app 目录时返回 `None`。
    pub(crate) fn resolve(state: &AppState, request: &BridgePromptRequest) -> Option<Self> {
        let (app_id, _) = resolve_app_root(state, request)?;
        let policy = AgentModePolicy::from_request(request);
        let world_scope = world_scope_from_request(request);
        let snap_res = build_world_context_snapshot(
            state.source_root.as_path(),
            app_id.as_str(),
            Some(&world_scope),
        );
        let (snapshot, snapshot_error) = match snap_res {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let profile = AgentScopeProfile::from_request_and_snapshot(
            request,
            snapshot.as_ref(),
            app_id.as_str(),
        );
        let reach_digest = profile.reach_digest();
        let browser_digest = browser_context_digest(request);
        let host_protocol_digest = host_protocol_digest(request);
        let host_contract_schema_digest = host_contract_schema_digest(request);
        let resource_scope = match snapshot.as_ref() {
            Some(s) => agent_resource_scope_from_request_with_snapshot(
                request,
                policy,
                Some(s),
                app_id.as_str(),
            ),
            None => agent_resource_scope_from_request_with_snapshot(
                request,
                policy,
                None,
                app_id.as_str(),
            ),
        };
        Some(Self {
            app_id,
            world_scope,
            snapshot,
            snapshot_error,
            profile,
            resource_scope,
            reach_digest,
            browser_digest,
            host_protocol_digest,
            host_contract_schema_digest,
        })
    }

    /// 供 `BridgePromptSummary` 等发送链可观测字段。
    pub(crate) fn scope_digest_token(&self) -> String {
        format!(
            "reach={}|vis={}|mode={}|route={}|browser={}|host_protocol={}|host_contract_schema={}",
            self.reach_digest,
            self.profile.resource_visibility.as_slug(),
            self.profile.mode_policy.mode.as_str(),
            self.profile.mode_policy.route_mode.as_str(),
            self.browser_digest,
            self.host_protocol_digest,
            self.host_contract_schema_digest,
        )
    }
}
