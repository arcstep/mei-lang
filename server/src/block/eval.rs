use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use mei_lang_kernel::resolve_app_root;
use mei_lang_toolchain::resolve_components_root;

use crate::graph::mcg::registry::McgRegistryWriter;
use crate::graph::types::GraphNodeKind;
use crate::graph::{hydrate_compiled_for_prebuild_eval, load_mrg_registry};
use crate::prebuild::{
    collect_required_xlsx_sources, ensure_compile_scope, ensure_request_artifacts_for_compiled,
    publish_required_data_snapshots, verify_required_xlsx_sources, CompileScope, CoverageState,
    PrebuildCoverageReport, PrebuildMode, SharedCompileOutcome,
};
use mei_lang_kernel::RuntimeWarmupApp;

use super::types::{BlockEvalReport, BlockId, BlockResult, BlockTimingMs};

#[derive(Debug, Clone)]
pub struct BlockEvalRequest {
    pub source_root: PathBuf,
    pub app_id: String,
    pub scene_id: Option<String>,
    pub target_file: Option<String>,
    pub owner_resource_id: String,
    pub metric_ids: Vec<String>,
}

/// Public SSOT: eval one or more metric worksets for a compiled scope.
pub fn materialize_worksets(
    source_root: &Path,
    app_id: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
    owner_resource_id: &str,
    metric_ids: &[String],
    mode: PrebuildMode,
) -> Result<BlockEvalReport> {
    let scope = CompileScope {
        requested_scene_id: scene_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        requested_target_file: target_file
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    }
    .canonicalized();
    materialize_worksets_for_scope(
        source_root,
        app_id,
        &scope,
        owner_resource_id,
        metric_ids,
        mode,
    )
}

