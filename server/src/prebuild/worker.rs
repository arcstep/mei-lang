use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::prelude::*;
use super::*;

use crate::block::BlockOrchestrator;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrebuildWorkerReport {
    pub ok: bool,
    pub wall_ms: u64,
    pub worker_peak_rss_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub coverage: PrebuildCoverageReport,
}

pub(crate) fn prebuild_subprocess_isolate_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MEI_PREBUILD_ISOLATE")
            .ok()
            .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
            .unwrap_or_else(|| {
                std::env::var("MEI_PREBUILD_ISOLATE_ARTIFACT")
                    .ok()
                    .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
                    .unwrap_or(true)
            })
    })
}

pub(crate) fn prebuild_isolate_compile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MEI_PREBUILD_ISOLATE_COMPILE")
            .ok()
            .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
            .unwrap_or(true)
    })
}

pub(crate) fn run_prebuild_worker_if_requested(
    args: &crate::cli::args::PrebuildArgs,
) -> Result<bool> {
    let Some(task) = std::env::var("MEI_PREBUILD_WORKER")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(false);
    };
    let package_root = crate::cli::util::resolve_package_root()?;
    let raw_source_root = crate::cli::util::resolve_source_root_arg(
        &package_root,
        args.workspace.as_deref(),
        &args.source_root,
    )?;
    let source_root = crate::cli::util::resolve_cli_source_root(&package_root, &raw_source_root)?;
    match task.as_str() {
        "materialize-scope" => {
            let app_id = std::env::var("MEI_PREBUILD_WORKER_APP_ID").context("worker app id")?;
            let scope_scene = std::env::var("MEI_PREBUILD_WORKER_SCOPE_SCENE").ok();
            let scope_target = std::env::var("MEI_PREBUILD_WORKER_SCOPE_TARGET").ok();
            let plan_file = std::env::var("MEI_PREBUILD_WORKER_PLAN_FILE")
                .context("MEI_PREBUILD_WORKER_PLAN_FILE required")?;
            let scope_plan = serde_json::from_str::<ScopeArtifactPlanWire>(
                &fs::read_to_string(plan_file.as_str())
                    .with_context(|| format!("read {plan_file}"))?,
            )?
            .into_scope_plan();
            let report = materialize_scope_worker(
                source_root.as_path(),
                app_id.as_str(),
                scope_scene,
                scope_target,
                &scope_plan,
            )?;
            println!("{}", serde_json::to_string(&report)?);
            if !report.ok {
                std::process::exit(1);
            }
        }
        "compile-scope" => {
            let app_id = std::env::var("MEI_PREBUILD_WORKER_APP_ID").context("worker app id")?;
            let scope_scene = std::env::var("MEI_PREBUILD_WORKER_SCOPE_SCENE").ok();
            let scope_target = std::env::var("MEI_PREBUILD_WORKER_SCOPE_TARGET").ok();
            let report = compile_scope_worker(
                source_root.as_path(),
                app_id.as_str(),
                scope_scene,
                scope_target,
            )?;
            println!("{}", serde_json::to_string(&report)?);
            if !report.ok {
                std::process::exit(1);
            }
        }
        other => anyhow::bail!("unknown MEI_PREBUILD_WORKER task `{other}`"),
    }
    Ok(true)
}

