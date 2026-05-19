mod revision;

use std::time::Instant;

use mei_lang_kernel::{compile_app_with_options, CompileOptions, CompiledApp};

use crate::{AppState, CachedCompiledApp};

pub(crate) struct CompileWithCacheOutcome {
    pub(crate) compiled: CompiledApp,
    pub(crate) cache_hit: bool,
    pub(crate) cache_lookup_ms: u64,
    pub(crate) compile_ms: u64,
}

pub(crate) struct CompileWithCacheFailure {
    pub(crate) error: anyhow::Error,
    pub(crate) cache_lookup_ms: u64,
    pub(crate) compile_ms: u64,
}

pub(crate) fn compile_app_with_cache(
    state: &AppState,
    app_id: &str,
    options: CompileOptions,
    components_root: &std::path::Path,
) -> Result<CompileWithCacheOutcome, CompileWithCacheFailure> {
    let lookup_started = Instant::now();
    let cache_key = compile_cache_key(app_id, &options);
    let app_revision = revision::compile_revision(state, app_id, components_root);
    if let Ok(cache) = state.compile_cache.lock() {
        if let Some(entry) = cache.get(&cache_key) {
            if entry.app_latest_modified_ms == app_revision {
                return Ok(CompileWithCacheOutcome {
                    compiled: entry.compiled.clone(),
                    cache_hit: true,
                    cache_lookup_ms: elapsed_ms(lookup_started),
                    compile_ms: 0,
                });
            }
        }
    } else {
        tracing::warn!(
            app_id = %app_id,
            "compile cache lock poisoned during lookup; fallback to direct compile"
        );
    }
    let cache_lookup_ms = elapsed_ms(lookup_started);
    let compile_started = Instant::now();
    let compiled = match compile_app_with_options(&state.source_root, app_id, options) {
        Ok(compiled) => compiled,
        Err(error) => {
            return Err(CompileWithCacheFailure {
                error,
                cache_lookup_ms,
                compile_ms: elapsed_ms(compile_started),
            });
        }
    };
    let compile_ms = elapsed_ms(compile_started);
    if let Ok(mut cache) = state.compile_cache.lock() {
        if cache.len() >= 128 {
            cache.clear();
        }
        cache.insert(
            cache_key,
            CachedCompiledApp {
                app_latest_modified_ms: app_revision,
                compiled: compiled.clone(),
            },
        );
    } else {
        tracing::warn!(
            app_id = %app_id,
            "compile cache lock poisoned during write; skip cache store"
        );
    }
    Ok(CompileWithCacheOutcome {
        compiled,
        cache_hit: false,
        cache_lookup_ms,
        compile_ms,
    })
}

fn compile_cache_key(app_id: &str, options: &CompileOptions) -> String {
    format!(
        "{app_id}|v3|scene={}|preview={}",
        options.scene.as_deref().unwrap_or(""),
        options.preview_target.as_deref().unwrap_or("")
    )
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}
