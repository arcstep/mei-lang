use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::http::Uri;
use mei_host_core::HostContext;
use mei_lang_app::UiRouteMode;

use crate::managed_plug::ManagedPlugDsPool;
use crate::review_axes::{
    access_readiness_requires_bootstrap, access_readiness_requires_plug_ds, PageRenderAxes,
};
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

#[derive(Debug, Clone)]
pub(crate) struct AppAccessProbe {
    pub ready: bool,
    pub reason: &'static str,
    pub bootstrap_reason: Option<String>,
}

pub(crate) fn probe_app_access_readiness(
    shell: &ShellState,
    app_id: &str,
    scene_id: &str,
    route_mode: UiRouteMode,
    axes: PageRenderAxes,
) -> AppAccessProbe {
    let readiness = evaluate_access_readiness(shell, app_id, scene_id, route_mode, axes);
    let bootstrap_reason = if route_mode.is_access_like() {
        Some(
            mei_host_graph::bootstrap_embed_status(
                shell.ctx.workspace_root.as_path(),
                app_id,
                scene_id,
            )
            .reason,
        )
    } else {
        None
    };
    AppAccessProbe {
        ready: readiness.ready,
        reason: readiness.reason,
        bootstrap_reason,
    }
}

pub(crate) fn format_app_access_line(app_id: &str, probe: &AppAccessProbe) -> String {
    if probe.ready {
        let bootstrap = probe.bootstrap_reason.as_deref().unwrap_or("-");
        format!("{app_id}: ACCESS READY (bootstrap={bootstrap})")
    } else {
        format!("{app_id}: waiting ({})", probe.reason)
    }
}

/// Warn when app config asks for clientBootstrap but MRG has no client-eligible slots
/// (empty Eval Pack → clients used to Pack-First-wait 8s). Call after access readiness settles.
pub(crate) fn warn_empty_client_bootstrap_packs(
    workspace_root: &Path,
    app_ids: &[String],
    scene_id: &str,
) {
    use mei_lang_kernel::{load_mei_config_for_app, resolve_app_root};

    for app_id in app_ids {
        let app_root = resolve_app_root(workspace_root, app_id.as_str());
        let config = load_mei_config_for_app(app_root.as_path(), Some(workspace_root));
        let Some(client_cfg) = config.runtime.client_bootstrap.as_ref() else {
            continue;
        };
        if !client_cfg.enabled {
            continue;
        }
        let scopes = if client_cfg.scopes.is_empty() {
            vec!["home".to_string()]
        } else {
            client_cfg.scopes.clone()
        };
        if !scopes.iter().any(|scope| scope == scene_id) {
            continue;
        }

        let status =
            mei_host_graph::bootstrap_embed_status(workspace_root, app_id.as_str(), scene_id);
        let registry = mei_host_graph::MrgRegistryWriter::load(workspace_root, app_id.as_str());
        let scope_slots = registry
            .slots
            .iter()
            .filter(|slot| slot.slot_id.scope_key == scene_id)
            .count();
        let client_eligible = registry
            .slots
            .iter()
            .filter(|slot| slot.client_eligible && slot.slot_id.scope_key == scene_id)
            .count();
        let empty_pack = status.reason == "no_client_bootstrap_required"
            || status.reason == "manifest_missing"
            || (scope_slots > 0 && client_eligible == 0);

        if !empty_pack {
            continue;
        }

        tracing::warn!(
            target: "mei.startup",
            app_id = %app_id,
            scene_id = %scene_id,
            bootstrap_reason = %status.reason,
            client_eligible,
            scope_slots,
            embed_mode = %client_cfg.embed_mode,
            "clientBootstrap enabled but Eval Pack will be empty (no client-eligible MRG slots); first paint metrics fall back to API — check warmup client tier / MEI_SKIP_PREBUILD"
        );
    }
}