fn materialize_scope_worker(
    source_root: &Path,
    app_id: &str,
    scene_id: Option<String>,
    target_file: Option<String>,
    scope_plan: &ScopeArtifactPlan,
) -> Result<PrebuildWorkerReport> {
    let started = Instant::now();
    let scope = CompileScope {
        requested_scene_id: scene_id,
        requested_target_file: target_file,
    }
    .canonicalized();
    let app_root = resolve_app_root(source_root, app_id);
    let mut peak = current_process_rss_bytes().unwrap_or(0);
    let mut coverage = PrebuildCoverageReport::default();
    let mut state = CoverageState::default();
    state.source_root = Some(source_root.to_path_buf());
    state.app_id = Some(app_id.to_string());
    state.pre_mcg_bundle_revisions =
        crate::graph::dedup::load_mcg_bundle_revisions(source_root, app_id);
    let target = scope
        .requested_target_file
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            scope
                .requested_scene_id
                .as_deref()
                .map(|scene| resolve_scene_assembly_rel(app_root.as_path(), scene))
        })
        .unwrap_or_default();
    let scene = scope.requested_scene_id.as_deref();
    let (mut compiled, compile_revision) = crate::graph::try_assemble_scope_from_scene_payload(
        source_root,
        app_id,
        scene,
        target.as_str(),
    )
    .with_context(|| format!("assemble scope `{}` for worker", scope.key()))?;
    let _ = crate::graph::hydrate_compiled_for_prebuild_eval(
        source_root,
        app_id,
        &mut compiled,
        &[],
        &[],
    );
    let outcome = SharedCompileOutcome {
        compiled: Arc::new(compiled),
        cache_hit: true,
        artifact_cache_hit: false,
        assemble_only: true,
        compile_revision,
        cache_lookup_ms: 0,
        artifact_load_ms: 0,
        compile_ms: 0,
        handle_only: false,
        assembly_handle: None,
    };
    let result = BlockOrchestrator::materialize_scope_plan(
        app_id,
        app_root.as_path(),
        &outcome,
        scope_plan,
        PrebuildMode::Build,
        &mut coverage,
        &state,
    );
    if let Some(rss) = current_process_rss_bytes() {
        peak = peak.max(rss);
    }
    state.clear();
    match result {
        Ok(()) => Ok(PrebuildWorkerReport {
            ok: true,
            wall_ms: started.elapsed().as_millis() as u64,
            worker_peak_rss_bytes: peak,
            error: None,
            coverage,
        }),
        Err(error) => Ok(PrebuildWorkerReport {
            ok: false,
            wall_ms: started.elapsed().as_millis() as u64,
            worker_peak_rss_bytes: peak,
            error: Some(format!("{error:#}")),
            coverage,
        }),
    }
}

fn compile_scope_worker(
    source_root: &Path,
    app_id: &str,
    scene_id: Option<String>,
    target_file: Option<String>,
) -> Result<PrebuildWorkerReport> {
    let started = Instant::now();
    let scope = CompileScope {
        requested_scene_id: scene_id,
        requested_target_file: target_file,
    }
    .canonicalized();
    let components_root = toolchain::resolve_components_root(source_root);
    let session = Mutex::new(PrebuildCompileSession::default());
    let diagnostics = PrebuildDiagnostics::default();
    let result = BlockOrchestrator::compile_scope_for_prebuild(
        &session,
        &diagnostics,
        source_root,
        app_id,
        &scope,
        PrebuildMode::Build,
        components_root.as_path(),
    );
    diagnostics.sample_memory_peak();
    let peak = diagnostics
        .peak_rss_bytes
        .load(Ordering::Relaxed)
        .max(current_process_rss_bytes().unwrap_or(0) as usize) as u64;
    match result {
        Ok(outcome) => {
            record_prebuild_scope_compile(
                source_root,
                app_id,
                &session,
                &scope,
                &outcome,
                &mut BTreeSet::new(),
                &mut std::collections::VecDeque::new(),
                &mut Vec::new(),
                &mut Vec::new(),
            );
            Ok(PrebuildWorkerReport {
                ok: true,
                wall_ms: started.elapsed().as_millis() as u64,
                worker_peak_rss_bytes: peak.max(current_process_rss_bytes().unwrap_or(0)),
                error: None,
                coverage: PrebuildCoverageReport::default(),
            })
        }
        Err(error) => Ok(PrebuildWorkerReport {
            ok: false,
            wall_ms: started.elapsed().as_millis() as u64,
            worker_peak_rss_bytes: peak,
            error: Some(format!("{error:#}")),
            coverage: PrebuildCoverageReport::default(),
        }),
    }
}

