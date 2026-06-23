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

impl RuntimeArtifactPolicy {
    pub(crate) fn from_headers(headers: &HeaderMap) -> Self {
        if is_build_view_request(headers) {
            return Self::BuildViewJit;
        }
        match std::env::var("MEI_RUNTIME_ARTIFACT_POLICY")
            .or_else(|_| std::env::var("MEI_ACCESS_ARTIFACT_POLICY"))
            .ok()
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("sealed" | "sealed_strict" | "strict" | "aot_strict") => Self::SealedStrict,
            _ => Self::ArtifactFirstFallback,
        }
    }

    pub(crate) fn allows_runtime_compile(self) -> bool {
        matches!(self, Self::ArtifactFirstFallback | Self::BuildViewJit)
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
    pub(crate) correctness_fallback: bool,
    pub(crate) artifact_backfilled: bool,
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
    toolchain::compile_app_with_cache(&state.source_root, app_id, options.clone(), components_root)
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
    policy: RuntimeArtifactPolicy,
) -> Result<Option<RuntimeCompileResolution>, toolchain::CompileWithCacheFailure> {
    if let Some(outcome) =
        load_compile_artifact_only_shared(state, app_id, options, components_root)
    {
        return Ok(Some(RuntimeCompileResolution {
            outcome,
            policy,
            correctness_fallback: false,
            artifact_backfilled: false,
        }));
    }
    if !policy.allows_runtime_compile() {
        return Ok(None);
    }
    let outcome = compile_app_with_cache_shared(state, app_id, options, components_root)?;
    Ok(Some(RuntimeCompileResolution {
        artifact_backfilled: policy.is_artifact_first_fallback(),
        correctness_fallback: policy.is_artifact_first_fallback(),
        outcome,
        policy,
    }))
}

pub(crate) fn clear_compile_cache_for_app(state: &AppState, app_id: &str) -> usize {
    toolchain::clear_compile_cache_for_app(&state.source_root, app_id)
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
    fn build_view_detects_referer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::REFERER,
            HeaderValue::from_static("http://localhost/apps/build/zhifa?tab=preview"),
        );
        assert!(is_build_view_request(&headers));
    }

    #[test]
    fn build_view_rejects_access_page_referer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::REFERER,
            HeaderValue::from_static("http://localhost/apps/zhifa/home"),
        );
        assert!(!is_build_view_request(&headers));
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
    fn runtime_policy_keeps_build_view_jit() {
        let mut headers = HeaderMap::new();
        headers.insert("x-mei-build-view", HeaderValue::from_static("1"));
        assert_eq!(
            RuntimeArtifactPolicy::from_headers(&headers),
            RuntimeArtifactPolicy::BuildViewJit
        );
    }

    #[test]
    fn runtime_policy_compile_fallback_is_explicitly_gated() {
        assert!(!RuntimeArtifactPolicy::SealedStrict.allows_runtime_compile());
        assert!(RuntimeArtifactPolicy::ArtifactFirstFallback.allows_runtime_compile());
        assert!(RuntimeArtifactPolicy::BuildViewJit.allows_runtime_compile());
    }
}
