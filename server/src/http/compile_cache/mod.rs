use std::sync::Arc;

use mei_lang_kernel::{CompileOptions, CompiledApp};
use mei_lang_toolchain as toolchain;

use crate::AppState;

pub(crate) use toolchain::{
    CompileWithCacheFailure, CompileWithCacheOutcome, CompileWithCacheOutcomeShared,
    PeekCompileCacheHit,
};

pub(crate) fn compile_app_with_cache(
    state: &AppState,
    app_id: &str,
    options: CompileOptions,
    components_root: &std::path::Path,
) -> Result<CompileWithCacheOutcome, CompileWithCacheFailure> {
    toolchain::compile_app_with_cache(&state.source_root, app_id, options, components_root)
}

pub(crate) fn compile_app_with_cache_shared(
    state: &AppState,
    app_id: &str,
    options: CompileOptions,
    components_root: &std::path::Path,
) -> Result<CompileWithCacheOutcomeShared, CompileWithCacheFailure> {
    toolchain::compile_app_with_cache_shared(&state.source_root, app_id, options, components_root)
}

pub(crate) fn load_compile_artifact_only(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Option<CompileWithCacheOutcome> {
    toolchain::load_compile_artifact_only(&state.source_root, app_id, options, components_root)
}

pub(crate) fn load_compile_artifact_only_shared(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Option<CompileWithCacheOutcomeShared> {
    toolchain::load_compile_artifact_only_shared(
        &state.source_root,
        app_id,
        options,
        components_root,
    )
}

pub(crate) fn peek_compile_cache_shared(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Option<Arc<CompiledApp>> {
    toolchain::peek_compile_cache_shared(&state.source_root, app_id, options, components_root)
}

pub(crate) fn peek_compile_cache_hit(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Option<PeekCompileCacheHit> {
    toolchain::peek_compile_cache_hit(&state.source_root, app_id, options, components_root)
}

pub(crate) fn recent_compile_failure(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
) -> bool {
    toolchain::recent_compile_failure(&state.source_root, app_id, options)
}

pub(crate) fn is_compile_inflight(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
) -> bool {
    toolchain::is_compile_inflight(&state.source_root, app_id, options)
}

pub(crate) fn start_compile_in_background_if_needed(
    state: AppState,
    app_id: String,
    options: CompileOptions,
    components_root: std::path::PathBuf,
) {
    if peek_compile_cache_shared(&state, &app_id, &options, components_root.as_path()).is_some() {
        return;
    }
    if is_compile_inflight(&state, &app_id, &options) {
        return;
    }
    let source_root = state.source_root.clone();
    tokio::task::spawn_blocking(move || {
        let _ = toolchain::compile_app_with_cache(
            &source_root,
            &app_id,
            options,
            components_root.as_path(),
        );
    });
}

pub(crate) fn clear_compile_cache_for_app(state: &AppState, app_id: &str) -> usize {
    toolchain::clear_compile_cache_for_app(&state.source_root, app_id)
}

pub(crate) fn clear_compiled_app_artifacts_for_app(state: &AppState, app_id: &str) -> usize {
    toolchain::clear_compiled_app_artifacts_for_app(&state.source_root, app_id)
}

pub(crate) fn env_flag_enabled(name: &str) -> bool {
    toolchain::env_flag_enabled(name)
}

pub(crate) fn access_artifact_only_mode_enabled() -> bool {
    env_flag_enabled("MEI_ACCESS_ARTIFACT_ONLY")
}