pub(crate) fn spawn_materialize_scope_worker(
    source_root: &Path,
    app_id: &str,
    prepared: &PreparedCompileOutcome,
    scope_plan: &ScopeArtifactPlan,
    diagnostics: &PrebuildDiagnostics,
) -> Result<PrebuildWorkerReport> {
    let exe = std::env::current_exe().context("resolve mei-toolchain exe")?;
    let plan_path = write_scope_plan_file(source_root, app_id, scope_plan)?;
    let output = Command::new(exe)
        .arg("prebuild")
        .arg("--source-root")
        .arg(source_root)
        .arg("--app")
        .arg(app_id)
        .env("MEI_PREBUILD_WORKER", "materialize-scope")
        .env("MEI_PREBUILD_WORKER_APP_ID", app_id)
        .env(
            "MEI_PREBUILD_WORKER_SCOPE_SCENE",
            prepared.scope.requested_scene_id.as_deref().unwrap_or(""),
        )
        .env(
            "MEI_PREBUILD_WORKER_SCOPE_TARGET",
            prepared
                .scope
                .requested_target_file
                .as_deref()
                .unwrap_or(""),
        )
        .env("MEI_PREBUILD_WORKER_PLAN_FILE", plan_path.as_os_str())
        .output()
        .with_context(|| {
            format!(
                "spawn materialize worker for scope `{}`",
                prepared.scope.key()
            )
        })?;
    let _ = fs::remove_file(plan_path.as_path());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let report =
        serde_json::from_str::<PrebuildWorkerReport>(stdout.trim()).with_context(|| {
            format!(
                "parse materialize worker stdout (status={}): {stdout}; stderr: {stderr}",
                output.status
            )
        })?;
    diagnostics.note_worker_peak_rss(report.worker_peak_rss_bytes);
    if !output.status.success() && report.ok {
        anyhow::bail!(
            "materialize worker exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(report)
}

fn write_scope_plan_file(
    source_root: &Path,
    app_id: &str,
    scope_plan: &ScopeArtifactPlan,
) -> Result<PathBuf> {
    let dir = source_root.join("runtime").join(".prebuild-worker");
    fs::create_dir_all(dir.as_path())?;
    let path = dir.join(format!("scope-plan-{}-{}.json", app_id, now_epoch_ms()));
    let wire = ScopeArtifactPlanWire::from_scope_plan(scope_plan);
    fs::write(path.as_path(), serde_json::to_string(&wire)?)?;
    Ok(path)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScopeArtifactPlanWire {
    pub metric_worksets: Vec<PlannedMetricWorksetWire>,
    pub dataframe_artifacts: Vec<PlannedDataframeArtifactWire>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlannedMetricWorksetWire {
    pub logical_node_id: String,
    pub scope_id: String,
    pub materialization_key: String,
    pub dataset_selector: String,
    pub owner_resource_id: String,
    pub requested_metric_ids: Vec<String>,
    pub request_all_metrics: bool,
    pub scene_id: String,
    pub scene_path: Option<String>,
    pub dependency_revision_key: String,
    pub response_cache_key: String,
    pub shared_cache_key: String,
    pub covered_metric_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlannedDataframeArtifactWire {
    pub logical_node_id: String,
    pub scope_id: String,
    pub materialization_key: String,
    pub artifact_key: String,
    pub shared_artifact_key: String,
    pub owner_resource_id: String,
    pub resource_selector_id: String,
    pub dataframe_metric_id: String,
    pub resolved_metric_id: String,
    pub page_size: usize,
    pub scene_id: String,
    pub scene_path: Option<String>,
    pub dependency_revision_key: String,
    pub scope_metric_token: String,
}

impl ScopeArtifactPlanWire {
    fn from_scope_plan(plan: &ScopeArtifactPlan) -> Self {
        Self {
            metric_worksets: plan
                .metric_worksets
                .iter()
                .map(|workset| PlannedMetricWorksetWire {
                    logical_node_id: workset.logical_node_id.clone(),
                    scope_id: workset.scope_id.clone(),
                    materialization_key: workset.materialization_key.clone(),
                    dataset_selector: workset.dataset_selector.clone(),
                    owner_resource_id: workset.owner_resource_id.clone(),
                    requested_metric_ids: workset.requested_metric_ids.clone(),
                    request_all_metrics: workset.request_all_metrics,
                    scene_id: workset.scene_id.clone(),
                    scene_path: workset.scene_path.clone(),
                    dependency_revision_key: workset.dependency_revision_key.clone(),
                    response_cache_key: workset.response_cache_key.clone(),
                    shared_cache_key: workset.shared_cache_key.clone(),
                    covered_metric_ids: workset.covered_metric_ids.iter().cloned().collect(),
                })
                .collect(),
            dataframe_artifacts: plan
                .dataframe_artifacts
                .iter()
                .map(|artifact| PlannedDataframeArtifactWire {
                    logical_node_id: artifact.logical_node_id.clone(),
                    scope_id: artifact.scope_id.clone(),
                    materialization_key: artifact.materialization_key.clone(),
                    artifact_key: artifact.artifact_key.clone(),
                    shared_artifact_key: artifact.shared_artifact_key.clone(),
                    owner_resource_id: artifact.owner_resource_id.clone(),
                    resource_selector_id: artifact.resource_selector_id.clone(),
                    dataframe_metric_id: artifact.dataframe_metric_id.clone(),
                    resolved_metric_id: artifact.resolved_metric_id.clone(),
                    page_size: artifact.page_size,
                    scene_id: artifact.scene_id.clone(),
                    scene_path: artifact.scene_path.clone(),
                    dependency_revision_key: artifact.dependency_revision_key.clone(),
                    scope_metric_token: artifact.scope_metric_token.clone(),
                })
                .collect(),
        }
    }

    fn into_scope_plan(self) -> ScopeArtifactPlan {
        ScopeArtifactPlan {
            metric_worksets: self
                .metric_worksets
                .into_iter()
                .map(|workset| PlannedMetricWorkset {
                    logical_node_id: workset.logical_node_id,
                    scope_id: workset.scope_id,
                    materialization_key: workset.materialization_key,
                    dataset_selector: workset.dataset_selector,
                    owner_resource_id: workset.owner_resource_id,
                    requested_metric_ids: workset.requested_metric_ids,
                    request_all_metrics: workset.request_all_metrics,
                    scene_id: workset.scene_id,
                    scene_path: workset.scene_path,
                    dependency_revision_key: workset.dependency_revision_key,
                    response_cache_key: workset.response_cache_key,
                    shared_cache_key: workset.shared_cache_key,
                    covered_metric_ids: workset.covered_metric_ids.into_iter().collect(),
                    defs_for_hydrate: Arc::new(BTreeMap::new()),
                })
                .collect(),
            dataframe_artifacts: self
                .dataframe_artifacts
                .into_iter()
                .map(|artifact| PlannedDataframeArtifact {
                    logical_node_id: artifact.logical_node_id,
                    scope_id: artifact.scope_id,
                    materialization_key: artifact.materialization_key,
                    artifact_key: artifact.artifact_key,
                    shared_artifact_key: artifact.shared_artifact_key,
                    owner_resource_id: artifact.owner_resource_id,
                    resource_selector_id: artifact.resource_selector_id,
                    dataframe_metric_id: artifact.dataframe_metric_id,
                    resolved_metric_id: artifact.resolved_metric_id,
                    page_size: artifact.page_size,
                    scene_id: artifact.scene_id,
                    scene_path: artifact.scene_path,
                    dependency_revision_key: artifact.dependency_revision_key,
                    scope_metric_token: artifact.scope_metric_token,
                    defs_for_hydrate: Arc::new(BTreeMap::new()),
                })
                .collect(),
        }
    }
}

pub(crate) fn spawn_compile_scope_worker(
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    diagnostics: &PrebuildDiagnostics,
) -> Result<SharedCompileOutcome> {
    let exe = std::env::current_exe().context("resolve mei-toolchain exe")?;
    let output = Command::new(exe)
        .arg("prebuild")
        .arg("--source-root")
        .arg(source_root)
        .arg("--app")
        .arg(app_id)
        .env("MEI_PREBUILD_WORKER", "compile-scope")
        .env("MEI_PREBUILD_WORKER_APP_ID", app_id)
        .env(
            "MEI_PREBUILD_WORKER_SCOPE_SCENE",
            scope.requested_scene_id.as_deref().unwrap_or(""),
        )
        .env(
            "MEI_PREBUILD_WORKER_SCOPE_TARGET",
            scope.requested_target_file.as_deref().unwrap_or(""),
        )
        .output()
        .with_context(|| format!("spawn compile worker for scope `{}`", scope.key()))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let report = serde_json::from_str::<PrebuildWorkerReport>(stdout.trim())
        .with_context(|| format!("parse compile worker stdout: {stdout}"))?;
    diagnostics.note_worker_peak_rss(report.worker_peak_rss_bytes);
    if !report.ok {
        anyhow::bail!(report
            .error
            .unwrap_or_else(|| "compile scope worker failed".to_string()));
    }
    let components_root = toolchain::resolve_components_root(source_root);
    let session = Mutex::new(PrebuildCompileSession::default());
    let diag = PrebuildDiagnostics::default();
    let compile_index =
        load_prebuild_compile_index(resolve_app_root(source_root, app_id).as_path())?;
    try_reuse_persisted_compile_index(
        &session,
        &diag,
        compile_index.as_ref(),
        source_root,
        app_id,
        scope,
        components_root.as_path(),
    )
    .map(|reuse| reuse.outcome)
    .or_else(|| {
        BlockOrchestrator::compile_scope_for_prebuild(
            &session,
            &diag,
            source_root,
            app_id,
            scope,
            PrebuildMode::Build,
            components_root.as_path(),
        )
        .ok()
    })
    .ok_or_else(|| anyhow::anyhow!("compile worker finished but parent could not load outcome"))
}
