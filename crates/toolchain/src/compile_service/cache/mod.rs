mod revision;
mod singleflight;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, OnceLock, RwLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

use mei_lang_kernel::{
    compile_app_with_options, compile_app_with_options_and_revision, resolve_app_root,
    CompileOptions, CompileWatchedFile, CompiledApp, COMPILE_SEMANTICS_GENERATION,
};

use mei_lang_kernel::resolve_components_root as kernel_resolve_components_root;

pub use singleflight::env_flag_enabled;

use revision::{coarse_compile_revision, compile_revision, components_revision, normalize_path};
use singleflight::{
    compile_singleflight_enabled, finish_compile_inflight, register_compile_inflight,
    wait_for_compile_inflight,
};

#[derive(Clone)]
pub(super) struct CachedCompiledApp {
    coarse_revision: u128,
    compile_revision: String,
    watched_files: Vec<CompileWatchedFile>,
    components_revision: u128,
    compiled: Arc<CompiledApp>,
}

pub struct CompileWithCacheOutcome {
    pub compiled: CompiledApp,
    pub cache_hit: bool,
    pub compile_revision: String,
    pub revision_scope: String,
    pub cache_validation: String,
    pub cache_lookup_ms: u64,
    pub compile_cache_lock_wait_ms: u64,
    pub compile_ms: u64,
}

pub struct CompileWithCacheOutcomeShared {
    pub compiled: Arc<CompiledApp>,
    pub cache_hit: bool,
    pub compile_revision: String,
    pub revision_scope: String,
    pub cache_validation: String,
    pub cache_lookup_ms: u64,
    pub compile_cache_lock_wait_ms: u64,
    pub compile_ms: u64,
}

pub struct CompileWithCacheFailure {
    pub error: anyhow::Error,
    pub revision_scope: String,
    pub cache_validation: String,
    pub cache_lookup_ms: u64,
    pub compile_cache_lock_wait_ms: u64,
    pub compile_ms: u64,
}

pub struct PeekCompileCacheHit {
    pub compiled: CompiledApp,
    pub compile_revision: String,
    pub revision_scope: String,
    pub cache_validation: String,
}

pub struct PeekCompileCacheHitShared {
    pub compiled: Arc<CompiledApp>,
    pub compile_revision: String,
    pub revision_scope: String,
    pub cache_validation: String,
}

impl CompileWithCacheOutcomeShared {
    fn into_owned(self) -> CompileWithCacheOutcome {
        CompileWithCacheOutcome {
            compiled: (*self.compiled).clone(),
            cache_hit: self.cache_hit,
            compile_revision: self.compile_revision,
            revision_scope: self.revision_scope,
            cache_validation: self.cache_validation,
            cache_lookup_ms: self.cache_lookup_ms,
            compile_cache_lock_wait_ms: self.compile_cache_lock_wait_ms,
            compile_ms: self.compile_ms,
        }
    }
}

impl PeekCompileCacheHitShared {
    fn into_owned(self) -> PeekCompileCacheHit {
        PeekCompileCacheHit {
            compiled: (*self.compiled).clone(),
            compile_revision: self.compile_revision,
            revision_scope: self.revision_scope,
            cache_validation: self.cache_validation,
        }
    }
}

pub(super) fn compile_cache() -> &'static RwLock<HashMap<String, CachedCompiledApp>> {
    static COMPILE_CACHE: OnceLock<RwLock<HashMap<String, CachedCompiledApp>>> = OnceLock::new();
    COMPILE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(super) fn compile_failure_latch() -> &'static StdMutex<HashMap<String, Instant>> {
    static COMPILE_FAILURE_LATCH: OnceLock<StdMutex<HashMap<String, Instant>>> = OnceLock::new();
    COMPILE_FAILURE_LATCH.get_or_init(|| StdMutex::new(HashMap::new()))
}

const COMPILE_FAILURE_LATCH_TTL: Duration = Duration::from_secs(45);

pub(super) fn record_compile_failure(cache_key: &str) {
    if let Ok(mut guard) = compile_failure_latch().lock() {
        guard.insert(cache_key.to_string(), Instant::now());
    }
}

pub(super) fn clear_compile_failure(cache_key: &str) {
    if let Ok(mut guard) = compile_failure_latch().lock() {
        guard.remove(cache_key);
    }
}

pub fn recent_compile_failure(source_root: &Path, app_id: &str, options: &CompileOptions) -> bool {
    let cache_key = compile_cache_key(source_root, app_id, options);
    let Ok(guard) = compile_failure_latch().lock() else {
        return false;
    };
    guard
        .get(&cache_key)
        .is_some_and(|at| at.elapsed() < COMPILE_FAILURE_LATCH_TTL)
}

