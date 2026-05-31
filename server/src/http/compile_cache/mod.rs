mod revision;

use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant};

use mei_lang_kernel::{compile_app_with_options, CompileOptions, CompiledApp};

use crate::{AppState, CachedCompiledApp};

pub(crate) struct CompileWithCacheOutcome {
    pub(crate) compiled: CompiledApp,
    pub(crate) cache_hit: bool,
    pub(crate) compile_revision: String,
    pub(crate) revision_scope: String,
    pub(crate) cache_validation: String,
    pub(crate) cache_lookup_ms: u64,
    /// 等待 `compile_cache` Mutex 的累计时间（lookup + 写入各一次，不含锁外编译）。
    pub(crate) compile_cache_lock_wait_ms: u64,
    pub(crate) compile_ms: u64,
}

pub(crate) struct CompileWithCacheFailure {
    pub(crate) error: anyhow::Error,
    pub(crate) revision_scope: String,
    pub(crate) cache_validation: String,
    pub(crate) cache_lookup_ms: u64,
    pub(crate) compile_cache_lock_wait_ms: u64,
    pub(crate) compile_ms: u64,
}

pub(crate) struct PeekCompileCacheHit {
    pub(crate) compiled: CompiledApp,
    pub(crate) compile_revision: String,
    pub(crate) revision_scope: String,
    pub(crate) cache_validation: String,
}

struct CompileInflight {
    result: StdMutex<Option<Result<CompiledApp, String>>>,
    ready: Condvar,
}

fn compile_failure_latch() -> &'static StdMutex<HashMap<String, Instant>> {
    static COMPILE_FAILURE_LATCH: OnceLock<StdMutex<HashMap<String, Instant>>> = OnceLock::new();
    COMPILE_FAILURE_LATCH.get_or_init(|| StdMutex::new(HashMap::new()))
}

const COMPILE_FAILURE_LATCH_TTL: Duration = Duration::from_secs(45);

fn record_compile_failure(cache_key: &str) {
    if let Ok(mut guard) = compile_failure_latch().lock() {
        guard.insert(cache_key.to_string(), Instant::now());
    }
}

fn clear_compile_failure(cache_key: &str) {
    if let Ok(mut guard) = compile_failure_latch().lock() {
        guard.remove(cache_key);
    }
}

