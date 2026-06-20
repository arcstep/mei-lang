use mei_lang_kernel::CompileOptions;
use mei_lang_toolchain as toolchain;

use crate::AppState;

pub(crate) use toolchain::CompileWithCacheOutcome;

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

pub(crate) fn clear_compile_cache_for_app(state: &AppState, app_id: &str) -> usize {
    toolchain::clear_compile_cache_for_app(&state.source_root, app_id)
}

pub(crate) fn clear_compiled_app_artifacts_for_app(state: &AppState, app_id: &str) -> usize {
    toolchain::clear_compiled_app_artifacts_for_app(&state.source_root, app_id)
}
