use serde::{Deserialize, Serialize};

use crate::{
    opencode::{
        bridge::{
            list_pending_permissions as bridge_list_pending_permissions,
            respond_permission as bridge_respond_permission, BridgePendingPermission,
            BridgePermissionResponseRequest,
        },
        events::looks_like_meilang_skill_path,
    },
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

pub(crate) fn classify_blocked_permission(
    permission: &str,
    patterns: &[String],
) -> (Option<String>, bool, String) {
    let path = patterns
        .iter()
        .map(String::as_str)
        .find(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string());
    if permission == "external_directory" {
        let all_skill = !patterns.is_empty()
            && patterns
                .iter()
                .all(|pattern| looks_like_meilang_skill_path(pattern));
        if all_skill {
            return (
                path,
                true,
                "系统尝试读取 MeiLang skill 目录，但当前 OpenCode 白名单未生效；请联系管理员检查权限配置。"
                    .to_string(),
            );
        }
        return (
            path,
            true,
            "你尝试访问了未授权的文件夹。请检查任务路径是否正确；若这是系统预期目录，请联系管理员加入白名单。"
                .to_string(),
        );
    }
    (
        path,
        true,
        format!("触发了未支持的运行时授权请求（permission={permission}）。请联系管理员检查策略。"),
    )
}

fn blocked_notice_from_pending(item: BridgePendingPermission) -> HostBlockedPermissionNotice {
    let (path, requires_admin, message) =
        classify_blocked_permission(&item.permission, &item.patterns);
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
    server_url: &str,
    session_id: &str,
) -> anyhow::Result<Vec<HostBlockedPermissionNotice>> {
    let items = bridge_list_pending_permissions(&state.opencode_http, server_url).await?;
    let mut notices = Vec::new();
    for item in items
        .into_iter()
        .filter(|item| item.session_id == session_id)
    {
        let permission_id = item.id.trim().to_string();
        let mut notice = blocked_notice_from_pending(item);
        if !permission_id.is_empty() {
            match bridge_respond_permission(
                &state.opencode_http,
                server_url,
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