pub(crate) fn recent_compile_failure(app_id: &str, options: &CompileOptions) -> bool {
    let cache_key = compile_cache_key(app_id, options);
    let Ok(guard) = compile_failure_latch().lock() else {
        return false;
    };
    guard
        .get(&cache_key)
        .is_some_and(|at| at.elapsed() < COMPILE_FAILURE_LATCH_TTL)
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

pub(crate) fn env_flag_enabled(name: &str) -> bool {
    env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn env_list_contains(name: &str, needle: &str) -> bool {
    env::var(name).ok().is_some_and(|value| {
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
        let outcome = compile_app_with_cache_uncached_path(
            state,
            app_id,
            &cache_key,
            options,
            components_root,
        );
        match &outcome {
            Ok(_) => clear_compile_failure(&cache_key),
            Err(_) => record_compile_failure(&cache_key),
        }
        return outcome;
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
                compile_revision: revision::compile_revision(
                    state,
                    app_id,
                    &options,
                    components_root,
                )
                .token,
                revision_scope: "singleflight_wait".to_string(),
                cache_validation: "singleflight_wait".to_string(),
                cache_lookup_ms: elapsed_ms(singleflight_started),
                compile_cache_lock_wait_ms: 0,
                compile_ms: 0,
            }),
            Err(message) => Err(CompileWithCacheFailure {
                error: anyhow::anyhow!(message),
                revision_scope: "singleflight_wait".to_string(),
                cache_validation: "singleflight_wait".to_string(),
                cache_lookup_ms: elapsed_ms(singleflight_started),
                compile_cache_lock_wait_ms: 0,
                compile_ms: 0,
            }),
        };
    }
    let outcome =
        compile_app_with_cache_uncached_path(state, app_id, &cache_key, options, components_root);
    match &outcome {
        Ok(value) => {
            clear_compile_failure(&cache_key);
            finish_compile_inflight(&cache_key, &inflight, Ok(value.compiled.clone()))
        }
        Err(error) => {
            record_compile_failure(&cache_key);
            finish_compile_inflight(&cache_key, &inflight, Err(error.error.to_string()))
        }
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
    let lookup_lock_started = Instant::now();
    let cache_lookup_ms;
    let mut compile_cache_lock_wait_ms = 0u64;
    if let Ok(cache) = state.compile_cache.lock() {
        compile_cache_lock_wait_ms += elapsed_ms(lookup_lock_started);
        let lookup_started = Instant::now();
        if let Some(entry) = cache.get(cache_key) {
            if watched_files_are_fresh(&state.source_root, app_id, entry, components_root) {
                cache_lookup_ms = elapsed_ms(lookup_started);
                return Ok(CompileWithCacheOutcome {
                    compiled: entry.compiled.clone(),
                    cache_hit: true,
                    compile_revision: entry.compile_revision.clone(),
                    revision_scope: "watch_set".to_string(),
                    cache_validation: "watch_set".to_string(),
                    cache_lookup_ms,
                    compile_cache_lock_wait_ms,
                    compile_ms: 0,
                });
            }
            let coarse_revision = revision::coarse_compile_revision(state, app_id, components_root);
            if entry.coarse_revision == coarse_revision {
                cache_lookup_ms = elapsed_ms(lookup_started);
                return Ok(CompileWithCacheOutcome {
                    compiled: entry.compiled.clone(),
                    cache_hit: true,
                    compile_revision: entry.compile_revision.clone(),
                    revision_scope: "coarse_fast_path".to_string(),
                    cache_validation: "coarse_fast_path".to_string(),
                    cache_lookup_ms,
                    compile_cache_lock_wait_ms,
                    compile_ms: 0,
                });
            }
            let revision_stamp =
                revision::compile_revision(state, app_id, &options, components_root);
            if entry.compile_revision == revision_stamp.token {
                cache_lookup_ms = elapsed_ms(lookup_started);
                return Ok(CompileWithCacheOutcome {
                    compiled: entry.compiled.clone(),
                    cache_hit: true,
                    compile_revision: revision_stamp.token.clone(),
                    revision_scope: revision_stamp.scope.to_string(),
                    cache_validation: "focused_token".to_string(),
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
    let revision_stamp = revision::compile_revision(state, app_id, &options, components_root);
    let coarse_revision = revision::coarse_compile_revision(state, app_id, components_root);
    let alias_options = options.clone();
    let compile_started = Instant::now();
    let compiled = match compile_app_with_options(&state.source_root, app_id, options) {
        Ok(compiled) => compiled,
        Err(error) => {
            return Err(CompileWithCacheFailure {
                error,
                revision_scope: revision_stamp.scope.to_string(),
                cache_validation: "miss".to_string(),
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
        let cache_entry = CachedCompiledApp {
            coarse_revision,
            compile_revision: revision_stamp.token.clone(),
            watched_files: revision_stamp.watched_files,
            components_revision: revision_stamp.components_revision,
            compiled: compiled.clone(),
        };
        cache.insert(cache_key.to_string(), cache_entry.clone());
        for alias_key in default_scene_alias_keys(app_id, &alias_options, &compiled) {
            cache.insert(alias_key, cache_entry.clone());
        }
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
        compile_revision: revision_stamp.token.clone(),
        revision_scope: revision_stamp.scope.to_string(),
        cache_validation: "miss".to_string(),
        cache_lookup_ms,
        compile_cache_lock_wait_ms,
        compile_ms,
    })
}

pub(crate) fn peek_compile_cache(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Option<CompiledApp> {
    let cache_key = compile_cache_key(app_id, options);
    let cache = state.compile_cache.lock().ok()?;
    let entry = cache.get(&cache_key)?;
    if watched_files_are_fresh(&state.source_root, app_id, entry, components_root) {
        Some(entry.compiled.clone())
    } else if entry.coarse_revision
        == revision::coarse_compile_revision(state, app_id, components_root)
    {
        Some(entry.compiled.clone())
    } else {
        let revision_stamp = revision::compile_revision(state, app_id, options, components_root);
        if entry.compile_revision == revision_stamp.token {
            Some(entry.compiled.clone())
        } else {
            None
        }
    }
}

pub(crate) fn peek_compile_cache_hit(
    state: &AppState,
    app_id: &str,
    options: &CompileOptions,
    components_root: &std::path::Path,
) -> Option<PeekCompileCacheHit> {
    let cache_key = compile_cache_key(app_id, options);
    let cache = state.compile_cache.lock().ok()?;
    let entry = cache.get(&cache_key)?;
    if watched_files_are_fresh(&state.source_root, app_id, entry, components_root) {
        Some(PeekCompileCacheHit {
            compiled: entry.compiled.clone(),
            compile_revision: entry.compile_revision.clone(),
            revision_scope: "watch_set".to_string(),
            cache_validation: "watch_set".to_string(),
        })
    } else if entry.coarse_revision
        == revision::coarse_compile_revision(state, app_id, components_root)
    {
        Some(PeekCompileCacheHit {
            compiled: entry.compiled.clone(),
            compile_revision: entry.compile_revision.clone(),
            revision_scope: "coarse_fast_path".to_string(),
            cache_validation: "coarse_fast_path".to_string(),
        })
    } else {
        let revision_stamp = revision::compile_revision(state, app_id, options, components_root);
        if entry.compile_revision == revision_stamp.token {
            Some(PeekCompileCacheHit {
                compiled: entry.compiled.clone(),
                compile_revision: revision_stamp.token.clone(),
                revision_scope: revision_stamp.scope.to_string(),
                cache_validation: "focused_token".to_string(),
            })
        } else {
            None
        }
    }
}

pub(crate) fn is_compile_inflight(app_id: &str, options: &CompileOptions) -> bool {
    let cache_key = compile_cache_key(app_id, options);
    compile_inflight_map()
        .lock()
        .ok()
        .is_some_and(|guard| guard.contains_key(&cache_key))
}

pub(crate) fn start_compile_in_background_if_needed(
    state: AppState,
    app_id: String,
    options: CompileOptions,
    components_root: std::path::PathBuf,
) {
    if peek_compile_cache(&state, &app_id, &options, components_root.as_path()).is_some() {
        return;
    }
    if is_compile_inflight(&app_id, &options) {
        return;
    }
    tokio::task::spawn_blocking(move || {
        let _ = compile_app_with_cache(&state, &app_id, options, components_root.as_path());
    });
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

fn watched_files_are_fresh(
    source_root: &Path,
    app_id: &str,
    entry: &CachedCompiledApp,
    components_root: &Path,
) -> bool {
    if entry.watched_files.is_empty() {
        return false;
    }
    if entry.components_revision != revision::components_revision(components_root) {
        return false;
    }
    let app_root = source_root.join(app_id);
    entry.watched_files.iter().all(|watched| {
        let path = app_root.join(&watched.rel_path);
        let Ok(metadata) = std::fs::metadata(&path) else {
            return false;
        };
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|dur| dur.as_millis())
            .unwrap_or(0);
        metadata.len() == watched.size_bytes && modified_ms == watched.modified_ms
    })
}

fn default_scene_alias_keys(
    app_id: &str,
    options: &CompileOptions,
    compiled: &CompiledApp,
) -> Vec<String> {
    if options.preview_target.is_some() {
        return Vec::new();
    }
    let active_scene = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty());
    let default_scene = compiled
        .scene_routes
        .iter()
        .find(|route| route.is_default)
        .map(|route| route.scene_id.trim())
        .filter(|scene| !scene.is_empty());
    let (Some(active_scene), Some(default_scene)) = (active_scene, default_scene) else {
        return Vec::new();
    };
    if active_scene != default_scene {
        return Vec::new();
    }
    let primary_key = compile_cache_key(app_id, options);
    let default_key = compile_cache_key(
        app_id,
        &CompileOptions {
            scene: None,
            preview_target: None,
        },
    );
    let explicit_default_key = compile_cache_key(
        app_id,
        &CompileOptions {
            scene: Some(default_scene.to_string()),
            preview_target: None,
        },
    );
    [default_key, explicit_default_key]
        .into_iter()
        .filter(|candidate| candidate != &primary_key)
        .collect()
}

pub(crate) fn clear_compile_cache_for_app(state: &AppState, app_id: &str) -> usize {
    let Ok(mut cache) = state.compile_cache.lock() else {
        tracing::warn!(app_id = %app_id, "compile cache lock poisoned during clear");
        return 0;
    };
    let prefix = format!("{app_id}|");
    let before = cache.len();
    cache.retain(|key, _| !key.starts_with(prefix.as_str()));
    before.saturating_sub(cache.len())
}
