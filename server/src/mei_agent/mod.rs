//! 内置 Agent：会话存储、流式对话、与 `/api/agent/*` 分发层对接。

pub(crate) mod agent_scope_profile;
pub(crate) mod browser_context;
pub(crate) mod dispatch;
mod llm;
pub(crate) mod llm_config;
pub(crate) mod mode_policy;
pub mod native;
pub(crate) mod permission_policy;
pub(crate) mod resource_tools;
mod workspace_snapshot_git;

pub(crate) use dispatch::{
    agent_abort_session, agent_create_session, agent_health, agent_list_pending_permissions,
    agent_list_sessions, agent_project_worktree, agent_respond_permission, agent_send_prompt,
    agent_session_messages, agent_vcs_summary, resolve_agent_conn, AgentConn,
};
pub(crate) use native::NativeAgent;