pub fn compile_app_with_cache(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
    components_root: &Path,
) -> Result<CompileWithCacheOutcome, CompileWithCacheFailure> {
    compile_app_with_cache_shared(source_root, app_id, options, components_root)
        .map(CompileWithCacheOutcomeShared::into_owned)
}

pub fn compile_app_with_cache_shared(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
    components_root: &Path,
) -> Result<CompileWithCacheOutcomeShared, CompileWithCacheFailure> {
    let cache_key = compile_cache_key(source_root, app_id, &options);
    if !compile_singleflight_enabled() {
        let outcome = compile_app_with_cache_uncached_path_shared(
            source_root,
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
        return compile_app_with_cache_uncached_path_shared(
            source_root,
            app_id,
            &cache_key,
            options,
            components_root,
        );
    };
    if !is_leader {
        return match wait_for_compile_inflight(&inflight) {
            Ok(compiled) => Ok(CompileWithCacheOutcomeShared {
                compiled,
                cache_hit: true,
                compile_revision: compile_revision(source_root, app_id, &options, components_root)
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
    let outcome = compile_app_with_cache_uncached_path_shared(
        source_root,
        app_id,
        &cache_key,
        options,
        components_root,
    );
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

pub(super) fn compile_app_with_cache_uncached_path_shared(
    source_root: &Path,
    app_id: &str,
    cache_key: &str,
    options: CompileOptions,
    components_root: &Path,
) -> Result<CompileWithCacheOutcomeShared, CompileWithCacheFailure> {
    let lookup_lock_started = Instant::now();
    let cache_lookup_ms;
    let mut compile_cache_lock_wait_ms = 0u64;
    let mut had_cache_entry = false;
    if let Ok(cache) = compile_cache().read() {
        compile_cache_lock_wait_ms += elapsed_ms(lookup_lock_started);
        let lookup_started = Instant::now();
        if let Some(entry) = cache.get(cache_key) {
            had_cache_entry = true;
            if let Some(hit) =
                validate_cached_entry(source_root, app_id, entry, components_root, &options)
            {
                cache_lookup_ms = elapsed_ms(lookup_started);
                return Ok(CompileWithCacheOutcomeShared {
                    compiled: entry.compiled.clone(),
                    cache_hit: true,
                    compile_revision: hit.compile_revision,
                    revision_scope: hit.revision_scope,
                    cache_validation: hit.cache_validation,
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
    let alias_options = options.clone();
    let compile_started = Instant::now();
    let (compiled, revision_stamp, coarse_revision) = if had_cache_entry {
        let revision_stamp = compile_revision(source_root, app_id, &options, components_root);
        let coarse_revision = coarse_compile_revision(source_root, app_id, components_root);
        let compiled = match compile_app_with_options(source_root, app_id, options) {
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
        (compiled, revision_stamp, coarse_revision)
    } else {
        match compile_app_with_options_and_revision(source_root, app_id, options) {
            Ok(artifacts) => {
                let coarse_revision = if artifacts.revision_plan.watched_files.is_empty() {
                    coarse_compile_revision(source_root, app_id, components_root)
                } else {
                    0
                };
                (
                    artifacts.compiled,
                    revision::CompileRevisionStamp {
                        token: artifacts.revision_plan.token,
                        scope: "focused_graph",
                        watched_files: artifacts.revision_plan.watched_files,
                        components_revision: artifacts.revision_plan.components_revision,
                    },
                    coarse_revision,
                )
            }
            Err(error) => {
                return Err(CompileWithCacheFailure {
                    error,
                    revision_scope: "miss".to_string(),
                    cache_validation: "miss".to_string(),
                    cache_lookup_ms,
                    compile_cache_lock_wait_ms,
                    compile_ms: elapsed_ms(compile_started),
                });
            }
        }
    };
    let compile_ms = elapsed_ms(compile_started);
    let compiled = Arc::new(compiled);
    let write_lock_started = Instant::now();
    if let Ok(mut cache) = compile_cache().write() {
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
        for alias_key in default_scene_alias_keys(source_root, app_id, &alias_options, &compiled) {
            cache.insert(alias_key, cache_entry.clone());
        }
    } else {
        tracing::warn!(
            app_id = %app_id,
            "compile cache lock poisoned during write; skip cache store"
        );
        compile_cache_lock_wait_ms += elapsed_ms(write_lock_started);
    }
    Ok(CompileWithCacheOutcomeShared {
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

fn validate_cached_entry(
    source_root: &Path,
    app_id: &str,
    entry: &CachedCompiledApp,
    components_root: &Path,
    options: &CompileOptions,
) -> Option<PeekCompileCacheHitShared> {
    if watched_files_are_fresh(source_root, app_id, entry, components_root) {
        return Some(PeekCompileCacheHitShared {
            compiled: entry.compiled.clone(),
            compile_revision: entry.compile_revision.clone(),
            revision_scope: "watch_set".to_string(),
            cache_validation: "watch_set".to_string(),
        });
    }
    if entry.coarse_revision == coarse_compile_revision(source_root, app_id, components_root) {
        return Some(PeekCompileCacheHitShared {
            compiled: entry.compiled.clone(),
            compile_revision: entry.compile_revision.clone(),
            revision_scope: "coarse_fast_path".to_string(),
            cache_validation: "coarse_fast_path".to_string(),
        });
    }
    let revision_stamp = compile_revision(source_root, app_id, options, components_root);
    (entry.compile_revision == revision_stamp.token).then(|| PeekCompileCacheHitShared {
        compiled: entry.compiled.clone(),
        compile_revision: revision_stamp.token,
        revision_scope: revision_stamp.scope.to_string(),
        cache_validation: "focused_token".to_string(),
    })
}

pub(super) fn compile_cache_key(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
) -> String {
    format!(
        "{}#{app_id}|v5|gen={COMPILE_SEMANTICS_GENERATION}|scene={}|focus={}",
        normalize_path(source_root),
        options.scene.as_deref().unwrap_or(""),
        options.preview_target.as_deref().unwrap_or("")
    )
}

pub(super) fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

pub(super) fn watched_files_are_fresh(
    source_root: &Path,
    app_id: &str,
    entry: &CachedCompiledApp,
    components_root: &Path,
) -> bool {
    if entry.watched_files.is_empty() {
        return false;
    }
    if entry.components_revision != components_revision(components_root) {
        return false;
    }
    let app_root = resolve_app_root(source_root, app_id);
    entry.watched_files.iter().all(|watched| {
        let path = app_root.join(&watched.rel_path);
        let Ok(metadata) = std::fs::metadata(&path) else {
            return false;
        };
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|dur| dur.as_millis())
            .unwrap_or(0);
        metadata.len() == watched.size_bytes && modified_ms == watched.modified_ms
    })
}

pub(super) fn default_scene_alias_keys(
    source_root: &Path,
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
    let primary_key = compile_cache_key(source_root, app_id, options);
    let default_key = compile_cache_key(
        source_root,
        app_id,
        &CompileOptions {
            scene: None,
            preview_target: None,
        },
    );
    let explicit_default_key = compile_cache_key(
        source_root,
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

pub fn peek_compile_cache(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<CompiledApp> {
    peek_compile_cache_shared(source_root, app_id, options, components_root)
        .map(|compiled| (*compiled).clone())
}

pub fn peek_compile_cache_shared(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<Arc<CompiledApp>> {
    let cache_key = compile_cache_key(source_root, app_id, options);
    let cache = compile_cache().read().ok()?;
    let entry = cache.get(&cache_key)?;
    validate_cached_entry(source_root, app_id, entry, components_root, options).map(|hit| hit.compiled)
}

pub fn peek_compile_cache_hit(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<PeekCompileCacheHit> {
    peek_compile_cache_hit_shared(source_root, app_id, options, components_root)
        .map(PeekCompileCacheHitShared::into_owned)
}

pub fn peek_compile_cache_hit_shared(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<PeekCompileCacheHitShared> {
    let cache_key = compile_cache_key(source_root, app_id, options);
    let cache = compile_cache().read().ok()?;
    let entry = cache.get(&cache_key)?;
    validate_cached_entry(source_root, app_id, entry, components_root, options)
}

pub fn is_compile_inflight(source_root: &Path, app_id: &str, options: &CompileOptions) -> bool {
    singleflight::is_compile_inflight(source_root, app_id, options)
}

pub fn clear_compile_cache_for_app(source_root: &Path, app_id: &str) -> usize {
    let Ok(mut cache) = compile_cache().write() else {
        tracing::warn!(app_id = %app_id, "compile cache lock poisoned during clear");
        return 0;
    };
    let prefix = format!("{}#{app_id}|", normalize_path(source_root));
    let before = cache.len();
    cache.retain(|key, _| !key.starts_with(prefix.as_str()));
    before.saturating_sub(cache.len())
}

pub fn resolve_components_root(source_root: &Path) -> PathBuf {
    kernel_resolve_components_root(source_root)
}