pub(crate) fn materialize_worksets_with_outcome(
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    outcome: &SharedCompileOutcome,
    owner_resource_id: &str,
    metric_ids: &[String],
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<BlockEvalReport> {
    let scope_key = crate::graph::mrg_eval_scope_key(
        scope.requested_scene_id.as_deref().unwrap_or(""),
        scope.requested_target_file.as_deref(),
    );
    let started = Instant::now();
    let app_root = resolve_app_root(source_root, app_id);

    verify_owner_bundle_gate(source_root, app_id, owner_resource_id)?;

    let hydrate_started = Instant::now();
    let outcome = hydrate_outcome_for_eval(
        source_root,
        app_id,
        outcome,
        metric_ids,
        &[owner_resource_id.to_string()],
    )?;
    let hydrate_ms = hydrate_started.elapsed().as_millis() as u64;

    if mode == PrebuildMode::Build {
        ensure_snapshots_for_outcome(source_root, app_id, app_root.as_path(), &outcome)?;
    }

    let eval_started = Instant::now();
    let eval_result = ensure_request_artifacts_for_compiled(
        app_id,
        app_root.as_path(),
        &outcome,
        owner_resource_id,
        metric_ids,
        mode,
        coverage,
        state,
    );
    let eval_ms = eval_started.elapsed().as_millis() as u64;

    build_eval_report(
        app_id,
        owner_resource_id,
        metric_ids,
        scope_key,
        source_root,
        eval_result,
        coverage,
        started,
        BlockTimingMs {
            hydrate_ms,
            eval_ms,
            total_ms: started.elapsed().as_millis() as u64,
            ..Default::default()
        },
    )
}

fn hydrate_outcome_for_eval(
    source_root: &Path,
    app_id: &str,
    outcome: &SharedCompileOutcome,
    metric_ids: &[String],
    owner_resource_ids: &[String],
) -> Result<SharedCompileOutcome> {
    let mut compiled = (*outcome.compiled).clone();
    hydrate_compiled_for_prebuild_eval(
        source_root,
        app_id,
        &mut compiled,
        metric_ids,
        owner_resource_ids,
    )?;
    Ok(SharedCompileOutcome {
        compiled: Arc::new(compiled),
        ..outcome.clone()
    })
}

fn ensure_snapshots_for_outcome(
    source_root: &Path,
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
) -> Result<()> {
    let warmup_app = RuntimeWarmupApp {
        app_id: app_id.to_string(),
        ..RuntimeWarmupApp::default()
    };
    let required_xlsx =
        collect_required_xlsx_sources(&warmup_app, std::iter::once(&*outcome.compiled));
    publish_required_data_snapshots(source_root, app_id, required_xlsx.iter().cloned().collect())?;
    verify_required_xlsx_sources(app_root, &required_xlsx)
}

fn build_eval_report(
    app_id: &str,
    owner_resource_id: &str,
    metric_ids: &[String],
    scope_key: String,
    source_root: &Path,
    eval_result: Result<()>,
    coverage: &PrebuildCoverageReport,
    started: Instant,
    mut timing: BlockTimingMs,
) -> Result<BlockEvalReport> {
    let mrg = load_mrg_registry(source_root, app_id);
    let slot_state = mrg
        .slots
        .iter()
        .find(|slot| {
            slot.owner_resource_id == owner_resource_id && slot.slot_id.scope_key == scope_key
        })
        .map(|slot| format!("{:?}", slot.state));

    let block_id = BlockId {
        kind: GraphNodeKind::MaterialSlot,
        key: format!("workset|app={app_id}|owner={owner_resource_id}"),
        scope_key: Some(scope_key.clone()),
    };

    timing.total_ms = started.elapsed().as_millis() as u64;

    match eval_result {
        Ok(()) => {
            let mut result = BlockResult::ok(block_id, "eval");
            result.slot_state = slot_state;
            result.rows = Some(coverage.metric_response_artifacts_built);
            result.timing = timing;
            Ok(BlockEvalReport {
                ok: true,
                app_id: app_id.to_string(),
                scope_key,
                owner_resource_id: owner_resource_id.to_string(),
                metric_ids: metric_ids.to_vec(),
                results: vec![result],
                error_chain: None,
            })
        }
        Err(error) => Ok(BlockEvalReport {
            ok: false,
            app_id: app_id.to_string(),
            scope_key,
            owner_resource_id: owner_resource_id.to_string(),
            metric_ids: metric_ids.to_vec(),
            results: vec![BlockResult::err(block_id, "eval", &error)],
            error_chain: Some(format!("{error:#}")),
        }),
    }
}

pub(crate) fn materialize_worksets_for_scope(
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    owner_resource_id: &str,
    metric_ids: &[String],
    mode: PrebuildMode,
) -> Result<BlockEvalReport> {
    let components_root = resolve_components_root(source_root);

    verify_owner_bundle_gate(source_root, app_id, owner_resource_id)?;

    let compile_started = Instant::now();
    let outcome = ensure_compile_scope(
        source_root,
        app_id,
        scope,
        PrebuildMode::Build,
        components_root.as_path(),
    )?;
    let compile_ms = compile_started.elapsed().as_millis() as u64;

    let mut coverage = PrebuildCoverageReport::default();
    let mut state = CoverageState::default();
    state.source_root = Some(source_root.to_path_buf());
    state.app_id = Some(app_id.to_string());
    state.pre_mcg_bundle_revisions =
        crate::graph::dedup::load_mcg_bundle_revisions(source_root, app_id);

    let mut report = materialize_worksets_with_outcome(
        source_root,
        app_id,
        scope,
        &outcome,
        owner_resource_id,
        metric_ids,
        mode,
        &mut coverage,
        &state,
    )?;
    if let Some(result) = report.results.first_mut() {
        result.timing.compile_ms = compile_ms;
    }
    Ok(report)
}

pub fn block_eval(request: BlockEvalRequest) -> Result<BlockEvalReport> {
    let owner = request.owner_resource_id.trim();
    if owner.is_empty() {
        anyhow::bail!("block eval requires --owner");
    }
    let scope = CompileScope {
        requested_scene_id: request.scene_id.clone(),
        requested_target_file: request.target_file.clone(),
    }
    .canonicalized();
    materialize_worksets(
        request.source_root.as_path(),
        request.app_id.as_str(),
        request.scene_id.as_deref(),
        request.target_file.as_deref(),
        owner,
        request.metric_ids.as_slice(),
        PrebuildMode::Build,
    )
    .with_context(|| format!("block eval scope=`{}` owner=`{owner}`", scope.key()))
}

pub fn verify_owner_bundle_gate(source_root: &Path, app_id: &str, owner: &str) -> Result<()> {
    let mcg = McgRegistryWriter::load(source_root, app_id);
    let node = mcg
        .nodes
        .iter()
        .find(|node| node.id.kind == GraphNodeKind::MetricDefBundle && node.id.key == owner)
        .ok_or_else(|| anyhow!("MCG metric_def_bundle missing for owner `{owner}`"))?;
    if node.payload_ref.is_none() {
        anyhow::bail!("metric_def_bundle `{owner}` has no payloadRef");
    }
    Ok(())
}
