mod revision;

use std::time::Instant;

use mei_lang_kernel::{compile_app_with_options, CompileOptions, CompiledApp};

use crate::{AppState, CachedCompiledApp};

pub(crate) struct CompileWithCacheOutcome {
    pub(crate) compiled: CompiledApp,
    pub(crate) cache_hit: bool,
    pub(crate) cache_lookup_ms: u64,
    /// 等待 `compile_cache` Mutex 的累计时间（lookup + 写入各一次，不含锁外编译）。
    pub(crate) compile_cache_lock_wait_ms: u64,
    pub(crate) compile_ms: u64,
}

pub(crate) struct CompileWithCacheFailure {
    pub(crate) error: anyhow::Error,
    pub(crate) cache_lookup_ms: u64,
    pub(crate) compile_cache_lock_wait_ms: u64,
    pub(crate) compile_ms: u64,
}

pub(crate) fn compile_app_with_cache(
    state: &AppState,
    app_id: &str,
    options: CompileOptions,
    components_root: &std::path::Path,
) -> Result<CompileWithCacheOutcome, CompileWithCacheFailure> {
    let cache_key = compile_cache_key(app_id, &options);
    let app_revision = revision::compile_revision(state, app_id, components_root);
    let lookup_lock_started = Instant::now();
    let cache_lookup_ms;
    let mut compile_cache_lock_wait_ms = 0u64;
    if let Ok(cache) = state.compile_cache.lock() {
        compile_cache_lock_wait_ms += elapsed_ms(lookup_lock_started);
        let lookup_started = Instant::now();
        if let Some(entry) = cache.get(&cache_key) {
            if entry.app_latest_modified_ms == app_revision {
                cache_lookup_ms = elapsed_ms(lookup_started);
                return Ok(CompileWithCacheOutcome {
                    compiled: entry.compiled.clone(),
                    cache_hit: true,
                    cache_lookup_ms,
                    compile_cache_lock_wait_ms,
                    compile_ms: 0,
                });
            }
        }
        cache_lookup_ms = elapsed_ms(lookup_started);
    } else {
        tracing::warn!(
            app_id = %app_id,
            "compile cache lock poisoned during lookup; fallback to direct compile"
        );
        cache_lookup_ms = elapsed_ms(lookup_lock_started);
    }
    let compile_started = Instant::now();
    let compiled = match compile_app_with_options(&state.source_root, app_id, options) {
        Ok(compiled) => compiled,
        Err(error) => {
            return Err(CompileWithCacheFailure {
                error,
                cache_lookup_ms,
                compile_cache_lock_wait_ms,
                compile_ms: elapsed_ms(compile_started),
            });
        }
    };
    let compile_ms = elapsed_ms(compile_started);
    let write_lock_started = Instant::now();
    if let Ok(mut cache) = state.compile_cache.lock() {
        compile_cache_lock_wait_ms += elapsed_ms(write_lock_started);
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
        compile_cache_lock_wait_ms += elapsed_ms(write_lock_started);
    }
    Ok(CompileWithCacheOutcome {
        compiled,
        cache_hit: false,
        cache_lookup_ms,
        compile_cache_lock_wait_ms,
        compile_ms,
    })
}

fn compile_cache_key(app_id: &str, options: &CompileOptions) -> String {
    format!(
        "{app_id}|v4|scene={}|focus={}",
        options.scene.as_deref().unwrap_or(""),
        options.preview_target.as_deref().unwrap_or("")
    )
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}
