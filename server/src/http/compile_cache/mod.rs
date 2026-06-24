use axum::http::HeaderMap;
use mei_lang_kernel::CompileOptions;
use mei_lang_toolchain as toolchain;

use crate::AppState;

pub(crate) use toolchain::CompileWithCacheOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeArtifactPolicy {
    SealedStrict,
    ArtifactFirstFallback,
    BuildViewJit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeAssemblyPolicy {
    Sealed,
    Jit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeEvalPolicy {
    ArtifactFirstThin,
    SealedStrict,
    Jit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeAccessPolicies {
    pub assembly: RuntimeAssemblyPolicy,
    pub eval: RuntimeEvalPolicy,
}

impl RuntimeAccessPolicies {
    pub(crate) fn from_headers(headers: &HeaderMap) -> Self {
        if is_build_view_request(headers) {
            return Self {
                assembly: RuntimeAssemblyPolicy::Jit,
                eval: RuntimeEvalPolicy::Jit,
            };
        }
        let assembly = match env_ascii("MEI_ACCESS_ASSEMBLY_POLICY")
            .or_else(|| env_ascii("MEI_RUNTIME_ASSEMBLY_POLICY"))
            .as_deref()
        {
            Some("jit" | "build_view" | "compile") => RuntimeAssemblyPolicy::Jit,
            _ => RuntimeAssemblyPolicy::Sealed,
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
            Some("jit" | "build_view") => RuntimeEvalPolicy::Jit,
            _ => RuntimeEvalPolicy::ArtifactFirstThin,
        };
        Self { assembly, eval }
    }

    pub(crate) fn allows_runtime_compile(self) -> bool {
        matches!(self.assembly, RuntimeAssemblyPolicy::Jit)
    }

    pub(crate) fn allows_thin_eval(self) -> bool {
        matches!(
            self.eval,
            RuntimeEvalPolicy::ArtifactFirstThin | RuntimeEvalPolicy::Jit
        )
    }

    pub(crate) fn legacy_runtime_artifact_policy(self) -> RuntimeArtifactPolicy {
        if matches!(self.eval, RuntimeEvalPolicy::Jit)
            || matches!(self.assembly, RuntimeAssemblyPolicy::Jit)
        {
            return RuntimeArtifactPolicy::BuildViewJit;
        }
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

pub(crate) fn is_build_view_request(headers: &HeaderMap) -> bool {
    if headers
        .get("x-mei-build-view")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "yes" | "on"))
    {
        return true;
    }
    headers
        .get(axum::http::header::REFERER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|referer| referer.contains("/apps/build/"))
}

pub(crate) fn load_compile_artifact_only(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Option<CompileWithCacheOutcome> {
    toolchain::load_compile_artifact_only(&state.source_root, app_id, options, components_root)
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

pub(crate) fn compile_app_with_cache_shared(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Result<toolchain::CompileWithCacheOutcomeShared, toolchain::CompileWithCacheFailure> {
    toolchain::compile_app_with_cache_shared(
        &state.source_root,
        app_id,
        options.clone(),
        components_root,
    )
}

pub(crate) fn resolve_runtime_compile_shared(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
    access_policies: RuntimeAccessPolicies,
) -> Result<Option<RuntimeCompileResolution>, toolchain::CompileWithCacheFailure> {
    let policy = access_policies.legacy_runtime_artifact_policy();
    if let Some(outcome) =
        load_compile_artifact_only_shared(state, app_id, options, components_root)
    {
        return Ok(Some(RuntimeCompileResolution {
            outcome,
            policy,
            access_policies,
            correctness_fallback: false,
            artifact_backfilled: false,
        }));
    }
    if !access_policies.allows_runtime_compile() {
        return Ok(None);
    }
    let outcome = compile_app_with_cache_shared(state, app_id, options, components_root)?;
    Ok(Some(RuntimeCompileResolution {
        artifact_backfilled: policy.is_artifact_first_fallback(),
        correctness_fallback: policy.is_artifact_first_fallback(),
        outcome,
        policy,
        access_policies,
    }))
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
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn build_view_detects_custom_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-mei-build-view", HeaderValue::from_static("1"));
        assert!(is_build_view_request(&headers));
    }

    #[test]
    fn access_defaults_to_sealed_assembly_and_thin_eval() {
        let headers = HeaderMap::new();
        let policies = RuntimeAccessPolicies::from_headers(&headers);
        assert_eq!(policies.assembly, RuntimeAssemblyPolicy::Sealed);
        assert_eq!(policies.eval, RuntimeEvalPolicy::ArtifactFirstThin);
        assert!(policies.allows_thin_eval());
        assert!(!policies.allows_runtime_compile());
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
