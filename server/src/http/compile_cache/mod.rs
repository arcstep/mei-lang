use mei_lang_kernel::CompileOptions;
use mei_lang_toolchain as toolchain;
use axum::http::HeaderMap;

use crate::AppState;

pub(crate) use toolchain::CompileWithCacheOutcome;

pub(crate) fn is_build_view_request(headers: &HeaderMap) -> bool {
    if headers
        .get("x-mei-build-view")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            matches!(
                value.trim(),
                "1" | "true" | "yes" | "on"
            )
        })
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
    allow_build_runtime_compile: bool,
) -> Option<toolchain::CompileWithCacheOutcomeShared> {
    if let Some(outcome) =
        load_compile_artifact_only_shared(state, app_id, options, components_root)
    {
        return Some(outcome);
    }
    if !allow_build_runtime_compile {
        return None;
    }
    compile_app_with_cache_shared(state, app_id, options, components_root).ok()
}

pub(crate) fn clear_compile_cache_for_app(state: &AppState, app_id: &str) -> usize {
    toolchain::clear_compile_cache_for_app(&state.source_root, app_id)
}

pub(crate) fn clear_compiled_app_artifacts_for_app(state: &AppState, app_id: &str) -> usize {
    toolchain::clear_compiled_app_artifacts_for_app(&state.source_root, app_id)
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
}
