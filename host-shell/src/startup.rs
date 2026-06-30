use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::http::Uri;
use mei_host_core::HostContext;
use mei_lang_app::UiRouteMode;

use crate::managed_plug::ManagedPlugDsPool;
use crate::state::{SharedState, ShellState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupPhase {
    Preparing,
    WaitingArtifacts,
    Importing,
    PlugDs,
    PrimingCache,
    Ready,
    Failed,
}

impl StartupPhase {
    pub(crate) fn as_slug(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::WaitingArtifacts => "waiting_artifacts",
            Self::Importing => "importing",
            Self::PlugDs => "plug_ds",
            Self::PrimingCache => "priming_cache",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn user_message(self) -> &'static str {
        match self {
            Self::Preparing => "正在启动 MeiLang 宿主服务…",
            Self::WaitingArtifacts => "正在等待工作区编译与导入产物…",
            Self::Importing => "正在装载场景装配与访问态资源…",
            Self::PlugDs => "正在启动数据侧车与指标预热…",
            Self::PrimingCache => "正在预热页面渲染缓存…",
            Self::Ready => "访问态已就绪",
            Self::Failed => "启动未完成",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AccessReadiness {
    pub ready: bool,
    pub reason: &'static str,
}

pub(crate) fn set_startup_phase(shell: &SharedState, phase: StartupPhase) {
    if let Ok(mut guard) = shell.write() {
        guard.startup_phase = phase.as_slug().to_string();
        guard.startup_detail = Some(phase.user_message().to_string());
    }
}

pub(crate) fn set_startup_failed(shell: &SharedState, detail: String) {
    if let Ok(mut guard) = shell.write() {
        guard.startup_phase = StartupPhase::Failed.as_slug().to_string();
        guard.startup_detail = Some(detail.clone());
        guard.startup_error = Some(detail);
    }
}

pub(crate) fn evaluate_access_readiness(
    shell: &ShellState,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
) -> AccessReadiness {
    if shell.startup_error.is_some() {
        return AccessReadiness {
            ready: false,
            reason: "failed",
        };
    }
    if !shell.imported {
        return AccessReadiness {
            ready: false,
            reason: "importing",
        };
    }
    if route_mode.is_access_like() {
        let workspace = shell.ctx.workspace_root.as_path();
        let bootstrap =
            mei_host_graph::bootstrap_embed_status(workspace, app_id, scene_id);
        if !bootstrap.allowed {
            return AccessReadiness {
                ready: false,
                reason: "warming",
            };
        }
        if shell.plug_ds_endpoint_for(app_id).is_none() {
            return AccessReadiness {
                ready: false,
                reason: "plug_ds",
            };
        }
    }
    AccessReadiness {
        ready: true,
        reason: "ready",
    }
}

pub(crate) fn defer_warmup_to_prebuild() -> bool {
    std::env::var("MEI_DEFER_WARMUP_TO_PREBUILD")
        .map(|value| {
            let trimmed = value.trim();
            trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false)
}

pub(crate) fn sanitize_return_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || !trimmed.starts_with('/') || trimmed.starts_with("//") {
        return "/".to_string();
    }
    if trimmed.to_ascii_lowercase().starts_with("/http") {
        return "/".to_string();
    }
    trimmed.to_string()
}

pub(crate) fn percent_encode_query_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push(hex_digit(byte >> 4));
                out.push(hex_digit(byte & 0x0f));
            }
        }
    }
    out
}

pub(crate) fn build_starting_location(
    uri: &Uri,
    app_id: &str,
    scene_id: &str,
    mode: &str,
) -> String {
    let return_path = uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or_else(|| uri.path());
    let return_path = sanitize_return_path(return_path);
    format!(
        "/host/starting?return={}&app={}&scene={}&mode={}",
        percent_encode_query_component(return_path.as_str()),
        percent_encode_query_component(app_id),
        percent_encode_query_component(scene_id),
        percent_encode_query_component(mode),
    )
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        _ => (b'A' + value - 10) as char,
    }
}

pub(crate) fn parse_warm_poll_from_path(path: &str, default_app: &str) -> (String, String, String) {
    let segments: Vec<&str> = path
        .trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() >= 3 && segments[0] == "apps" {
        let mode = segments[1].to_string();
        let app = segments.get(2).unwrap_or(&default_app).to_string();
        let scene = if segments.len() >= 5 && segments[3] == "scene" {
            segments[4].to_string()
        } else {
            "home".to_string()
        };
        return (app, scene, mode);
    }
    (
        default_app.to_string(),
        "home".to_string(),
        "app".to_string(),
    )
}

