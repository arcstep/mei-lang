use serde::{Deserialize, Serialize};

use crate::{
    mei_agent::{
        agent_list_pending_permissions, agent_respond_permission, permission_policy, AgentConn,
    },
    opencode::bridge::{BridgePendingPermission, BridgePermissionResponseRequest},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct SessionMessagesQuery {
    pub limit: Option<usize>,
}

const DEFAULT_SESSION_MESSAGES_LIMIT: usize = 80;
const MAX_SESSION_MESSAGES_LIMIT: usize = 300;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostBlockedPermissionNotice {
    permission_id: String,
    permission: String,
    path: Option<String>,
    patterns: Vec<String>,
    requires_admin: bool,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HostBlockedPermissionList {
    pub(crate) session_id: String,
    pub(crate) pending: Vec<HostBlockedPermissionNotice>,
}

pub(crate) fn normalize_session_messages_limit(limit: Option<usize>) -> usize {
    let resolved = limit.unwrap_or(DEFAULT_SESSION_MESSAGES_LIMIT);
    resolved.clamp(1, MAX_SESSION_MESSAGES_LIMIT)
}

fn blocked_notice_from_pending(item: BridgePendingPermission) -> HostBlockedPermissionNotice {
    let (path, requires_admin, message) =
        permission_policy::classify_blocked_permission(&item.permission, &item.patterns);
    HostBlockedPermissionNotice {
        permission_id: item.id,
        permission: item.permission,
        path,
        patterns: item.patterns,
        requires_admin,
        message,
    }
}

pub(crate) async fn collect_and_reject_blocked_permissions(
    state: &AppState,
    conn: &AgentConn,
    session_id: &str,
) -> anyhow::Result<Vec<HostBlockedPermissionNotice>> {
    let items: Vec<BridgePendingPermission> = agent_list_pending_permissions(state, conn).await?;
    let mut notices = Vec::new();
    for item in items
        .into_iter()
        .filter(|item| item.session_id == session_id)
    {
        let permission_id = item.id.trim().to_string();
        let mut notice = blocked_notice_from_pending(item);
        if !permission_id.is_empty() {
            match agent_respond_permission(
                state,
                conn,
                session_id,
                &permission_id,
                BridgePermissionResponseRequest {
                    response: "reject".to_string(),
                },
            )
            .await
            {
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(
                        session_id = %session_id,
                        permission_id = %permission_id,
                        %error,
                        "failed to auto-reject pending opencode permission"
                    );
                    notice.message = format!("{}（自动拒绝失败：{}）", notice.message, error);
                }
            }
        }
        notices.push(notice);
    }
    Ok(notices)
}
