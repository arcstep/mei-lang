use super::prelude::*;
use super::*;

#[derive(Debug, Clone, Default, Serialize)]
pub struct ScopedMaterializeReport {
    #[serde(rename = "scopeArtifactsMs")]
    pub scope_artifacts_ms: u64,
    #[serde(rename = "mrgSlotsReady")]
    pub mrg_slots_ready: usize,
    #[serde(rename = "evalArtifactsWarmed")]
    pub eval_artifacts_warmed: usize,
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
    };
    let mut coverage = PrebuildCoverageReport::default();
    let mut state = CoverageState::default();
    state.source_root = Some(source_root.to_path_buf());
    state.app_id = Some(app_id.to_string());
    state.pre_mcg_bundle_revisions =
        crate::graph::dedup::load_mcg_bundle_revisions(source_root, app_id);

    let mut warmed_via_plan = false;
    if let Ok(Some(manifest)) = resolve_runtime_warmup_manifest(source_root) {
        if let Some(app) = manifest.apps.iter().find(|entry| entry.app_id == app_id) {
            let plan = build_prebuild_manifest_plan(app, PrebuildScopeProfile::Full);
            let matching =
                matching_warmup_requests_for_outcome(&plan.warmup_requests, &shared);
            if !matching.is_empty() {
                let scope_plan = build_scope_artifact_plan(
                    app_id,
                    app_root.as_path(),
                    &scope,
                    &shared,
                    matching.as_slice(),
                )?;
                ensure_scope_artifacts(
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

    Ok(ScopedMaterializeReport {
        scope_artifacts_ms: started.elapsed().as_millis() as u64,
        mrg_slots_ready,
        eval_artifacts_warmed,
    })
}