pub(crate) struct ServeStartupPlan {
    pub workspace: PathBuf,
    pub package_root: PathBuf,
    pub default_app_id: String,
    pub auth_enabled: bool,
    pub app_ids: Vec<String>,
    pub managed_plug_slot: Arc<Mutex<Option<ManagedPlugDsPool>>>,
}

pub(crate) async fn run_background_startup(shell: SharedState, plan: ServeStartupPlan) {
    set_startup_phase(&shell, StartupPhase::Preparing);
    if let Err(error) = run_background_startup_inner(shell.clone(), plan).await {
        tracing::error!(%error, "host startup failed");
        set_startup_failed(&shell, error.to_string());
    }
}

async fn run_background_startup_inner(
    shell: SharedState,
    plan: ServeStartupPlan,
) -> anyhow::Result<()> {
    if let Some(report) = mei_host_core::ensure_workspace_stock_materialized(
        plan.workspace.as_path(),
        plan.package_root.as_path(),
    )? {
        if report.components.copied_files > 0
            || report.templates.copied_files > 0
            || report.authoring.copied_files > 0
        {
            tracing::info!(
                components = report.components.copied_files,
                templates = report.templates.copied_files,
                authoring = report.authoring.copied_files,
                "refreshed workspace stock during startup"
            );
        }
    }

    set_startup_phase(&shell, StartupPhase::WaitingArtifacts);
    let default_ctx = HostContext::new(plan.workspace.clone(), plan.default_app_id.clone());
    if defer_warmup_to_prebuild() {
        tracing::info!("deferring import/warmup to background prebuild; host will wait and only start plug-ds");
        wait_for_workspace_import(&shell, &default_ctx).await?;
    } else {
        ensure_registry_materialized_with_wait(&shell, &default_ctx).await?;
    }

    set_startup_phase(&shell, StartupPhase::PlugDs);
    let external_plug_ds = crate::plug_proxy::configured_plug_ds_endpoint(&default_ctx);
    let mut managed_pool = None;
    let mut plug_ds_by_app = BTreeMap::new();
    if let Some(endpoint) = external_plug_ds.as_ref() {
        plug_ds_by_app.insert(plan.default_app_id.clone(), endpoint.clone());
    } else {
        let pool = crate::managed_plug::spawn_managed_plug_ds_pool(
            plan.workspace.as_path(),
            plan.app_ids.as_slice(),
        )
        .await?;
        plug_ds_by_app = pool.endpoints.clone();
        managed_pool = Some(pool);
    }
    if plug_ds_by_app.is_empty() {
        anyhow::bail!("no plug-ds endpoints available for serve");
    }
    let plug_ds_managed = managed_pool.is_some();
    if let Some(pool) = managed_pool {
        if let Ok(mut slot) = plan.managed_plug_slot.lock() {
            *slot = Some(pool);
        }
    }

    {
        let mut guard = shell.write().expect("state lock");
        guard.plug_ds_by_app = plug_ds_by_app.clone();
        guard.plug_ds_endpoint = plug_ds_by_app
            .get(plan.default_app_id.as_str())
            .cloned()
            .unwrap_or_default();
        guard.plug_ds_managed = plug_ds_managed;
    }

    crate::build_ops::refresh_materialization_flags(&mut shell.write().expect("state lock"));

    if defer_warmup_to_prebuild() {
        wait_for_prebuild_warmup(&shell, &plan).await?;
    } else {
        set_startup_phase(&shell, StartupPhase::PrimingCache);
        let skip_page_cache = shell
            .read()
            .map(|guard| guard.warmed_up)
            .unwrap_or(false);
        let primed = if skip_page_cache {
            0
        } else {
            crate::access_page_cache::warm_access_page_render_caches(
                plan.workspace.as_path(),
                plan.package_root.as_path(),
                plan.app_ids.as_slice(),
                plan.auth_enabled,
            )
        };
        if primed > 0 {
            tracing::info!(primed, "page SSR cache primed during startup");
        }
    }

    crate::build_ops::refresh_materialization_flags(&mut shell.write().expect("state lock"));
    set_startup_phase(&shell, StartupPhase::Ready);
    if let Ok(mut guard) = shell.write() {
        guard.startup_error = None;
    }

    Ok(())
}

