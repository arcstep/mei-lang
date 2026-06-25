use std::path::Path;
use std::sync::Arc;

use axum::http::HeaderMap;
use mei_lang_app::UiRouteMode;
use mei_lang_kernel::{CompiledApp, CompileOptions, Severity};
use mei_lang_toolchain as toolchain;

use crate::AppState;

pub(crate) use toolchain::CompileWithCacheOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeArtifactPolicy {
    SealedStrict,
    ArtifactFirstFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeAssemblyPolicy {
    Sealed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeEvalPolicy {
    ArtifactFirstThin,
    SealedStrict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAccessPolicies {
    pub assembly: RuntimeAssemblyPolicy,
    pub eval: RuntimeEvalPolicy,
}

impl RuntimeAccessPolicies {
    pub(crate) fn from_headers(_headers: &HeaderMap) -> Self {
        let assembly = match env_ascii("MEI_ACCESS_ASSEMBLY_POLICY")
            .or_else(|| env_ascii("MEI_RUNTIME_ASSEMBLY_POLICY"))
            .as_deref()
        {
            Some("sealed") | None => RuntimeAssemblyPolicy::Sealed,
            Some(other) => {
                tracing::warn!(
                    policy = other,
                    "unknown MEI_ACCESS_ASSEMBLY_POLICY; defaulting to sealed AOT"
                );
                RuntimeAssemblyPolicy::Sealed
            }
        };
        let eval = match env_ascii("MEI_ACCESS_EVAL_POLICY")
            .or_else(|| env_ascii("MEI_RUNTIME_EVAL_POLICY"))
            .or_else(|| env_ascii("MEI_RUNTIME_ARTIFACT_POLICY"))
            .or_else(|| env_ascii("MEI_ACCESS_ARTIFACT_POLICY"))
            .as_deref()
        {
            Some("sealed" | "sealed_strict" | "strict" | "aot_strict") => {
                RuntimeEvalPolicy::SealedStrict
            }
            Some("artifact_first_thin") | None => RuntimeEvalPolicy::ArtifactFirstThin,
            Some(other) => {
                tracing::warn!(
                    policy = other,
                    "unknown MEI_ACCESS_EVAL_POLICY; defaulting to artifact_first_thin"
                );
                RuntimeEvalPolicy::ArtifactFirstThin
            }
        };
        Self { assembly, eval }
    }

    pub(crate) fn allows_thin_eval(self) -> bool {
        matches!(self.eval, RuntimeEvalPolicy::ArtifactFirstThin)
    }

    pub(crate) fn legacy_runtime_artifact_policy(self) -> RuntimeArtifactPolicy {
        if matches!(self.eval, RuntimeEvalPolicy::SealedStrict) {
            return RuntimeArtifactPolicy::SealedStrict;
        }
        RuntimeArtifactPolicy::ArtifactFirstFallback
    }

    pub(crate) fn default_for_access_host() -> Self {
        Self {
            assembly: RuntimeAssemblyPolicy::Sealed,
            eval: RuntimeEvalPolicy::ArtifactFirstThin,
        }
    }
}

impl RuntimeArtifactPolicy {
    pub(crate) fn from_headers(headers: &HeaderMap) -> Self {
        RuntimeAccessPolicies::from_headers(headers).legacy_runtime_artifact_policy()
    }

    pub(crate) fn is_artifact_first_fallback(self) -> bool {
        matches!(self, Self::ArtifactFirstFallback)
    }

    pub(crate) fn is_sealed_strict(self) -> bool {
        matches!(self, Self::SealedStrict)
    }
}

pub(crate) struct RuntimeCompileResolution {
    pub(crate) outcome: toolchain::CompileWithCacheOutcomeShared,
    pub(crate) policy: RuntimeArtifactPolicy,
    pub(crate) access_policies: RuntimeAccessPolicies,
    pub(crate) correctness_fallback: bool,
    pub(crate) artifact_backfilled: bool,
}

fn env_ascii(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

pub(crate) fn load_compile_artifact_only(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Option<CompileWithCacheOutcome> {
    toolchain::load_compile_artifact_only(&state.source_root, app_id, options, components_root)
}

/// Build preview targets such as stock authoring examples are outside app scene routes;
/// artifact-first loads may patch `active_target_file` on a parent scope without updating
/// `scene_contract`. Those previews must run a scoped compile.
pub(crate) fn build_preview_target_requires_scoped_compile(
    compiled: &CompiledApp,
    preview_target: &str,
) -> bool {
    let target = preview_target.trim();
    if target.is_empty() {
        return false;
    }
    if target.ends_with(".world.mei") || target.ends_with(".board.mei") {
        return false;
    }
    if compiled
        .scene_routes
        .iter()
        .any(|route| route.target_file.trim() == target)
    {
        return compiled.scene_contract.is_none();
    }
    true
}

pub(crate) fn build_preview_diagnostic_error_count(compiled: &CompiledApp) -> usize {
    compiled
        .diagnostics
        .iter()
        .filter(|diag| diag.severity == Severity::Error)
        .count()
}

/// Resolve build-view preview compile: reuse warmed artifacts when they truly match the
/// requested target; otherwise compile the preview scope on demand.
pub(crate) fn resolve_build_preview_compile(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Result<Option<CompileWithCacheOutcome>, toolchain::CompileWithCacheFailure> {
    let preview_target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(preview_target) = preview_target else {
        return Ok(resolve_runtime_compile_shared(
            state,
            app_id,
            options,
            components_root,
            RuntimeAccessPolicies::default_for_access_host(),
            UiRouteMode::Build,
        )
        .ok()
        .flatten()
        .map(|resolution| compile_outcome_from_shared(resolution.outcome)));
    };

    if let Ok(Some(resolution)) = resolve_runtime_compile_shared(
        state,
        app_id,
        options,
        components_root,
        RuntimeAccessPolicies::default_for_access_host(),
        UiRouteMode::Build,
    ) {
        let compiled = resolution.outcome.compiled.as_ref();
        if !build_preview_target_requires_scoped_compile(compiled, preview_target) {
            return Ok(Some(compile_outcome_from_shared(resolution.outcome)));
        }
    }

    compile_app_with_cache(state, app_id, options, components_root).map(Some)
}

pub(crate) fn compile_app_with_cache(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Result<CompileWithCacheOutcome, toolchain::CompileWithCacheFailure> {
    let outcome = toolchain::compile_app_with_cache(
        &state.source_root,
        app_id,
        options.clone(),
        components_root,
    )?;
    let payloads = crate::graph::runtime_payloads_from_compiled(&outcome.compiled);
    crate::graph::maybe_update_graph_after_compile(
        state.source_root.as_ref().as_path(),
        app_id,
        options,
        &outcome.compiled,
        outcome.compile_revision.as_str(),
        &payloads,
    );
    Ok(outcome)
}

pub(crate) fn load_compile_artifact_only_shared(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Option<toolchain::CompileWithCacheOutcomeShared> {
    toolchain::load_compile_artifact_only_shared(
        &state.source_root,
        app_id,
        options,
        components_root,
    )
}

pub(crate) fn resolve_runtime_compile_shared(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
    access_policies: RuntimeAccessPolicies,
    route_mode: UiRouteMode,
) -> Result<Option<RuntimeCompileResolution>, toolchain::CompileWithCacheFailure> {
    use crate::http::pages::AppQuery;
    use crate::readiness::scope_gate::resolve_scope_gate;

    let query = AppQuery {
        file: options.preview_target.clone(),
        scene: options.scene.clone(),
        tab: None,
        diag_filter: None,
        world_metric: None,
        world_dataset: None,
        explain: None,
        node: None,
        scope: None,
        focus: None,
        chrome: None,
    };
    let gate = resolve_scope_gate(
        state.source_root.as_path(),
        app_id,
        route_mode,
        options.scene.as_deref(),
        &query,
    );
    if !gate.shell_ready {
        return Ok(None);
    }

    let policy = access_policies.legacy_runtime_artifact_policy();
    if let Some(target) = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some((compiled, compile_revision)) =
            crate::graph::try_assemble_scope_from_scene_payload(
                state.source_root.as_path(),
                app_id,
                options.scene.as_deref(),
                target,
            )
        {
            return Ok(Some(RuntimeCompileResolution {
                outcome: toolchain::CompileWithCacheOutcomeShared {
                    compiled: Arc::new(compiled),
                    cache_hit: true,
                    artifact_cache_hit: false,
                    compile_revision,
                    revision_scope: "mcg_scene_payload".to_string(),
                    cache_validation: "mcg_assemble".to_string(),
                    cache_lookup_ms: 0,
                    artifact_load_ms: 0,
                    compile_cache_lock_wait_ms: 0,
                    compile_ms: 0,
                },
                policy,
                access_policies,
                correctness_fallback: false,
                artifact_backfilled: false,
            }));
        }
    }
    if let Some(outcome) =
        load_compile_artifact_only_shared(state, app_id, options, components_root)
    {
        let mut compiled = (*outcome.compiled).clone();
        if compiled.world_metrics.is_empty() {
            if let Some(target) = options
                .preview_target
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                crate::graph::hydrate_world_metrics_from_scene_payload(
                    state.source_root.as_path(),
                    app_id,
                    target,
                    &mut compiled,
                );
            }
        }
        return Ok(Some(RuntimeCompileResolution {
            outcome: toolchain::CompileWithCacheOutcomeShared {
                compiled: Arc::new(compiled),
                cache_hit: outcome.cache_hit,
                artifact_cache_hit: outcome.artifact_cache_hit,
                compile_revision: outcome.compile_revision,
                revision_scope: outcome.revision_scope,
                cache_validation: outcome.cache_validation,
                cache_lookup_ms: outcome.cache_lookup_ms,
                artifact_load_ms: outcome.artifact_load_ms,
                compile_cache_lock_wait_ms: outcome.compile_cache_lock_wait_ms,
                compile_ms: outcome.compile_ms,
            },
            policy,
            access_policies,
            correctness_fallback: false,
            artifact_backfilled: false,
        }));
    }
    Ok(None)
}

pub(crate) fn compile_outcome_from_shared(
    outcome: toolchain::CompileWithCacheOutcomeShared,
) -> CompileWithCacheOutcome {
    CompileWithCacheOutcome {
        compiled: (*outcome.compiled).clone(),
        cache_hit: outcome.cache_hit,
        artifact_cache_hit: outcome.artifact_cache_hit,
        compile_revision: outcome.compile_revision,
        revision_scope: outcome.revision_scope,
        cache_validation: outcome.cache_validation,
        cache_lookup_ms: outcome.cache_lookup_ms,
        artifact_load_ms: outcome.artifact_load_ms,
        compile_cache_lock_wait_ms: outcome.compile_cache_lock_wait_ms,
        compile_ms: outcome.compile_ms,
    }
}

pub(crate) fn clear_compile_cache_for_app(state: &AppState, app_id: &str) -> usize {
    toolchain::clear_compile_cache_for_app(&state.source_root, app_id)
}

pub(crate) fn access_import_required() -> bool {
    mei_lang_kernel::access_parquet_import_required()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn build_view_header_does_not_change_policy() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-mei-build-view",
            axum::http::HeaderValue::from_static("1"),
        );
        let policies = RuntimeAccessPolicies::from_headers(&headers);
        assert_eq!(policies.assembly, RuntimeAssemblyPolicy::Sealed);
        assert_eq!(policies.eval, RuntimeEvalPolicy::ArtifactFirstThin);
        assert_eq!(
            RuntimeArtifactPolicy::from_headers(&headers),
            RuntimeArtifactPolicy::ArtifactFirstFallback
        );
    }

    #[test]
    fn access_defaults_to_sealed_assembly_and_thin_eval() {
        let headers = HeaderMap::new();
        let policies = RuntimeAccessPolicies::from_headers(&headers);
        assert_eq!(policies.assembly, RuntimeAssemblyPolicy::Sealed);
        assert_eq!(policies.eval, RuntimeEvalPolicy::ArtifactFirstThin);
        assert!(policies.allows_thin_eval());
    }

    #[test]
    fn runtime_policy_defaults_access_to_artifact_first_fallback() {
        let headers = HeaderMap::new();
        assert_eq!(
            RuntimeArtifactPolicy::from_headers(&headers),
            RuntimeArtifactPolicy::ArtifactFirstFallback
        );
    }

    #[test]
    fn access_import_required_for_default_access_host() {
        assert!(access_import_required());
    }
}