pub(crate) fn build_access_ready_banner_lines(
    shell: &ShellState,
    app_ids: &[String],
    scene_id: &str,
    listen_url: &str,
) -> Vec<String> {
    let mut lines = vec![
        listen_url.to_string(),
        crate::build_info::host_version_banner_line(shell.ctx.workspace_root.as_path()),
        format!("defaultApp={}", shell.ctx.app_id),
    ];
    for app_id in app_ids {
        let probe = probe_app_access_readiness(
            shell,
            app_id.as_str(),
            scene_id,
            UiRouteMode::App,
            PageRenderAxes::default(),
        );
        lines.push(format_app_access_line(app_id.as_str(), &probe));
    }
    warn_empty_client_bootstrap_packs(shell.ctx.workspace_root.as_path(), app_ids, scene_id);
    lines.push("all listed apps ready — access pages may be served".to_string());
    lines
}

fn all_apps_access_ready(shell: &ShellState, app_ids: &[String], scene_id: &str) -> bool {
    app_ids.iter().all(|app_id| {
        probe_app_access_readiness(
            shell,
            app_id.as_str(),
            scene_id,
            UiRouteMode::App,
            PageRenderAxes::default(),
        )
        .ready
    })
}

pub(crate) fn prime_view_layer_artifacts(shell: &ShellState, app_ids: &[String], scene_id: &str) {
    let workspace_root = shell.ctx.workspace_root.as_path();
    let topbar_menu = mei_lang_app::load_topbar_menu_context(workspace_root);
    let discovered = crate::landing::discover_workspace_apps(workspace_root).unwrap_or_default();
    let apps = crate::landing::enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
    let chrome_host = crate::scene_manifest::SceneChromeHostContext {
        apps: apps.as_slice(),
        topbar_menu: Some(&topbar_menu),
        auth_enabled: false,
        auth_account: None,
    };
    for app_id in app_ids {
        if let Err(err) =
            mei_host_graph::warm_manifest_index_for_app(workspace_root, app_id.as_str(), scene_id)
        {
            tracing::warn!(
                target: "mei.startup",
                app_id = %app_id,
                scene_id = %scene_id,
                error = %err,
                "view layer semantic warmup failed"
            );
        }
        let mut hits = crate::artifact_observability::ArtifactHitMatrix::default();
        if let Err(err) = crate::scene_manifest::ensure_manifest_index(
            workspace_root,
            app_id.as_str(),
            scene_id,
            mei_lang_kernel::DataMode::Eval,
            &mut hits,
            Some(&chrome_host),
        ) {
            tracing::warn!(
                target: "mei.startup",
                app_id = %app_id,
                scene_id = %scene_id,
                error = %err,
                "view layer manifest index with chrome failed"
            );
        }
        let assemble_started = std::time::Instant::now();
        if let Err(err) =
            mei_host_graph::assemble_scope_from_registry(workspace_root, app_id.as_str(), scene_id)
        {
            tracing::warn!(
                target: "mei.startup",
                app_id = %app_id,
                scene_id = %scene_id,
                error = %err,
                "view layer assembly warmup failed"
            );
        } else {
            tracing::info!(
                target: "mei.startup",
                app_id = %app_id,
                scene_id = %scene_id,
                elapsed_ms = assemble_started.elapsed().as_millis() as u64,
                "view layer assembly warmup completed"
            );
        }
    }
}

fn all_apps_imported(workspace: &Path, app_ids: &[String]) -> bool {
    app_ids
        .iter()
        .all(|app_id| crate::landing::app_has_prebuilt_access_entry(workspace, app_id.as_str()))
}

