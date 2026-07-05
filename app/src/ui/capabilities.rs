use serde::{Deserialize, Serialize};

/// 宿主 UI 与 HTTP API 共用的能力真源（由角色推导；`--auth` 关闭时全开）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCapabilities {
    /// 访问视图 `/apps/app/*`
    pub access_view: bool,
    /// 配置/上传视图及 `/api/ops/*`、`/api/upload/*`
    pub config_upload: bool,
    /// 应用访问面 `/apps/{id}/app`
    pub build_view: bool,
    /// 访问侧 Agent（session、message、probe、context/preview 等）
    pub access_agent: bool,
    /// 作者侧写回（diff/revert/unrevert）；handler 已统一 410，仅保留鉴权矩阵与过渡 UI 开关。
    pub authoring_agent: bool,
    /// Agent 进程控制（start/stop/skill sync）
    pub agent_control: bool,
    /// 运行态组件脚本 `/workspace-components/*`
    pub runtime_components: bool,
}

impl Default for HostCapabilities {
    fn default() -> Self {
        Self::auth_disabled()
    }
}

impl HostCapabilities {
    pub fn auth_disabled() -> Self {
        Self {
            access_view: true,
            config_upload: true,
            build_view: true,
            access_agent: true,
            authoring_agent: true,
            agent_control: true,
            runtime_components: true,
        }
    }

    pub fn from_role_slug(role: &str) -> Self {
        match role {
            "super" => Self {
                access_view: true,
                config_upload: true,
                build_view: true,
                access_agent: true,
                authoring_agent: true,
                agent_control: true,
                runtime_components: true,
            },
            "admin" => Self {
                access_view: true,
                config_upload: true,
                build_view: false,
                access_agent: true,
                authoring_agent: false,
                agent_control: false,
                runtime_components: true,
            },
            _ => Self {
                access_view: true,
                config_upload: false,
                build_view: false,
                access_agent: true,
                authoring_agent: false,
                agent_control: false,
                runtime_components: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HostCapabilities;

    #[test]
    fn role_matrix_matches_four_view_policy() {
        let guest = HostCapabilities::from_role_slug("guest");
        assert!(guest.access_view && guest.access_agent && guest.runtime_components);
        assert!(!guest.config_upload && !guest.build_view && !guest.authoring_agent);

        let admin = HostCapabilities::from_role_slug("admin");
        assert!(admin.config_upload && !admin.build_view);

        let super_user = HostCapabilities::from_role_slug("super");
        assert!(super_user.build_view && super_user.agent_control);
    }
}
