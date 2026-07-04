use std::path::{Path, PathBuf};

use mei_host_core::ImportReport;
use mei_lang_kernel::{
    finalize_and_promote_build, prepare_dev_build_generation_with_hint, read_links_state,
    discover_apps, resolve_active_build_identity, resolve_app_build_generation_from_current,
    resolve_app_root, resolve_build_footer_label, resolve_toolchain_version_with_hint,
    resolve_workspace_version, PrebuildGeneration,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::landing::build_discovered_app_summaries;
use crate::state::ShellState;

pub fn toolchain_hint() -> &'static str {
    crate::build_info::CARGO_PACKAGE_VERSION
}

pub fn canonical_workspace(workspace: &Path) -> PathBuf {
    workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf())
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
pub struct OpsJobState {
    pub kind: String,
    pub status: String,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub message: Option<String>,
    pub error: Option<String>,
}

impl OpsJobState {
    pub fn running(kind: &str, started_at_ms: u64) -> Self {
        Self {
            kind: kind.to_string(),
            status: "running".to_string(),
            started_at_ms,
            finished_at_ms: None,
            message: None,
            error: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.status == "running"
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
    let options = mei_host_graph::ImportOptions { bundle_path: bundle };
    let report = mei_host_graph::import_bundle(&ctx, &options).map_err(|e| anyhow::anyhow!("{e}"))?;
    let _ = crate::access_page_cache::clear_access_page_render_cache_for_app(
        ctx.workspace_root.as_path(),
        app,
    );
    let _ = crate::build_fragment_cache::clear_build_fragment_cache_for_app(app);
    let _ = crate::layout_tuning_draft_store::clear_layout_tuning_drafts_for_app(
        ctx.workspace_root.as_path(),
        app,
    );
    Ok(report)
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
    Ok(ReloadOutcome {
        accepted: true,
        blocks_changed: report.registry_revision != prev_revision,
        block_count: report.block_count,
        registry_revision: report.registry_revision,
        previous_revision: prev_revision,
    })
}

pub fn prebuild_pipeline(workspace: &Path, app: &str, policy: &str) -> anyhow::Result<String> {
    let workspace = canonical_workspace(workspace);
    crate::build_info::log_host_identity(Some(workspace.as_path()), "prebuild");

    let generation = prepare_dev_build_generation_with_hint(
        workspace.as_path(),
        &[app.to_string()],
        Some(toolchain_hint()),
    )?;
    let build_id = generation.env_version.clone();

    crate::tool_exec::run_mei_compiler_compile(workspace.as_path(), app)?;
    import_with_options(workspace.as_path(), app, None)?;

    let _ = mei_host_graph::publish_app_data_snapshots(workspace.as_path(), app)?;

    let force_clear = std::env::var("MEI_FORCE_EVAL_CACHE_CLEAR")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false);
    let invalidation = mei_host_graph::invalidate_app_eval_cache(
        workspace.as_path(),
        app,
        force_clear,
    )?;
    tracing::info!(
        app_id = %app,
        force_cleared = invalidation.force_cleared,
        removed_artifact_files = invalidation.removed_artifact_files,
        retained_artifact_files = invalidation.retained_artifact_files,
        cleared_bootstrap_scopes = invalidation.cleared_bootstrap_scopes,
        "eval-cache incremental invalidation"
    );

    crate::tool_exec::run_mei_plug_ds_warmup(workspace.as_path(), app, policy, "all")?;

    let generation = PrebuildGeneration {
        env_version: build_id.clone(),
        build_generation: build_id.clone(),
        toolchain_version: resolve_toolchain_version_with_hint(
            workspace.as_path(),
            Some(toolchain_hint()),
        ),
        workspace_version: resolve_workspace_version(workspace.as_path()),
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

    let prebuild_lines = vec![
        format!("app={app} | envVersion={build_id}"),
        format!("warmup policy={policy}"),
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
    let version = crate::build_info::version_descriptor(
        Some(workspace),
        Some(shell.host_started_at_ms),
    );
    let access_ready = shell.imported;
    let warmup_ready = shell.warmed_up;
    let phase = if !shell.imported {
        "starting"
    } else if warmup_ready {
        "ready"
    } else {
        "bound"
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
    json!({
        "hostShellOps": true,
        "binary": crate::build_info::binary_descriptor(),
        "version": version,
        "displayLabel": resolve_build_footer_label(workspace),
        "appId": shell.ctx.app_id,
        "defaultAppId": shell.ctx.app_id,
        "scopeNote": "ops materialization and plug-ds primary endpoint are default-app scoped",
        "discoveredApps": build_discovered_app_summaries(shell),
        "accessReady": access_ready,
        "warmupReady": warmup_ready,
        "phase": phase,
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
        "job": shell.ops_job,
        "lastJob": shell.last_ops_job,
    })
}

pub fn refresh_materialization_flags(shell: &mut ShellState) {
    shell.imported = mei_host_graph::mcg_registry_path(
        shell.ctx.workspace_root.as_path(),
        shell.ctx.app_id.as_str(),
    )
    .is_file();
    shell.warmed_up = mei_host_graph::mrg_registry_path(
        shell.ctx.workspace_root.as_path(),
        shell.ctx.app_id.as_str(),
    )
    .is_file();
}

pub fn begin_ops_job(shell: &mut ShellState, kind: &str) -> Result<(), String> {
    if shell.ops_job.as_ref().is_some_and(OpsJobState::is_running) {
        return Err("another host-shell ops job is already running".to_string());
    }
    shell.ops_job = Some(OpsJobState::running(
        kind,
        crate::state::current_time_ms(),
    ));
    Ok(())
}

pub fn finish_ops_job_success(shell: &mut ShellState, message: String) {
    let finished_at_ms = crate::state::current_time_ms();
    if let Some(job) = shell.ops_job.as_mut() {
        job.status = "success".to_string();
        job.finished_at_ms = Some(finished_at_ms);
        job.message = Some(message);
    }
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
        job.finished_at_ms = Some(finished_at_ms);
        job.error = Some(error.clone());
    }
    if let Some(job) = shell.ops_job.clone() {
        shell.last_ops_job = Some(job);
        shell.ops_job = None;
    }
}