fn log_newly_ready_apps(
    shell: &ShellState,
    app_ids: &[String],
    scene_id: &str,
    logged: &mut BTreeSet<String>,
) {
    for app_id in app_ids {
        if logged.contains(app_id) {
            continue;
        }
        let probe = probe_app_access_readiness(
            shell,
            app_id.as_str(),
            scene_id,
            UiRouteMode::App,
            PageRenderAxes::default(),
        );
        if probe.ready {
            logged.insert(app_id.clone());
            tracing::info!(
                target: "mei.startup",
                app_id = %app_id,
                scene_id = %scene_id,
                gate_reason = %probe.reason,
                bootstrap_reason = probe.bootstrap_reason.as_deref().unwrap_or("-"),
                "app access ready"
            );
        }
    }
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
    axes: PageRenderAxes,
) -> AccessReadiness {
    if !shell.data_plane_enabled {
        return AccessReadiness {
            ready: false,
            reason: if crate::workspace_profile_api::read_host_control_state(
                shell.ctx.workspace_root.as_path(),
            )
            .is_some_and(|value| value.get("activeProfile").is_some())
            {
                "disabled"
            } else {
                "unconfigured"
            },
        };
    }
    if shell.startup_error.is_some() {
        return AccessReadiness {
            ready: false,
            reason: "failed",
        };
    }
    let workspace = shell.ctx.workspace_root.as_path();
    if !crate::landing::app_has_prebuilt_access_entry(workspace, app_id) {
        return AccessReadiness {
            ready: false,
            reason: "importing",
        };
    }
    if route_mode.is_access_like() {
        let dev_eval = crate::dev_eval_scope::current();
        let skip_gates = dev_eval.profile.skips_access_bootstrap_gate();
        if !skip_gates && access_readiness_requires_bootstrap(axes) {
            let bootstrap = mei_host_graph::bootstrap_embed_status(workspace, app_id, scene_id);
            if !bootstrap.allowed {
                return AccessReadiness {
                    ready: false,
                    reason: "warming",
                };
            }
        }
        if !skip_gates
            && access_readiness_requires_plug_ds(axes)
            && shell.plug_ds_endpoint_for(app_id).is_none()
        {
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
    if segments.first().copied() != Some("apps") {
        return (
            default_app.to_string(),
            "home".to_string(),
            "app".to_string(),
        );
    }
    // Legacy: /apps/{mode}/{app}/scene/{stage} where mode is reserved (app/view/…)
    if segments.len() >= 3 && crate::shell_redirects::is_reserved_stage_segment(segments[1]) {
        let mode = segments[1].to_string();
        let app = segments.get(2).unwrap_or(&default_app).to_string();
        let scene = if segments.len() >= 5 && segments[3] == "scene" {
            segments[4].to_string()
        } else {
            "home".to_string()
        };
        return (app, scene, mode);
    }
    // Canonical: /apps/{app}/{stage}
    if segments.len() >= 3 && !crate::shell_redirects::is_reserved_stage_segment(segments[2]) {
        return (
            segments[1].to_string(),
            segments[2].to_string(),
            "app".to_string(),
        );
    }
    // /apps/{app}
    if segments.len() >= 2 {
        return (
            segments[1].to_string(),
            "home".to_string(),
            "app".to_string(),
        );
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
    pub listen_url: String,
    pub app_ids: Vec<String>,
    pub data_mode_ceiling: mei_lang_kernel::DataModeCeiling,
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
        tracing::info!(
            "deferring import/warmup to background prebuild; host will wait and only start plug-ds"
        );
        wait_for_workspace_import(
            &shell,
            plan.workspace.as_path(),
            plan.app_ids.as_slice(),
            &default_ctx,
        )
        .await?;
    } else {
        ensure_registry_materialized_with_wait(&shell, &default_ctx).await?;
    }

    set_startup_phase(&shell, StartupPhase::PlugDs);
    let external_plug_ds = crate::plug_proxy::configured_plug_ds_endpoint(&default_ctx);
    let mut managed_pool = None;
    let mut plug_ds_by_app = BTreeMap::new();
    let skip_plug_ds = crate::dev_eval_scope::current()
        .profile
        .skips_plug_ds_startup();
    let covered_by_runtime = {
        let guard = shell.read().expect("state lock");
        crate::legacy_compat::apps_covered_by_desired_runtime(&guard.launch_manifest)
    };
    if plan.data_mode_ceiling.requires_plug_ds() && !skip_plug_ds {
        if let Some(endpoint) = external_plug_ds.as_ref() {
            plug_ds_by_app.insert(plan.default_app_id.clone(), endpoint.clone());
        } else {
            let pool = crate::managed_plug::spawn_managed_plug_ds_pool(
                plan.workspace.as_path(),
                plan.app_ids.as_slice(),
                &covered_by_runtime,
            )
            .await?;
            plug_ds_by_app = pool.endpoints.clone();
            managed_pool = Some(pool);
        }
        let needing = crate::legacy_compat::apps_needing_managed_plug_ds(
            plan.app_ids.as_slice(),
            &covered_by_runtime,
        );
        if plug_ds_by_app.is_empty() && !needing.is_empty() {
            anyhow::bail!("no plug-ds endpoints available for serve");
        }
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
        let wants_warmup = plan.data_mode_ceiling.requires_metric_warmup()
            && !crate::dev_eval_scope::current()
                .profile
                .skips_startup_warmup();
        if wants_warmup {
            wait_for_prebuild_warmup(&shell, &plan).await?;
        }
    } else {
        let cleared = crate::access_page_cache::clear_legacy_page_render_cache_for_apps(
            plan.workspace.as_path(),
            plan.app_ids.as_slice(),
        );
        if cleared > 0 {
            tracing::info!(
                cleared,
                "removed legacy page-render-cache entries during startup"
            );
        }
    }

    crate::build_ops::refresh_materialization_flags(&mut shell.write().expect("state lock"));
    set_startup_phase(&shell, StartupPhase::Ready);
    if let Ok(mut guard) = shell.write() {
        guard.startup_error = None;
    }
    let guard = shell.read().expect("state lock");
    let warmup_detail = build_access_ready_banner_lines(
        &guard,
        plan.app_ids.as_slice(),
        "home",
        plan.listen_url.as_str(),
    );
    drop(guard);
    let warmup_refs: Vec<&str> = warmup_detail.iter().map(String::as_str).collect();
    crate::startup_banner::emit_access_warmup_ready_banner(warmup_refs.as_slice());

    tokio::spawn(crate::hot_reload::run_cli_artifact_hot_reload_loop(
        shell,
        plan.app_ids.clone(),
    ));

    Ok(())
}

async fn wait_for_workspace_import(
    shell: &SharedState,
    workspace: &Path,
    app_ids: &[String],
    default_ctx: &HostContext,
) -> anyhow::Result<()> {
    set_startup_phase(shell, StartupPhase::Importing);
    let wait_app_ids: Vec<String> = if app_ids.is_empty() {
        vec![default_ctx.app_id.clone()]
    } else {
        app_ids.to_vec()
    };
    let mut polls: u32 = 0;
    loop {
        polls = polls.saturating_add(1);
        if defer_warmup_to_prebuild() {
            if let Ok(mut guard) = shell.write() {
                crate::build_ops::refresh_materialization_flags(&mut guard);
            }
            if all_apps_imported(workspace, wait_app_ids.as_slice()) {
                tracing::info!(
                    target: "mei.startup",
                    apps = %wait_app_ids.join(", "),
                    "workspace import observed from background prebuild"
                );
                return Ok(());
            }
            if let Ok(mut guard) = shell.write() {
                guard.startup_detail =
                    Some("正在等待后台 prebuild 完成各 app 的编译与 import…".to_string());
                if polls == 1 || polls.is_multiple_of(15) {
                    let pending: Vec<String> = wait_app_ids
                        .iter()
                        .filter(|app_id| {
                            !crate::landing::app_has_prebuilt_access_entry(
                                workspace,
                                app_id.as_str(),
                            )
                        })
                        .cloned()
                        .collect();
                    tracing::info!(
                        target: "mei.startup",
                        startup_phase = %guard.startup_phase,
                        pending_apps = %pending.join(", "),
                        "waiting for background prebuild import"
                    );
                }
            }
        } else {
            match try_ensure_registry_materialized(default_ctx) {
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
        guard.startup_detail = Some("正在等待各 app 完成指标预热与 client-bootstrap…".to_string());
    }
    let wait_app_ids: Vec<String> = if plan.app_ids.is_empty() {
        vec![plan.default_app_id.clone()]
    } else {
        plan.app_ids.clone()
    };
    let mut polls: u32 = 0;
    let mut logged_ready = BTreeSet::new();
    loop {
        polls = polls.saturating_add(1);
        {
            let mut guard = shell.write().expect("state lock");
            crate::build_ops::refresh_materialization_flags(&mut guard);
        }
        {
            let guard = shell.read().expect("state lock");
            log_newly_ready_apps(&guard, wait_app_ids.as_slice(), "home", &mut logged_ready);
            if all_apps_access_ready(&guard, wait_app_ids.as_slice(), "home") {
                prime_view_layer_artifacts(&guard, wait_app_ids.as_slice(), "home");
                warn_empty_client_bootstrap_packs(
                    guard.ctx.workspace_root.as_path(),
                    wait_app_ids.as_slice(),
                    "home",
                );
                tracing::info!(
                    target: "mei.startup",
                    apps = %wait_app_ids.join(", "),
                    "all apps access warmup complete"
                );
                return Ok(());
            }
            if polls == 1 || polls.is_multiple_of(15) {
                let pending: Vec<String> = wait_app_ids
                    .iter()
                    .filter(|app_id| {
                        !probe_app_access_readiness(
                            &guard,
                            app_id.as_str(),
                            "home",
                            UiRouteMode::App,
                            PageRenderAxes::default(),
                        )
                        .ready
                    })
                    .cloned()
                    .collect();
                let sample = pending
                    .first()
                    .map(|app_id| {
                        probe_app_access_readiness(
                            &guard,
                            app_id.as_str(),
                            "home",
                            UiRouteMode::App,
                            PageRenderAxes::default(),
                        )
                    })
                    .map(|probe| {
                        format!(
                            "{}:{}",
                            probe.reason,
                            probe.bootstrap_reason.unwrap_or_default()
                        )
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                tracing::info!(
                    target: "mei.startup",
                    pending_apps = %pending.join(", "),
                    sample_pending = %sample,
                    "waiting for background prebuild warmup"
                );
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn ensure_registry_materialized_with_wait(
    shell: &SharedState,
    ctx: &HostContext,
) -> anyhow::Result<()> {
    wait_for_workspace_import(
        shell,
        ctx.workspace_root.as_path(),
        &[ctx.app_id.clone()],
        ctx,
    )
    .await
}

fn is_missing_artifact_error(error: &anyhow::Error) -> bool {
    let text = error.to_string();
    text.contains("MCG registry missing")
        || text.contains("bundle not found")
        || text.contains("run prebuild")
}

/// Import the app's meibundle when MCG registry is missing or empty (e.g. after `--skip-prebuild`).
pub(crate) fn try_ensure_app_registry_materialized(
    workspace: &std::path::Path,
    app_id: &str,
) -> anyhow::Result<()> {
    if crate::landing::app_has_prebuilt_access_entry(workspace, app_id) {
        return Ok(());
    }
    let ctx = HostContext::new(workspace.to_path_buf(), app_id.to_string());
    try_ensure_registry_materialized(&ctx)
}

fn try_ensure_registry_materialized(ctx: &HostContext) -> anyhow::Result<()> {
    let mcg_path =
        mei_host_graph::mcg_registry_path(ctx.workspace_root.as_path(), ctx.app_id.as_str());
    if mcg_path.is_file() {
        let registry = mei_host_graph::McgRegistryWriter::load(
            ctx.workspace_root.as_path(),
            ctx.app_id.as_str(),
        );
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
        app_id = %ctx.app_id,
        bundle = %mei_host_core::path_for_log(ctx.workspace_root.as_path(), bundle_path.as_path()),
        "auto-importing meibundle for app access"
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
            sanitize_return_path("/apps/mini-data/home"),
            "/apps/mini-data/home"
        );
        assert_eq!(
            sanitize_return_path("/apps/app/data-demo"),
            "/apps/app/data-demo"
        );
    }

    #[test]
    fn build_starting_location_preserves_return_target() {
        let uri: Uri = "/apps/mini-data/home?chrome=none".parse().expect("uri");
        let location = build_starting_location(&uri, "mini-data", "home", "app");
        assert!(location.contains("return=%2Fapps%2Fmini-data%2Fhome%3Fchrome%3Dnone"));
    }

    #[test]
    fn evaluate_access_readiness_checks_requested_app_registry() {
        use mei_lang_app::UiRouteMode;
        use std::collections::BTreeMap;
        use std::path::PathBuf;

        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path();
        std::fs::write(
            workspace.join("workspace.json"),
            r#"{"schemaVersion":1,"workspace":{"id":"test","defaultApp":"data-demo"}}"#,
        )
        .expect("workspace.json");
        let apps_dir = workspace.join("apps");
        for app_id in ["data-demo", "mini-park"] {
            std::fs::create_dir_all(
                apps_dir
                    .join(app_id)
                    .join("env/WS-20260628.0/build/registry"),
            )
            .expect("registry dir");
            std::fs::write(
                apps_dir.join(app_id).join("app.config.json"),
                format!(r#"{{"schemaVersion":1,"app":{{"id":"{app_id}"}}}}"#),
            )
            .expect("app config");
            std::os::unix::fs::symlink("WS-20260628.0", apps_dir.join(app_id).join("env/current"))
                .expect("env/current");
        }
        std::fs::write(
            apps_dir.join("data-demo/env/WS-20260628.0/build/registry/mcg-registry.json"),
            r#"{
  "schemaVersion": "mei-mcg-registry-v2",
  "appId": "data-demo",
  "registryRevision": "test-rev",
  "updatedAtMs": 1,
  "nodes": [
    {
      "id": { "kind": "app_skeleton", "key": "app_skeleton:data-demo" },
      "revision": "blk:test",
      "state": "ready",
      "layer": "import"
    }
  ]
}"#,
        )
        .expect("data-demo mcg");
        std::fs::write(
            apps_dir.join("mini-park/env/WS-20260628.0/build/registry/mcg-registry.json"),
            r#"{
  "schemaVersion": "mei-mcg-registry-v2",
  "appId": "mini-park",
  "registryRevision": "empty",
  "updatedAtMs": 1,
  "nodes": []
}"#,
        )
        .expect("mini-park mcg");

        let mut plug_ds_by_app = BTreeMap::new();
        plug_ds_by_app.insert("data-demo".to_string(), "http://127.0.0.1:9001".to_string());
        plug_ds_by_app.insert("mini-park".to_string(), "http://127.0.0.1:9002".to_string());
        let mut shell = ShellState::new(
            workspace.to_path_buf(),
            "data-demo".to_string(),
            PathBuf::from("/tmp/pkg"),
            plug_ds_by_app,
            false,
        );
        shell.imported = true;
        shell.data_plane_enabled = true;

        let data_demo = evaluate_access_readiness(
            &shell,
            "data-demo",
            "home",
            UiRouteMode::App,
            PageRenderAxes::default(),
        );
        assert!(data_demo.ready, "default app registry should be ready");

        let mini_park = evaluate_access_readiness(
            &shell,
            "mini-park",
            "home",
            UiRouteMode::App,
            PageRenderAxes::default(),
        );
        assert!(
            !mini_park.ready,
            "mini-park without nodes must not be ready"
        );
        assert_eq!(mini_park.reason, "importing");
        assert!(
            all_apps_access_ready(&shell, &[String::from("data-demo")], "home"),
            "single ready app should pass"
        );
        assert!(
            !all_apps_access_ready(
                &shell,
                &[String::from("data-demo"), String::from("mini-park")],
                "home"
            ),
            "all-apps gate must wait for mini-park"
        );
    }
}
