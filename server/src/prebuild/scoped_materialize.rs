use super::prelude::*;
use super::*;

use crate::block::{block_eval_hint, BlockOrchestrator};

#[derive(Debug, Clone, Default, Serialize)]
pub struct ScopedMaterializeReport {
    #[serde(rename = "scopeArtifactsMs")]
    pub scope_artifacts_ms: u64,
    #[serde(rename = "mrgSlotsReady")]
    pub mrg_slots_ready: usize,
    #[serde(rename = "evalArtifactsWarmed")]
    pub eval_artifacts_warmed: usize,
    #[serde(rename = "blockEvalHint", skip_serializing_if = "Option::is_none")]
    pub block_eval_hint: Option<String>,
}

/// Write-path: warm metric/dataframe artifacts for a scoped compile outcome (Build scoped rebuild).
pub fn materialize_scope_after_compile(
    source_root: &Path,
    app_id: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
    outcome: &toolchain::CompileWithCacheOutcome,
    mode: PrebuildMode,
) -> Result<ScopedMaterializeReport> {
    use crate::graph::types::MaterialState;

    let started = Instant::now();
    let app_root = resolve_app_root(source_root, app_id);
    let shared = SharedCompileOutcome {
        compiled: Arc::new(outcome.compiled.clone()),
        cache_hit: outcome.cache_hit,
        artifact_cache_hit: outcome.artifact_cache_hit,
        compile_revision: outcome.compile_revision.clone(),
        cache_lookup_ms: outcome.cache_lookup_ms,
        artifact_load_ms: outcome.artifact_load_ms,
        compile_ms: outcome.compile_ms,
    };
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
    let mut coverage = PrebuildCoverageReport::default();
    let mut state = CoverageState::default();
    state.source_root = Some(source_root.to_path_buf());
    state.app_id = Some(app_id.to_string());
    state.pre_mcg_bundle_revisions =
        crate::graph::dedup::load_mcg_bundle_revisions(source_root, app_id);

    let scope_profile = PrebuildScopeProfile::HotOnly;
    let frontier = build_mrg_eval_frontier(source_root, app_id, scope_profile);
    tracing::info!(
        target: "mei.scoped_build",
        app_id = %app_id,
        scene_id = ?scene_id,
        target_file = ?target_file,
        dirty_slot_count = frontier.dirty_slot_count,
        plan_source = frontier.plan_source,
        "[SCOPED-BUILD] warming scope artifacts"
    );

    let workspace_flag = format!("--workspace {}", source_root.display());
    let block_eval_hint = block_eval_hint(
        workspace_flag.as_str(),
        app_id,
        scene_id,
        target_file,
        "<owner>",
        &[],
    );

    let mut warmed_via_plan = false;
    if let Ok(Some(manifest)) = resolve_runtime_warmup_manifest(source_root) {
        if let Some(app) = manifest.apps.iter().find(|entry| entry.app_id == app_id) {
            let plan = build_prebuild_manifest_plan(app, scope_profile);
            let matching = matching_warmup_requests_for_outcome(&plan.warmup_requests, &shared);
            if !matching.is_empty() {
                let mut scope_plan = build_scope_artifact_plan(
                    source_root,
                    app_id,
                    app_root.as_path(),
                    &scope,
                    &shared,
                    matching.as_slice(),
                    scope_profile,
                    plan.warmup_requests.as_slice(),
                )?;
                if frontier.dirty_slot_count > 0 {
                    retain_dirty_scope_plan(&mut scope_plan, &frontier);
                }
                if !scope_plan.metric_worksets.is_empty()
                    || !scope_plan.dataframe_artifacts.is_empty()
                {
                    BlockOrchestrator::materialize_scope_plan(
                        app_id,
                        app_root.as_path(),
                        &shared,
                        &scope_plan,
                        mode,
                        &mut coverage,
                        &state,
                    )?;
                    warmed_via_plan = true;
                }
            }
        }
    }

    if !warmed_via_plan {
        for resource in &shared.compiled.resources {
            let Some(dataset) = resource.dataset.as_ref() else {
                continue;
            };
            if !dataset.has_runtime_metric_defs() {
                continue;
            }
            let _ = ensure_request_artifacts_for_compiled(
                app_id,
                app_root.as_path(),
                &shared,
                resource.id.as_str(),
                &[],
                mode,
                &mut coverage,
                &state,
            );
        }
        if compiled_has_world_metrics_runtime_defs(&shared.compiled) {
            let _ = ensure_request_artifacts_for_compiled(
                app_id,
                app_root.as_path(),
                &shared,
                "__world_metrics__",
                &[],
                mode,
                &mut coverage,
                &state,
            );
        }
    }

    if let (Some(scene), Some(target)) = (
        scope.requested_scene_id.as_deref(),
        scope.requested_target_file.as_deref(),
    ) {
        let _ = crate::graph::mrg::navigation::sync_navigation_for_compile_scopes(
            source_root,
            app_id,
            &[crate::graph::mrg::navigation::CompileScopeNav {
                scene_id: scene.to_string(),
                target_file: target.to_string(),
            }],
        );
    }
    if let Err(error) = patch_prebuild_compile_index_entry(source_root, app_id, &scope, &shared) {
        tracing::warn!(target: "mei.scoped_build", app_id = %app_id, error = %error, "patch scoped compile index failed");
    }
    if let Err(error) = crate::prebuild_fingerprint::bump_scoped_prebuild_timestamp(source_root, app_id) {
        tracing::warn!(target: "mei.scoped_build", app_id = %app_id, error = %error, "scoped prebuild fingerprint bump failed");
    }

    let eval_artifacts_warmed = coverage
        .metric_response_artifacts_built
        .saturating_add(coverage.metric_dataframe_artifacts_built)
        .saturating_add(coverage.metric_response_artifacts_skipped_bundle_unchanged)
        .saturating_add(coverage.metric_dataframe_artifacts_skipped_bundle_unchanged);
    let mrg_slots_ready = if crate::graph::feature::graph_registry_dedup_enabled() {
        crate::graph::load_mrg_registry(source_root, app_id)
            .slots
            .iter()
            .filter(|slot| slot.state == MaterialState::Ready)
            .count()
    } else {
        0
    };

    tracing::info!(
        target: "mei.scoped_build",
        app_id = %app_id,
        eval_warmed = eval_artifacts_warmed,
        mrg_slots_ready,
        scope_ms = started.elapsed().as_millis() as u64,
        hint = %block_eval_hint,
        "[SCOPED-BUILD] complete"
    );

    Ok(ScopedMaterializeReport {
        scope_artifacts_ms: started.elapsed().as_millis() as u64,
        mrg_slots_ready,
        eval_artifacts_warmed,
        block_eval_hint: Some(block_eval_hint),
    })
}
