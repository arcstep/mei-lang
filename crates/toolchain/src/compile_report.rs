use std::path::Path;

use anyhow::Result;
use mei_lang_kernel::{
    compile_revision_plan_from_root_with_options, CompileOptions, CompileWatchedFile, CompiledApp,
};

pub struct CompileReport {
    pub compiled: CompiledApp,
    pub revision_token: String,
    pub components_revision: u128,
    pub watched_files: Vec<CompileWatchedFile>,
    pub cache_hit: bool,
    pub cache_lookup_ms: u64,
    pub compile_cache_lock_wait_ms: u64,
    pub compile_ms: u64,
}

pub fn compile_report(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
) -> Result<CompileReport> {
    let app_root = mei_lang_kernel::resolve_app_root(source_root, app_id);
    let components_root = crate::resolve_components_root(source_root);
    let outcome = crate::compile_app_with_cache(
        source_root,
        app_id,
        options.clone(),
        components_root.as_path(),
    )
    .map_err(|failure| failure.error)?;
    let revision_plan =
        compile_revision_plan_from_root_with_options(source_root, &app_root, &options)?;
    Ok(CompileReport {
        compiled: outcome.compiled,
        revision_token: outcome.compile_revision,
        components_revision: revision_plan.components_revision,
        watched_files: revision_plan.watched_files,
        cache_hit: outcome.cache_hit,
        cache_lookup_ms: outcome.cache_lookup_ms,
        compile_cache_lock_wait_ms: outcome.compile_cache_lock_wait_ms,
        compile_ms: outcome.compile_ms,
    })
}
