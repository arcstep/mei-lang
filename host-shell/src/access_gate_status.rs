//! Short title for `/host/starting` — one line only, no extra actions/copy.

use mei_host_core::DesiredState;

use crate::app_runtime_supervisor::AppRuntimeSupervisor;
use crate::state::ShellState;

/// One-line status title shown in the starting card.
pub fn resolve_access_gate_title(
    shell: &ShellState,
    supervisor: Option<&AppRuntimeSupervisor>,
    app_id: &str,
    readiness_reason: &str,
) -> &'static str {
    if shell.startup_error.is_some() || readiness_reason == "failed" {
        return "启动未完成";
    }
    match readiness_reason {
        "unconfigured" => return "工作区尚未配置",
        "disabled" => return "应用数据面已关闭",
        _ => {}
    }
    if shell
        .ops_job
        .as_ref()
        .is_some_and(crate::build_ops::OpsJobState::is_running)
    {
        return "应用正在启动";
    }

    let app_label = app_id.trim();
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
            Some(DesiredState::Running) => "应用正在启动",
            Some(DesiredState::Standby) => "应用处于待机",
            _ => "应用未启动",
        };
    }

    match readiness_reason {
        "importing" => "正在装载应用",
        "warming" | "assembling" => "访问面准备中",
        "plug_ds" => "数据侧车未就绪",
        _ => "应用暂不可用",
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
    fn unconfigured_title_is_not_generic_preparing() {
        let shell = empty_shell(false);
        assert_eq!(
            resolve_access_gate_title(&shell, None, "zhifa", "unconfigured"),
            "工作区尚未配置"
        );
    }

    #[test]
    fn missing_runtime_title_is_app_stopped() {
        let shell = empty_shell(true);
        assert_eq!(
            resolve_access_gate_title(&shell, None, "zhifa", "warming"),
            "应用未启动"
        );
    }

    #[test]
    fn running_build_job_reports_app_starting_before_route_exists() {
        let mut shell = empty_shell(true);
        shell.ops_job = Some(crate::build_ops::OpsJobState::running("prebuild", 1));
        assert_eq!(
            resolve_access_gate_title(&shell, None, "zhifa", "runtime_starting"),
            "应用正在启动"
        );
    }
}
