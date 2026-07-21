//! Status copy for `/host/starting` and access-readiness polls.
//!
//! Distinguish **waiting** (enabled → demand-load in seconds) from **blocked**
//! (not enabled / misconfigured → will never auto-load).

use mei_host_core::DesiredState;

use crate::app_runtime_supervisor::AppRuntimeSupervisor;
use crate::state::ShellState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessGateKind {
    /// Enabled (or in-flight); user should wait — page will auto-enter.
    Waiting,
    /// Will not become ready without operator action.
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessGateStatus {
    pub title: &'static str,
    pub hint: &'static str,
    pub kind: AccessGateKind,
}

impl AccessGateStatus {
    pub fn waiting(title: &'static str, hint: &'static str) -> Self {
        Self {
            title,
            hint,
            kind: AccessGateKind::Waiting,
        }
    }

    pub fn blocked(title: &'static str, hint: &'static str) -> Self {
        Self {
            title,
            hint,
            kind: AccessGateKind::Blocked,
        }
    }

    pub fn kind_slug(self) -> &'static str {
        match self.kind {
            AccessGateKind::Waiting => "waiting",
            AccessGateKind::Blocked => "blocked",
        }
    }
}

/// Status shown in the starting / gate card.
pub fn resolve_access_gate_status(
    shell: &ShellState,
    supervisor: Option<&AppRuntimeSupervisor>,
    app_id: &str,
    readiness_reason: &str,
) -> AccessGateStatus {
    if shell.startup_error.is_some() || readiness_reason == "failed" {
        return AccessGateStatus::blocked(
            "启动未完成",
            "宿主启动失败，不会自动恢复。请查看日志或重新 prebuild。",
        );
    }
    let app_label = app_id.trim();
    let enabled = !app_label.is_empty() && shell.enabled_apps.contains(app_label);

    match readiness_reason {
        // Control-plane first boot keeps data_plane_enabled=false until a runtime cutover.
        // Enabled apps are waiting on demand-load — never treat that as「工作区尚未配置」.
        "unconfigured" if !enabled => {
            return AccessGateStatus::blocked(
                "工作区尚未配置",
                "控制面未就绪，应用不会自动载入。请先完成工作区配置。",
            );
        }
        "disabled" if !enabled => {
            return AccessGateStatus::blocked(
                "应用数据面已关闭",
                "当前数据面不可用，请检查宿主配置后再访问。",
            );
        }
        _ => {}
    }

    if !enabled {
        return AccessGateStatus::blocked(
            "应用未启用",
            "该应用不在启用清单中，不会自动载入。请到应用中心启用后再访问。",
        );
    }

    if shell
        .ops_job
        .as_ref()
        .is_some_and(crate::build_ops::OpsJobState::is_running)
    {
        return AccessGateStatus::waiting(
            "应用载入中",
            "正在编译/准备产物，通常数秒后自动进入。",
        );
    }

    let route = shell.launch_manifest.routes.get(app_label);
    let active_instance = route.and_then(|r| r.active.as_deref());
    let runtime_up = active_instance.is_some_and(|id| {
        shell.app_runtime_by_instance.contains_key(id)
            || supervisor.is_some_and(|s| s.runtime_for(id).is_some())
    });
    if !runtime_up {
        let desired = active_instance.and_then(|id| {
            shell
                .launch_manifest
                .instances
                .get(id)
                .map(|inst| inst.desired_state)
        });
        return match desired {
            Some(DesiredState::Standby) => AccessGateStatus::waiting(
                "应用载入中",
                "运行时正在切换，就绪后将自动进入。",
            ),
            _ => AccessGateStatus::waiting(
                "应用载入中",
                "正在拉起运行时，通常数秒后自动进入。",
            ),
        };
    }

    match readiness_reason {
        "importing" => AccessGateStatus::waiting(
            "应用载入中",
            "正在装载应用数据，就绪后将自动进入。",
        ),
        "warming" | "assembling" => AccessGateStatus::waiting(
            "应用载入中",
            "访问面准备中，通常数秒后自动进入。",
        ),
        "plug_ds" => AccessGateStatus::waiting(
            "应用载入中",
            "数据侧车即将就绪，请稍候。",
        ),
        "runtime_starting" => AccessGateStatus::waiting(
            "应用载入中",
            "运行时正在启动，就绪后将自动进入。",
        ),
        _ => AccessGateStatus::waiting(
            "应用载入中",
            "即将就绪，请稍候。",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn empty_shell(data_plane: bool) -> ShellState {
        let mut shell = ShellState::new(
            PathBuf::from("/tmp/ws"),
            "zhifa".to_string(),
            PathBuf::from("/tmp/pkg"),
            BTreeMap::new(),
            false,
        );
        shell.data_plane_enabled = data_plane;
        shell
    }

    #[test]
    fn unconfigured_is_blocked() {
        let shell = empty_shell(false);
        let status = resolve_access_gate_status(&shell, None, "zhifa", "unconfigured");
        assert_eq!(status.title, "工作区尚未配置");
        assert_eq!(status.kind, AccessGateKind::Blocked);
    }

    #[test]
    fn unconfigured_but_enabled_is_waiting_for_demand_load() {
        let mut shell = empty_shell(false);
        shell.enabled_apps.insert("zhifa".into());
        let status = resolve_access_gate_status(&shell, None, "zhifa", "unconfigured");
        assert_eq!(status.title, "应用载入中");
        assert_eq!(status.kind, AccessGateKind::Waiting);
    }

    #[test]
    fn not_enabled_is_blocked_never_loads() {
        let shell = empty_shell(true);
        let status = resolve_access_gate_status(&shell, None, "zhifa", "warming");
        assert_eq!(status.title, "应用未启用");
        assert_eq!(status.kind, AccessGateKind::Blocked);
        assert!(status.hint.contains("不会自动载入"));
    }

    #[test]
    fn enabled_without_runtime_is_waiting_load() {
        let mut shell = empty_shell(true);
        shell.enabled_apps.insert("zhifa".into());
        let status = resolve_access_gate_status(&shell, None, "zhifa", "warming");
        assert_eq!(status.title, "应用载入中");
        assert_eq!(status.kind, AccessGateKind::Waiting);
        assert!(status.hint.contains("数秒") || status.hint.contains("稍候") || status.hint.contains("自动进入"));
    }

    #[test]
    fn running_build_job_reports_loading_while_enabled() {
        let mut shell = empty_shell(true);
        shell.enabled_apps.insert("zhifa".into());
        shell.ops_job = Some(crate::build_ops::OpsJobState::running("prebuild", 1));
        let status = resolve_access_gate_status(&shell, None, "zhifa", "runtime_starting");
        assert_eq!(status.title, "应用载入中");
        assert_eq!(status.kind, AccessGateKind::Waiting);
    }
}
