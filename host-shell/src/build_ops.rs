use std::path::{Path, PathBuf};

use mei_host_core::ImportReport;
use mei_lang_kernel::{
    discover_apps, finalize_and_promote_build, prepare_dev_build_generation_with_hint,
    read_links_state, resolve_active_build_identity, resolve_app_build_generation_from_current,
    resolve_app_root, resolve_build_footer_label, resolve_toolchain_version_with_hint,
    resolve_workspace_version, PrebuildGeneration,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::landing::build_discovered_app_summaries;
use crate::state::ShellState;

pub fn toolchain_hint() -> &'static str {
    crate::build_info::BUILD_VERSION
}

pub fn canonical_workspace(workspace: &Path) -> PathBuf {
    workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf())
}

pub fn resolve_app_id(workspace: &Path, app: Option<&str>) -> anyhow::Result<String> {
    if let Some(app) = app.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(app.to_string());
    }
    let cfg = mei_lang_kernel::load_workspace_config(workspace);
    if let Some(default) = cfg
        .workspace
        .default_app
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(default.to_string());
    }
    anyhow::bail!("no app specified and workspace has no defaultApp")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsAppProgress {
    pub app_id: String,
    pub phase: String,
    pub completed: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsJobState {
    pub kind: String,
    pub status: String,
    pub phase: String,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub message: Option<String>,
    pub error: Option<String>,
    pub profile_id: Option<String>,
    pub profile_revision: Option<String>,
    pub generation: Option<String>,
    #[serde(default)]
    pub apps: Vec<OpsAppProgress>,
    #[serde(default)]
    pub log_summary: Vec<String>,
}

impl OpsJobState {
    pub fn running(kind: &str, started_at_ms: u64) -> Self {
        Self {
            kind: kind.to_string(),
            status: "running".to_string(),
            phase: "queued".to_string(),
            started_at_ms,
            finished_at_ms: None,
            message: None,
            error: None,
            profile_id: None,
            profile_revision: None,
            generation: None,
            apps: Vec::new(),
            log_summary: vec!["job queued".to_string()],
        }
    }

    pub fn is_running(&self) -> bool {
        self.status == "running"
    }

    fn push_log(&mut self, message: impl Into<String>) {
        const MAX_LOG_SUMMARY: usize = 24;
        self.log_summary.push(message.into());
        if self.log_summary.len() > MAX_LOG_SUMMARY {
            self.log_summary.remove(0);
        }
    }

    pub fn append_log(&mut self, message: impl Into<String>) {
        self.push_log(message);
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReloadOutcome {
    pub accepted: bool,
    pub blocks_changed: bool,
    pub block_count: usize,
    pub registry_revision: String,
    pub previous_revision: String,
}

pub fn import_with_options(
    workspace: &Path,
    app: &str,
    bundle: Option<PathBuf>,
) -> anyhow::Result<ImportReport> {
    let workspace = canonical_workspace(workspace);
    crate::build_info::log_host_identity(Some(workspace.as_path()), "import");
    let ctx = mei_host_core::HostContext::new(workspace, app.to_string());
    let options = mei_host_graph::ImportOptions {
        bundle_path: bundle,
    };
    let report =
        mei_host_graph::import_bundle(&ctx, &options).map_err(|e| anyhow::anyhow!("{e}"))?;
    let _ = crate::access_page_cache::clear_legacy_page_render_cache_for_app(
        ctx.workspace_root.as_path(),
        app,
    );
    Ok(report)
}

/// Import 之后的热重载后半段：清 assemble 缓存、增量失效 eval-cache，并跑 warmup
/// 把 client-bootstrap 写回。与 prebuild 的 invalidate + warmup 对齐，但不做
/// data snapshot / finalize。
pub fn rewarm_after_import(workspace: &Path, app: &str, policy: &str) -> anyhow::Result<()> {
    let workspace = canonical_workspace(workspace);
    mei_host_graph::clear_assemble_cache_for_app(app);

    let force_clear = std::env::var("MEI_FORCE_EVAL_CACHE_CLEAR")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false);
    let invalidation =
        mei_host_graph::invalidate_app_eval_cache(workspace.as_path(), app, force_clear)?;
    tracing::info!(
        app_id = %app,
        force_cleared = invalidation.force_cleared,
        removed_artifact_files = invalidation.removed_artifact_files,
        retained_artifact_files = invalidation.retained_artifact_files,
        cleared_bootstrap_scopes = invalidation.cleared_bootstrap_scopes,
        "eval-cache incremental invalidation (reload/hot-reload)"
    );

    let dev_eval = crate::dev_eval_scope::current_for_app(app);
    if !dev_eval.allows_rewarm() {
        tracing::info!(
            app_id = %app,
            profile = dev_eval.profile.slug(),
            "skipping warmup rewarm under non-full dev eval profile"
        );
        return Ok(());
    }

    crate::tool_exec::run_mei_plug_ds_warmup(workspace.as_path(), app, policy, "client")?;
    Ok(())
}

pub fn rewarm_after_import_for_scenes(
    workspace: &Path,
    app: &str,
    scenes: &[String],
) -> anyhow::Result<()> {
    if scenes.is_empty() {
        tracing::info!(
            app_id = %app,
            "skipping warmup rewarm: no hot scenes configured"
        );
        return Ok(());
    }
    let workspace = canonical_workspace(workspace);
    // Invalidate once, then disk+client warmup per required hot scene (no cross-process L1).
    mei_host_graph::clear_assemble_cache_for_app(app);
    let force_clear = std::env::var("MEI_FORCE_EVAL_CACHE_CLEAR")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false);
    let invalidation =
        mei_host_graph::invalidate_app_eval_cache(workspace.as_path(), app, force_clear)?;
    tracing::info!(
        app_id = %app,
        force_cleared = invalidation.force_cleared,
        removed_artifact_files = invalidation.removed_artifact_files,
        retained_artifact_files = invalidation.retained_artifact_files,
        cleared_bootstrap_scopes = invalidation.cleared_bootstrap_scopes,
        scenes = %scenes.join(","),
        "eval-cache incremental invalidation (multi-scene rewarm)"
    );
    let dev_eval = crate::dev_eval_scope::current_for_app(app);
    if !dev_eval.allows_rewarm() {
        tracing::info!(
            app_id = %app,
            profile = dev_eval.profile.slug(),
            "skipping warmup rewarm under non-full dev eval profile"
        );
        return Ok(());
    }
    for scene in scenes {
        crate::tool_exec::run_mei_plug_ds_warmup(
            workspace.as_path(),
            app,
            scene.as_str(),
            "client",
        )?;
    }
    Ok(())
}

pub fn reload_pipeline(workspace: &Path, app: &str) -> anyhow::Result<ReloadOutcome> {
    let workspace = canonical_workspace(workspace);
    crate::build_info::log_host_identity(Some(workspace.as_path()), "reload");

    let _generation = prepare_dev_build_generation_with_hint(
        workspace.as_path(),
        &[app.to_string()],
        Some(toolchain_hint()),
    )?;

    crate::tool_exec::run_mei_compiler_compile(workspace.as_path(), app)?;

    let prev_revision = mei_host_graph::McgRegistryWriter::load(workspace.as_path(), app)
        .registry_revision
        .clone();
    let report = import_with_options(workspace.as_path(), app, None)?;
    // 日常改 .mei 的热重载闭环：import 会清 stale bootstrap，必须立刻 warmup 写回。
    // 须用 home（与 prebuild / warmup_policy 对齐）；standard 无 workset → manifest_missing。
    rewarm_after_import(workspace.as_path(), app, "home")?;
    Ok(ReloadOutcome {
        accepted: true,
        blocks_changed: report.registry_revision != prev_revision,
        block_count: report.block_count,
        registry_revision: report.registry_revision,
        previous_revision: prev_revision,
    })
}

pub fn prebuild_pipeline(workspace: &Path, app: &str, scenes: &[String]) -> anyhow::Result<String> {
    let workspace = canonical_workspace(workspace);
    crate::build_info::log_host_identity(Some(workspace.as_path()), "prebuild");
    let phase = mei_host_core::ProcessPhaseTimer::start();

    let generation = prepare_dev_build_generation_with_hint(
        workspace.as_path(),
        &[app.to_string()],
        Some(toolchain_hint()),
    )?;
    let build_id = generation.env_version.clone();
    let config_digest = generation.config_digest.clone();

    let compile_phase = mei_host_core::ProcessPhaseTimer::start();
    crate::tool_exec::run_mei_compiler_compile(workspace.as_path(), app)?;
    let compile_sample = compile_phase.finish();
    tracing::info!(
        app_id = %app,
        wall_ms = compile_sample.wall_ms,
        rss_before = ?compile_sample.rss_before_bytes,
        rss_after = ?compile_sample.rss_bytes,
        cpu_user_ms = ?compile_sample.cpu_user_ms,
        "prebuild phase=compile"
    );

    let import_phase = mei_host_core::ProcessPhaseTimer::start();
    import_with_options(workspace.as_path(), app, None)?;
    let import_sample = import_phase.finish();
    tracing::info!(
        app_id = %app,
        wall_ms = import_sample.wall_ms,
        rss_after = ?import_sample.rss_bytes,
        "prebuild phase=import"
    );

    let snapshot_phase = mei_host_core::ProcessPhaseTimer::start();
    let _ = mei_host_graph::publish_app_data_snapshots(workspace.as_path(), app)?;
    let snapshot_sample = snapshot_phase.finish();
    tracing::info!(
        app_id = %app,
        wall_ms = snapshot_sample.wall_ms,
        "prebuild phase=snapshot"
    );

    let warmup_phase = mei_host_core::ProcessPhaseTimer::start();
    rewarm_after_import_for_scenes(workspace.as_path(), app, scenes)?;
    let warmup_sample = warmup_phase.finish();
    tracing::info!(
        app_id = %app,
        wall_ms = warmup_sample.wall_ms,
        scenes = %scenes.join(","),
        rss_after = ?warmup_sample.rss_bytes,
        cpu_user_ms = ?warmup_sample.cpu_user_ms,
        "prebuild phase=warmup"
    );

    let finalize_phase = mei_host_core::ProcessPhaseTimer::start();
    let generation = PrebuildGeneration {
        env_version: build_id.clone(),
        build_generation: build_id.clone(),
        toolchain_version: resolve_toolchain_version_with_hint(
            workspace.as_path(),
            Some(toolchain_hint()),
        ),
        workspace_version: resolve_workspace_version(workspace.as_path()),
        config_digest,
        store_dirs: std::iter::once(app.to_string())
            .map(|app_id| {
                let app_root = resolve_app_root(workspace.as_path(), app_id.as_str());
                (
                    app_id,
                    mei_lang_kernel::app_env_build_dir(app_root.as_path(), build_id.as_str()),
                )
            })
            .collect(),
    };
    finalize_and_promote_build(
        workspace.as_path(),
        &generation,
        &[app.to_string()],
        None,
        None,
        true,
    )?;
    let finalize_sample = finalize_phase.finish();
    let total = phase.finish();
    tracing::info!(
        app_id = %app,
        wall_ms = finalize_sample.wall_ms,
        total_wall_ms = total.wall_ms,
        total_rss_after = ?total.rss_bytes,
        "prebuild phase=finalize"
    );

    let policy_label = if scenes.is_empty() {
        "home".to_string()
    } else {
        scenes.join(",")
    };
    let prebuild_lines = vec![
        format!("app={app} | envVersion={build_id}"),
        format!("warmup policy={policy_label}"),
        "compile/import/warmup script finished for this app".to_string(),
        "host emits green ACCESS READY only after every discovered app is ready".to_string(),
    ];
    let prebuild_refs: Vec<&str> = prebuild_lines.iter().map(String::as_str).collect();
    crate::startup_banner::emit_prebuild_pipeline_complete_banner(prebuild_refs.as_slice());

    Ok(build_id)
}

pub fn build_status_aggregate(shell: &ShellState) -> Value {
    let workspace = shell.ctx.workspace_root.as_path();
    let identity = resolve_active_build_identity(workspace);
    let links = read_links_state(workspace).ok();
    let version =
        crate::build_info::version_descriptor(Some(workspace), Some(shell.host_started_at_ms));
    let access_ready = shell.data_plane_enabled && shell.imported;
    let warmup_ready = shell.warmed_up;
    let has_active_profile = crate::workspace_profile_api::read_host_control_state(workspace)
        .is_some_and(|value| {
            value
                .get("activeProfile")
                .is_some_and(|profile| profile.is_object())
        });
    let phase = if shell.ops_job.as_ref().is_some_and(OpsJobState::is_running) {
        "building"
    } else if !has_active_profile {
        "unconfigured"
    } else if !shell.data_plane_enabled || !shell.imported || shell.startup_error.is_some() {
        "degraded"
    } else if warmup_ready {
        "ready"
    } else {
        "degraded"
    };
    let app_ids: Vec<String> = discover_apps(workspace)
        .unwrap_or_default()
        .into_iter()
        .map(|app| app.id)
        .collect();
    let mut current_by_app = serde_json::Map::new();
    for app_id in &app_ids {
        let app_root = resolve_app_root(workspace, app_id.as_str());
        if let Ok(current) = resolve_app_build_generation_from_current(app_root.as_path()) {
            current_by_app.insert(app_id.clone(), json!(current));
        }
    }
    let dev_eval = crate::dev_eval_scope::current_for_app(shell.default_app().unwrap_or("*"));
    let probe_scope = dev_eval
        .eval_scopes
        .first()
        .cloned()
        .unwrap_or_else(|| "home".to_string());
    let probe_eval_allowed = dev_eval.allows_eval_scope(probe_scope.as_str());
    let probe_warmup_scope = dev_eval
        .warmup_scopes
        .first()
        .cloned()
        .unwrap_or_else(|| probe_scope.clone());
    let probe_warmup_allowed = dev_eval.allows_warmup_scope(probe_warmup_scope.as_str());
    json!({
        "hostShellOps": true,
        "binary": crate::build_info::binary_descriptor(),
        "version": version,
        "displayLabel": resolve_build_footer_label(workspace),
        "appId": shell.default_app(),
        "defaultAppId": shell.default_app(),
        "scopeNote": "ops materialization and plug-ds primary endpoint are default-app scoped",
        "discoveredApps": build_discovered_app_summaries(shell),
        "accessReady": access_ready,
        "warmupReady": warmup_ready,
        "phase": phase,
        "devEval": {
            "config": dev_eval.client_payload(),
            "probeEvalScope": probe_scope,
            "probeEvalAllowed": probe_eval_allowed,
            "probeWarmupScope": probe_warmup_scope,
            "probeWarmupAllowed": probe_warmup_allowed,
        },
        "env": {
            "currentByApp": current_by_app,
            "candidate": links.as_ref().and_then(|state| state.build.candidate.clone()),
            "previous": links.as_ref().and_then(|state| state.build.previous.clone()),
        },
        "toolchain": {
            "active": links
                .as_ref()
                .and_then(|state| state.toolchain.active.clone())
                .unwrap_or_else(|| identity.meilang_version.clone()),
        },
        "plugDs": {
            "endpoint": shell.plug_ds_endpoint.clone(),
            "endpoints": shell.plug_ds_by_app,
            "managed": shell.plug_ds_managed,
        },
        "workspaceVersion": identity.workspace_version,
        "jobPhase": shell.ops_job.as_ref().map(|job| job.phase.as_str()),
        "jobApps": shell.ops_job.as_ref().map(|job| job.apps.as_slice()),
        "jobLogSummary": shell.ops_job.as_ref().map(|job| job.log_summary.as_slice()),
        "jobProfileId": shell.ops_job.as_ref().and_then(|job| job.profile_id.as_deref()),
        "jobProfileRevision": shell.ops_job.as_ref().and_then(|job| job.profile_revision.as_deref()),
        "job": shell.ops_job,
        "lastJob": shell.last_ops_job,
    })
}

pub fn refresh_materialization_flags(shell: &mut ShellState) {
    let Some(app_id) = shell.default_app().map(str::to_string) else {
        shell.imported = false;
        shell.warmed_up = false;
        return;
    };
    let app_root = resolve_app_root(shell.ctx.workspace_root.as_path(), app_id.as_str());
    let current = app_root.join("env/current");
    if !current.exists() && !current.is_symlink() {
        shell.imported = false;
        shell.warmed_up = false;
        return;
    }
    shell.imported =
        mei_host_graph::mcg_registry_path(shell.ctx.workspace_root.as_path(), app_id.as_str())
            .is_file();
    shell.warmed_up =
        mei_host_graph::mrg_registry_path(shell.ctx.workspace_root.as_path(), app_id.as_str())
            .is_file();
}

pub fn begin_ops_job(shell: &mut ShellState, kind: &str) -> Result<(), String> {
    if shell.ops_job.as_ref().is_some_and(OpsJobState::is_running) {
        return Err("another host-shell ops job is already running".to_string());
    }
    shell.ops_job = Some(OpsJobState::running(kind, crate::state::current_time_ms()));
    emit_job_event(shell);
    Ok(())
}

pub fn begin_profile_ops_job(
    shell: &mut ShellState,
    profile_id: &str,
    profile_revision: &str,
    app_ids: &[String],
) -> Result<(), String> {
    begin_ops_job(shell, "apply-profile")?;
    if let Some(job) = shell.ops_job.as_mut() {
        job.profile_id = Some(profile_id.to_string());
        job.profile_revision = Some(profile_revision.to_string());
        job.apps = app_ids
            .iter()
            .map(|app_id| OpsAppProgress {
                app_id: app_id.clone(),
                phase: "queued".to_string(),
                completed: false,
                message: None,
            })
            .collect();
    }
    emit_job_event(shell);
    Ok(())
}

pub fn update_ops_job_phase(shell: &mut ShellState, phase: &str, message: impl Into<String>) {
    if let Some(job) = shell.ops_job.as_mut() {
        let message = message.into();
        job.phase = phase.to_string();
        job.push_log(message);
    }
    emit_job_event(shell);
}

pub fn update_ops_job_generation(shell: &mut ShellState, generation: &str) {
    if let Some(job) = shell.ops_job.as_mut() {
        job.generation = Some(generation.to_string());
        job.push_log(format!("generation: {generation}"));
    }
    emit_job_event(shell);
}

pub fn update_ops_app_progress(
    shell: &mut ShellState,
    app_id: &str,
    phase: &str,
    completed: bool,
    message: impl Into<String>,
) {
    if let Some(job) = shell.ops_job.as_mut() {
        let message = message.into();
        if let Some(app) = job.apps.iter_mut().find(|app| app.app_id == app_id) {
            app.phase = phase.to_string();
            app.completed = completed;
            app.message = Some(message.clone());
        }
        job.push_log(format!("{app_id}: {message}"));
    }
    emit_job_event(shell);
}

fn emit_job_event(shell: &ShellState) {
    let Some(job) = shell.ops_job.as_ref() else {
        return;
    };
    let payload = serde_json::to_value(job).unwrap_or_else(|_| json!({}));
    let _ = shell
        .events
        .send(crate::state::HostEvent::new("job-phase", payload));
}

pub fn finish_ops_job_success(shell: &mut ShellState, message: String) {
    let finished_at_ms = crate::state::current_time_ms();
    if let Some(job) = shell.ops_job.as_mut() {
        job.status = "succeeded".to_string();
        job.phase = "succeeded".to_string();
        job.finished_at_ms = Some(finished_at_ms);
        job.message = Some(message.clone());
        job.push_log(message);
    }
    emit_job_event(shell);
    if let Some(job) = shell.ops_job.clone() {
        shell.last_ops_job = Some(job);
        shell.ops_job = None;
    }
    refresh_materialization_flags(shell);
}

pub fn finish_ops_job_failure(shell: &mut ShellState, error: String) {
    let finished_at_ms = crate::state::current_time_ms();
    if let Some(job) = shell.ops_job.as_mut() {
        job.status = "failed".to_string();
        job.phase = "failed".to_string();
        job.finished_at_ms = Some(finished_at_ms);
        job.error = Some(error.clone());
        job.push_log(error);
    }
    emit_job_event(shell);
    if let Some(job) = shell.ops_job.clone() {
        shell.last_ops_job = Some(job);
        shell.ops_job = None;
    }
}
