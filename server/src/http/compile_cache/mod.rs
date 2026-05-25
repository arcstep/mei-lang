mod revision;

use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Condvar, Mutex as StdMutex, OnceLock};
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

struct CompileInflight {
    result: StdMutex<Option<Result<CompiledApp, String>>>,
    ready: Condvar,
}

fn compile_inflight_map() -> &'static StdMutex<HashMap<String, Arc<CompileInflight>>> {
    static COMPILE_INFLIGHT: OnceLock<StdMutex<HashMap<String, Arc<CompileInflight>>>> =
        OnceLock::new();
    COMPILE_INFLIGHT.get_or_init(|| StdMutex::new(HashMap::new()))
}

fn compile_singleflight_enabled() -> bool {
    if env_flag_enabled("MEI_DISABLE_COMPILE_SINGLEFLIGHT") {
        return false;
    }
    !env_list_contains("MEI_PERF_DISABLE", "compile_singleflight")
}

fn env_flag_enabled(name: &str) -> bool {
    env::var(name)
        .ok()
        .is_some_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
}

fn env_list_contains(name: &str, needle: &str) -> bool {
    env::var(name)
        .ok()
        .is_some_and(|value| {
            value
                .split(',')
                .map(|item| item.trim().to_ascii_lowercase())
                .any(|item| item == needle)
        })
}

fn register_compile_inflight(cache_key: &str) -> Option<(Arc<CompileInflight>, bool)> {
    let map = compile_inflight_map();
    let mut guard = map.lock().ok()?;
    if let Some(entry) = guard.get(cache_key) {
        return Some((entry.clone(), false));
    }
    let entry = Arc::new(CompileInflight {
        result: StdMutex::new(None),
        ready: Condvar::new(),
    });
    guard.insert(cache_key.to_string(), entry.clone());
    Some((entry, true))
}

fn finish_compile_inflight(
    cache_key: &str,
    entry: &Arc<CompileInflight>,
    result: Result<CompiledApp, String>,
) {
    if let Ok(mut slot) = entry.result.lock() {
        *slot = Some(result);
        entry.ready.notify_all();
    }
    if let Ok(mut guard) = compile_inflight_map().lock() {
        guard.remove(cache_key);
    }
}

fn wait_for_compile_inflight(entry: &Arc<CompileInflight>) -> Result<CompiledApp, String> {
    let mut slot = entry
        .result
        .lock()
        .map_err(|_| "compile inflight lock poisoned".to_string())?;
    while slot.is_none() {
        slot = entry
            .ready
            .wait(slot)
            .map_err(|_| "compile inflight wait poisoned".to_string())?;
    }
    slot.clone()
        .ok_or_else(|| "compile inflight finished without result".to_string())?
}

pub(crate) fn compile_app_with_cache(
    state: &AppState,
    app_id: &str,
    options: CompileOptions,
    components_root: &std::path::Path,
) -> Result<CompileWithCacheOutcome, CompileWithCacheFailure> {
    let cache_key = compile_cache_key(app_id, &options);
    if !compile_singleflight_enabled() {
        return compile_app_with_cache_uncached_path(
            state,
            app_id,
            &cache_key,
            options,
            components_root,
        );
    }
    let singleflight_started = Instant::now();
    let Some((inflight, is_leader)) = register_compile_inflight(&cache_key) else {
        tracing::warn!(
            app_id = %app_id,
            "compile inflight map lock poisoned; fallback to direct compile path"
        );
        return compile_app_with_cache_uncached_path(
            state,
            app_id,
            &cache_key,
            options,
            components_root,
        );
    };
    if !is_leader {
        return match wait_for_compile_inflight(&inflight) {
            Ok(compiled) => Ok(CompileWithCacheOutcome {
                compiled,
                cache_hit: true,
                cache_lookup_ms: elapsed_ms(singleflight_started),
                compile_cache_lock_wait_ms: 0,
                compile_ms: 0,
            }),
            Err(message) => Err(CompileWithCacheFailure {
                error: anyhow::anyhow!(message),
                cache_lookup_ms: elapsed_ms(singleflight_started),
                compile_cache_lock_wait_ms: 0,
                compile_ms: 0,
            }),
        };
    }
    let outcome =
        compile_app_with_cache_uncached_path(state, app_id, &cache_key, options, components_root);
    match &outcome {
        Ok(value) => finish_compile_inflight(&cache_key, &inflight, Ok(value.compiled.clone())),
        Err(error) => finish_compile_inflight(
            &cache_key,
            &inflight,
            Err(error.error.to_string()),
        ),
    }
    outcome
}

fn compile_app_with_cache_uncached_path(
    state: &AppState,
    app_id: &str,
    cache_key: &str,
    options: CompileOptions,
    components_root: &std::path::Path,
) -> Result<CompileWithCacheOutcome, CompileWithCacheFailure> {
    let app_revision = revision::compile_revision(state, app_id, components_root);
    let lookup_lock_started = Instant::now();
    let cache_lookup_ms;
    let mut compile_cache_lock_wait_ms = 0u64;
    if let Ok(cache) = state.compile_cache.lock() {
        compile_cache_lock_wait_ms += elapsed_ms(lookup_lock_started);
        let lookup_started = Instant::now();
        if let Some(entry) = cache.get(cache_key) {
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
            cache_key.to_string(),
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