async fn wait_for_workspace_import(
    shell: &SharedState,
    ctx: &HostContext,
) -> anyhow::Result<()> {
    set_startup_phase(shell, StartupPhase::Importing);
    let mut polls: u32 = 0;
    loop {
        polls = polls.saturating_add(1);
        if defer_warmup_to_prebuild() {
            if let Ok(mut guard) = shell.write() {
                crate::build_ops::refresh_materialization_flags(&mut guard);
                if guard.imported {
                    tracing::info!("workspace import observed from background prebuild");
                    return Ok(());
                }
                guard.startup_detail =
                    Some("正在等待后台 prebuild 完成编译与 import…".to_string());
                if polls == 1 || polls.is_multiple_of(15) {
                    tracing::info!(
                        startup_phase = %guard.startup_phase,
                        imported = guard.imported,
                        "waiting for background prebuild import"
                    );
                }
            }
        } else {
            match try_ensure_registry_materialized(ctx) {
                Ok(()) => {
                    if let Ok(mut guard) = shell.write() {
                        crate::build_ops::refresh_materialization_flags(&mut guard);
                    }
                    return Ok(());
                }
                Err(error) if is_missing_artifact_error(&error) => {
                    set_startup_phase(shell, StartupPhase::WaitingArtifacts);
                    if let Ok(mut guard) = shell.write() {
                        crate::build_ops::refresh_materialization_flags(&mut guard);
                        if guard.imported {
                            return Ok(());
                        }
                    }
                }
                Err(error) => return Err(error),
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn wait_for_prebuild_warmup(
    shell: &SharedState,
    plan: &ServeStartupPlan,
) -> anyhow::Result<()> {
    set_startup_phase(shell, StartupPhase::PrimingCache);
    if let Ok(mut guard) = shell.write() {
        guard.startup_detail =
            Some("正在等待后台 prebuild 完成指标与 client-bootstrap 预热…".to_string());
    }
    let mut polls: u32 = 0;
    loop {
        polls = polls.saturating_add(1);
        {
            let mut guard = shell.write().expect("state lock");
            crate::build_ops::refresh_materialization_flags(&mut guard);
        }
        let bootstrap = mei_host_graph::bootstrap_embed_status(
            plan.workspace.as_path(),
            plan.default_app_id.as_str(),
            "home",
        );
        if bootstrap.allowed {
            tracing::info!(
                app_id = %plan.default_app_id,
                bootstrap_reason = %bootstrap.reason,
                metric_count = bootstrap.metric_count,
                "background prebuild warmup complete"
            );
            return Ok(());
        }
        if polls == 1 || polls.is_multiple_of(15) {
            tracing::info!(
                app_id = %plan.default_app_id,
                bootstrap_reason = %bootstrap.reason,
                metric_count = bootstrap.metric_count,
                "waiting for background prebuild warmup"
            );
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn ensure_registry_materialized_with_wait(
    shell: &SharedState,
    ctx: &HostContext,
) -> anyhow::Result<()> {
    wait_for_workspace_import(shell, ctx).await
}

fn is_missing_artifact_error(error: &anyhow::Error) -> bool {
    let text = error.to_string();
    text.contains("MCG registry missing")
        || text.contains("bundle not found")
        || text.contains("run prebuild")
}

fn try_ensure_registry_materialized(ctx: &HostContext) -> anyhow::Result<()> {
    let mcg_path = mei_host_graph::mcg_registry_path(
        ctx.workspace_root.as_path(),
        ctx.app_id.as_str(),
    );
    if mcg_path.is_file() {
        let registry =
            mei_host_graph::McgRegistryWriter::load(ctx.workspace_root.as_path(), ctx.app_id.as_str());
        if !registry.nodes.is_empty() {
            return Ok(());
        }
    }
    let bundle_path = ctx.bundle_path();
    if !bundle_path.is_file() {
        anyhow::bail!(
            "MCG registry missing and bundle not found at {}; run prebuild or `mei-host-shell import`",
            mei_host_core::path_for_log(ctx.workspace_root.as_path(), bundle_path.as_path())
        );
    }
    tracing::info!(
        bundle = %mei_host_core::path_for_log(ctx.workspace_root.as_path(), bundle_path.as_path()),
        "auto-importing meibundle during startup"
    );
    mei_host_graph::import_bundle(
        ctx,
        &mei_host_graph::ImportOptions {
            bundle_path: Some(bundle_path),
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_return_path_rejects_external_urls() {
        assert_eq!(sanitize_return_path("https://evil.test/x"), "/");
        assert_eq!(sanitize_return_path("//evil.test/x"), "/");
        assert_eq!(
            sanitize_return_path("/apps/app/data-demo"),
            "/apps/app/data-demo"
        );
    }

    #[test]
    fn build_starting_location_preserves_return_target() {
        let uri: Uri = "/apps/app/data-demo/scene/home?tab=board"
            .parse()
            .expect("uri");
        let location = build_starting_location(&uri, "data-demo", "home", "app");
        assert!(location.contains("return=%2Fapps%2Fapp%2Fdata-demo%2Fscene%2Fhome%3Ftab%3Dboard"));
    }
}
