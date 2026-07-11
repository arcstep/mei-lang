//! CLI 离线 compile / reload 与运行中 host 的热同步。
//!
//! 常态：另一个终端改 `.mei` 并 compile（或 `deploy/reload.sh`），已运行的 host
//! 应自动 import + invalidate + warmup，把 client-bootstrap 写回，而不是卡在
//! `manifest_missing` / warming。

use std::path::Path;
use std::time::{Duration, SystemTime};

use mei_host_core::HostContext;

use crate::build_ops::{
    begin_ops_job, finish_ops_job_failure, finish_ops_job_success, import_with_options,
    rewarm_after_import,
};
use crate::state::SharedState;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const BUNDLE_WRITE_DEBOUNCE: Duration = Duration::from_secs(1);
const DEFAULT_WARMUP_POLICY: &str = "standard";

pub fn hot_reload_enabled() -> bool {
    std::env::var("MEI_DISABLE_HOT_RELOAD")
        .map(|value| {
            let trimmed = value.trim();
            !(trimmed == "1" || trimmed.eq_ignore_ascii_case("true"))
        })
        .unwrap_or(true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HotReloadNeed {
    None,
    /// meibundle 比 MCG registry 新（CLI 只 compile，或 import 尚未跟上）
    ImportAndRewarm,
    /// 结构已导入但 client-bootstrap 被清掉且尚未写回
    RewarmOnly,
}

pub(crate) fn detect_hot_reload_need(workspace: &Path, app_id: &str) -> HotReloadNeed {
    let ctx = HostContext::new(workspace.to_path_buf(), app_id.to_string());
    let bundle_path = ctx.bundle_path();
    let mcg_path = mei_host_graph::mcg_registry_path(workspace, app_id);

    if file_mtime(bundle_path.as_path()).is_some_and(|bundle_mtime| {
        match file_mtime(mcg_path.as_path()) {
            Some(mcg_mtime) => bundle_mtime > mcg_mtime,
            None => true,
        }
    }) {
        return HotReloadNeed::ImportAndRewarm;
    }

    if client_bootstrap_needs_rewarm(workspace, app_id) {
        return HotReloadNeed::RewarmOnly;
    }

    HotReloadNeed::None
}

fn client_bootstrap_needs_rewarm(workspace: &Path, app_id: &str) -> bool {
    // home 是日常 Access 主 scope；其它 scope 由 JIT / activate 补洞
    let status = mei_host_graph::bootstrap_embed_status(workspace, app_id, "home");
    !status.allowed && status.reason == "manifest_missing"
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// 在 serve Ready 后后台轮询：吃掉 CLI 离线 compile/import 留下的缺口。
pub(crate) async fn run_cli_artifact_hot_reload_loop(shell: SharedState, app_ids: Vec<String>) {
    if !hot_reload_enabled() {
        tracing::info!(target: "mei.hot_reload", "disabled via MEI_DISABLE_HOT_RELOAD");
        return;
    }
    if app_ids.is_empty() {
        return;
    }
    tracing::info!(
        target: "mei.hot_reload",
        apps = %app_ids.join(", "),
        "watching CLI compile/import artifacts for hot reload"
    );
    loop {
        tokio::time::sleep(POLL_INTERVAL).await;
        let workspace = {
            let Ok(guard) = shell.read() else {
                continue;
            };
            if guard
                .ops_job
                .as_ref()
                .is_some_and(crate::build_ops::OpsJobState::is_running)
            {
                continue;
            }
            if guard.startup_phase != "ready" {
                continue;
            }
            guard.ctx.workspace_root.clone()
        };

        for app_id in &app_ids {
            let need = detect_hot_reload_need(workspace.as_path(), app_id.as_str());
            if need == HotReloadNeed::None {
                continue;
            }
            if need == HotReloadNeed::ImportAndRewarm {
                let ctx = HostContext::new(workspace.clone(), app_id.clone());
                let Some(mtime_before) = file_mtime(ctx.bundle_path().as_path()) else {
                    continue;
                };
                tokio::time::sleep(BUNDLE_WRITE_DEBOUNCE).await;
                let Some(mtime_after) = file_mtime(ctx.bundle_path().as_path()) else {
                    continue;
                };
                if mtime_after != mtime_before {
                    // CLI 仍在写 bundle
                    continue;
                }
                if detect_hot_reload_need(workspace.as_path(), app_id.as_str())
                    != HotReloadNeed::ImportAndRewarm
                {
                    continue;
                }
            } else if need == HotReloadNeed::RewarmOnly {
                // 给并行 CLI `reload`（import 后立刻 rewarm）留出写回窗口，避免双跑 warmup
                tokio::time::sleep(BUNDLE_WRITE_DEBOUNCE + BUNDLE_WRITE_DEBOUNCE).await;
                if detect_hot_reload_need(workspace.as_path(), app_id.as_str())
                    != HotReloadNeed::RewarmOnly
                {
                    continue;
                }
            }

            {
                let mut guard = shell.write().expect("state lock");
                if let Err(error) = begin_ops_job(&mut guard, "hot_reload") {
                    tracing::debug!(
                        target: "mei.hot_reload",
                        app_id = %app_id,
                        %error,
                        "skip hot reload; ops job busy"
                    );
                    break;
                }
                guard.startup_detail = Some(format!(
                    "正在热重载 {app_id}（CLI 产物同步：import / warmup）…"
                ));
            }

            let workspace_for_job = workspace.clone();
            let app_for_job = app_id.clone();
            let need_for_job = need;
            let result = tokio::task::spawn_blocking(move || {
                apply_hot_reload(
                    workspace_for_job.as_path(),
                    app_for_job.as_str(),
                    need_for_job,
                )
            })
            .await
            .map_err(|error| format!("hot_reload join failed: {error}"))
            .and_then(|inner| inner.map_err(|error| error.to_string()));

            let mut guard = shell.write().expect("state lock");
            match result {
                Ok(message) => {
                    finish_ops_job_success(&mut guard, message.clone());
                    crate::build_ops::refresh_materialization_flags(&mut guard);
                    guard.startup_detail = Some("访问态已就绪".to_string());
                    tracing::info!(
                        target: "mei.hot_reload",
                        app_id = %app_id,
                        %message,
                        "CLI artifact hot reload complete"
                    );
                }
                Err(error) => {
                    finish_ops_job_failure(&mut guard, error.clone());
                    tracing::warn!(
                        target: "mei.hot_reload",
                        app_id = %app_id,
                        %error,
                        "CLI artifact hot reload failed"
                    );
                }
            }
            // 一次循环只处理一个 app，避免长时间占住 ops 锁
            break;
        }
    }
}

fn apply_hot_reload(workspace: &Path, app_id: &str, need: HotReloadNeed) -> anyhow::Result<String> {
    match need {
        HotReloadNeed::None => Ok(format!("hot_reload noop for {app_id}")),
        HotReloadNeed::ImportAndRewarm => {
            tracing::info!(
                target: "mei.hot_reload",
                app_id = %app_id,
                "bundle ahead of MCG — import + rewarm"
            );
            let report = import_with_options(workspace, app_id, None)?;
            rewarm_after_import(workspace, app_id, DEFAULT_WARMUP_POLICY)?;
            Ok(format!(
                "hot_reload import+rewarm ok (app={app_id}, revision={}, blocks={})",
                report.registry_revision, report.block_count
            ))
        }
        HotReloadNeed::RewarmOnly => {
            tracing::info!(
                target: "mei.hot_reload",
                app_id = %app_id,
                "client-bootstrap missing — rewarm"
            );
            rewarm_after_import(workspace, app_id, DEFAULT_WARMUP_POLICY)?;
            Ok(format!("hot_reload rewarm ok (app={app_id})"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use std::time::Duration;

    fn seed_app_env(workspace: &Path, app: &str) {
        let app_root = workspace.join("apps").join(app);
        let env = app_root.join("env/WS-1");
        let build = env.join("build");
        fs::create_dir_all(build.join("exchange")).expect("exchange");
        fs::create_dir_all(build.join("registry")).expect("registry");
        #[cfg(unix)]
        {
            let current = app_root.join("env/current");
            let _ = fs::remove_file(&current);
            let _ = fs::remove_dir_all(&current);
            std::os::unix::fs::symlink("WS-1", &current).expect("symlink");
        }
        #[cfg(not(unix))]
        fs::create_dir_all(app_root.join("env/current/build/exchange")).expect("current");
    }

    #[test]
    fn detect_import_when_bundle_newer_than_mcg() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        let app = "demo";
        seed_app_env(workspace, app);

        let mcg = mei_host_graph::mcg_registry_path(workspace, app);
        fs::create_dir_all(mcg.parent().expect("parent")).expect("registry dir");
        fs::write(
            &mcg,
            r#"{"schemaVersion":"mei-mcg-registry-v2","appId":"demo","registryRevision":"old","updatedAtMs":1,"nodes":[]}"#,
        )
        .expect("mcg");
        thread::sleep(Duration::from_millis(30));

        let ctx = HostContext::new(workspace.to_path_buf(), app.to_string());
        let bundle = ctx.bundle_path();
        fs::create_dir_all(bundle.parent().expect("parent")).expect("exchange");
        fs::write(&bundle, b"fake-bundle").expect("bundle");

        assert_eq!(
            detect_hot_reload_need(workspace, app),
            HotReloadNeed::ImportAndRewarm
        );
    }
}
