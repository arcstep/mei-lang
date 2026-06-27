//! Unified block/layer orchestration — SSOT entry for CLI, prebuild MRG pass, scoped build.

use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use mei_lang_toolchain::resolve_components_root;

use crate::prebuild::{
    ensure_compile_scope, ensure_compile_scope_for_prebuild, ensure_scope_artifacts, CompileScope,
    CoverageState, PrebuildCompileSession, PrebuildCoverageReport, PrebuildDiagnostics,
    PrebuildMode, ScopeArtifactPlan, SharedCompileOutcome,
};

use super::compile::{block_assemble_only, block_compile};
use super::eval::{materialize_worksets, materialize_worksets_with_outcome};
use super::inspect::block_inspect;
use super::types::{BlockEvalReport, BlockId, BlockResult};
use super::verify::block_verify;

pub struct BlockOrchestrator;

impl BlockOrchestrator {
    pub fn compile(
        source_root: &Path,
        app_id: &str,
        block_id: &BlockId,
        assemble_only: bool,
    ) -> Result<BlockResult> {
        if assemble_only && block_id.kind == crate::graph::types::GraphNodeKind::ScenePayload {
            return block_assemble_only(source_root, app_id, block_id);
        }
        block_compile(source_root, app_id, block_id)
    }

    /// MCG compile for a single scope (prebuild hot path / block-scoped pass).
    pub fn compile_scope(
        source_root: &Path,
        app_id: &str,
        scope: &CompileScope,
        mode: PrebuildMode,
        assemble_only: bool,
    ) -> Result<SharedCompileOutcome> {
        if assemble_only {
            if let Some(target) = scope
                .canonicalized()
                .requested_target_file
                .as_deref()
                .filter(|value| !value.is_empty())
            {
                let block_id = BlockId {
                    kind: crate::graph::types::GraphNodeKind::ScenePayload,
                    key: target.to_string(),
                    scope_key: scope.canonicalized().requested_scene_id.clone(),
                };
                let result = block_assemble_only(source_root, app_id, &block_id)?;
                if !result.ok {
                    return Err(anyhow::anyhow!(
                        "assemble-only compile failed for `{}`",
                        scope.key()
                    ));
                }
            }
        }
        let components_root = resolve_components_root(source_root);
        ensure_compile_scope(
            source_root,
            app_id,
            &scope.canonicalized(),
            mode,
            components_root.as_path(),
        )
    }

    /// Prebuild MCG pass with session reuse / discover policy.
    pub fn compile_scope_for_prebuild(
        session: &Mutex<PrebuildCompileSession>,
        diagnostics: &PrebuildDiagnostics,
        source_root: &Path,
        app_id: &str,
        scope: &CompileScope,
        mode: PrebuildMode,
        components_root: &Path,
    ) -> Result<SharedCompileOutcome> {
        ensure_compile_scope_for_prebuild(
            session,
            diagnostics,
            source_root,
            app_id,
            scope,
            mode,
            components_root,
        )
    }

    pub fn verify(source_root: &Path, app_id: &str, block_id: &BlockId) -> Result<BlockResult> {
        block_verify(source_root, app_id, block_id)
    }

    pub fn inspect(source_root: &Path, app_id: &str, block_id: &BlockId) -> Result<BlockResult> {
        block_inspect(source_root, app_id, block_id)
    }

    /// CLI / standalone eval (compile + hydrate + snapshot + eval).
    pub fn materialize_owner(
        source_root: &Path,
        app_id: &str,
        scene_id: Option<&str>,
        target_file: Option<&str>,
        owner_resource_id: &str,
        metric_ids: &[String],
        mode: PrebuildMode,
    ) -> Result<BlockEvalReport> {
        materialize_worksets(
            source_root,
            app_id,
            scene_id,
            target_file,
            owner_resource_id,
            metric_ids,
            mode,
        )
    }

    /// Prebuild / scoped path: reuse an already-compiled scope outcome.
    pub fn materialize_owner_with_outcome(
        source_root: &Path,
        app_id: &str,
        scope: &CompileScope,
        outcome: &SharedCompileOutcome,
        owner_resource_id: &str,
        metric_ids: &[String],
        mode: PrebuildMode,
        coverage: &mut PrebuildCoverageReport,
        state: &CoverageState,
    ) -> Result<()> {
        let report = materialize_worksets_with_outcome(
            source_root,
            app_id,
            scope,
            outcome,
            owner_resource_id,
            metric_ids,
            mode,
            coverage,
            state,
        )?;
        if report.ok {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "{}",
                report.error_chain.as_deref().unwrap_or("block eval failed")
            ))
        }
    }

    /// Batch scope artifact plan (prebuild MRG pass / scoped warm).
    pub fn materialize_scope_plan(
        app_id: &str,
        app_root: &Path,
        outcome: &SharedCompileOutcome,
        plan: &ScopeArtifactPlan,
        mode: PrebuildMode,
        coverage: &mut PrebuildCoverageReport,
        state: &CoverageState,
    ) -> Result<()> {
        ensure_scope_artifacts(app_id, app_root, outcome, plan, mode, coverage, state)
    }
}
