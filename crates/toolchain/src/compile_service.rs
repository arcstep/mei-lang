use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mei_lang_kernel::{
    compile_app_with_options, compile_revision_plan_from_root_with_options, resolve_app_root,
    resolve_components_root as kernel_resolve_components_root,
    resolve_templates_root as kernel_resolve_templates_root, CompileOptions, CompileWatchedFile,
    CompiledApp, COMPILE_SEMANTICS_GENERATION,
};
use serde::Serialize;
use walkdir::WalkDir;

#[derive(Clone)]
struct CachedCompiledApp {
    coarse_revision: u128,
    compile_revision: String,
    watched_files: Vec<CompileWatchedFile>,
    components_revision: u128,
    compiled: CompiledApp,
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

struct CompileInflight {
    result: StdMutex<Option<Result<CompiledApp, String>>>,
    ready: Condvar,
}

#[derive(Debug, Clone)]
struct CompileRevisionStamp {
    token: String,
    scope: &'static str,
    watched_files: Vec<CompileWatchedFile>,
    components_revision: u128,
}

fn compile_cache() -> &'static StdMutex<HashMap<String, CachedCompiledApp>> {
    static COMPILE_CACHE: OnceLock<StdMutex<HashMap<String, CachedCompiledApp>>> = OnceLock::new();
    COMPILE_CACHE.get_or_init(|| StdMutex::new(HashMap::new()))
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

pub fn recent_compile_failure(source_root: &Path, app_id: &str, options: &CompileOptions) -> bool {
    let cache_key = compile_cache_key(source_root, app_id, options);
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

pub fn env_flag_enabled(name: &str) -> bool {
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

pub fn compile_app_with_cache(
    source_root: &Path,
    app_id: &str,
    options: CompileOptions,
    components_root: &Path,
) -> Result<CompileWithCacheOutcome, CompileWithCacheFailure> {
    let cache_key = compile_cache_key(source_root, app_id, &options);
    if !compile_singleflight_enabled() {
        let outcome = compile_app_with_cache_uncached_path(
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
        return compile_app_with_cache_uncached_path(
            source_root,
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
                compile_revision: compile_revision(source_root, app_id, &options, components_root).token,
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
    let outcome = compile_app_with_cache_uncached_path(
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

fn compile_app_with_cache_uncached_path(
    source_root: &Path,
    app_id: &str,
    cache_key: &str,
    options: CompileOptions,
    components_root: &Path,
) -> Result<CompileWithCacheOutcome, CompileWithCacheFailure> {
    let lookup_lock_started = Instant::now();
    let cache_lookup_ms;
    let mut compile_cache_lock_wait_ms = 0u64;
    if let Ok(cache) = compile_cache().lock() {
        compile_cache_lock_wait_ms += elapsed_ms(lookup_lock_started);
        let lookup_started = Instant::now();
        if let Some(entry) = cache.get(cache_key) {
            if watched_files_are_fresh(source_root, app_id, entry, components_root) {
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
            let coarse_revision = coarse_compile_revision(source_root, app_id, components_root);
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
            let revision_stamp = compile_revision(source_root, app_id, &options, components_root);
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
    let revision_stamp = compile_revision(source_root, app_id, &options, components_root);
    let coarse_revision = coarse_compile_revision(source_root, app_id, components_root);
    let alias_options = options.clone();
    let compile_started = Instant::now();
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
    let compile_ms = elapsed_ms(compile_started);
    let write_lock_started = Instant::now();
    if let Ok(mut cache) = compile_cache().lock() {
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

pub fn peek_compile_cache(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<CompiledApp> {
    let cache_key = compile_cache_key(source_root, app_id, options);
    let cache = compile_cache().lock().ok()?;
    let entry = cache.get(&cache_key)?;
    if watched_files_are_fresh(source_root, app_id, entry, components_root) {
        Some(entry.compiled.clone())
    } else if entry.coarse_revision == coarse_compile_revision(source_root, app_id, components_root) {
        Some(entry.compiled.clone())
    } else {
        let revision_stamp = compile_revision(source_root, app_id, options, components_root);
        if entry.compile_revision == revision_stamp.token {
            Some(entry.compiled.clone())
        } else {
            None
        }
    }
}

pub fn peek_compile_cache_hit(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<PeekCompileCacheHit> {
    let cache_key = compile_cache_key(source_root, app_id, options);
    let cache = compile_cache().lock().ok()?;
    let entry = cache.get(&cache_key)?;
    if watched_files_are_fresh(source_root, app_id, entry, components_root) {
        Some(PeekCompileCacheHit {
            compiled: entry.compiled.clone(),
            compile_revision: entry.compile_revision.clone(),
            revision_scope: "watch_set".to_string(),
            cache_validation: "watch_set".to_string(),
        })
    } else if entry.coarse_revision == coarse_compile_revision(source_root, app_id, components_root) {
        Some(PeekCompileCacheHit {
            compiled: entry.compiled.clone(),
            compile_revision: entry.compile_revision.clone(),
            revision_scope: "coarse_fast_path".to_string(),
            cache_validation: "coarse_fast_path".to_string(),
        })
    } else {
        let revision_stamp = compile_revision(source_root, app_id, options, components_root);
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

pub fn is_compile_inflight(source_root: &Path, app_id: &str, options: &CompileOptions) -> bool {
    let cache_key = compile_cache_key(source_root, app_id, options);
    compile_inflight_map()
        .lock()
        .ok()
        .is_some_and(|guard| guard.contains_key(&cache_key))
}

pub fn clear_compile_cache_for_app(source_root: &Path, app_id: &str) -> usize {
    let Ok(mut cache) = compile_cache().lock() else {
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

#[derive(Debug, Clone, Serialize)]
pub struct LayoutCheck {
    pub id: String,
    pub level: String,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceLayoutRoots {
    pub source_root: String,
    pub app_root: String,
    pub components_root: String,
    pub components_resolution: String,
    pub templates_root: String,
    pub vendor_root: String,
    pub upload_root: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceLayoutInspection {
    pub app_id: String,
    pub roots: SourceLayoutRoots,
    pub checks: Vec<LayoutCheck>,
    pub ok: bool,
}

pub fn inspect_source_layout(source_root: &Path, app_id: &str) -> SourceLayoutInspection {
    let app_id = app_id.trim();
    let app_root = resolve_app_root(source_root, app_id);
    let components_root = resolve_components_root(source_root);
    let templates_root = kernel_resolve_templates_root(source_root);
    let vendor_root = components_root.join("vendor");
    let upload_root = app_root.join("upload");
    let components_resolution = components_root
        .strip_prefix(source_root)
        .map(|rel| format!("source_root/{}", rel.to_string_lossy().replace('\\', "/")))
        .unwrap_or_else(|_| components_root.display().to_string());

    let mut checks: Vec<LayoutCheck> = Vec::new();
    push_layout_check(
        &mut checks,
        "app_root_exists",
        app_root.is_dir(),
        "error",
        format!("app root `{}` does not exist", app_root.display()),
        Some(format!(
            "create `{}` and place `main.mei` under it, or update --app/--source-root",
            app_root.display()
        )),
    );
    push_layout_check(
        &mut checks,
        "app_main_exists",
        app_root.join("main.mei").is_file(),
        "error",
        format!("`{}` is missing", app_root.join("main.mei").display()),
        Some("ensure entry.main resolves to main.mei or provide a valid app root".to_string()),
    );
    push_layout_check(
        &mut checks,
        "components_root_exists",
        components_root.is_dir(),
        "error",
        format!("components root `{}` does not exist", components_root.display()),
        Some(
            "run `mei workspace materialize` or set paths.components in `.mei-workspace.json`"
                .to_string(),
        ),
    );
    push_layout_check(
        &mut checks,
        "vendor_root_exists",
        vendor_root.is_dir(),
        "warning",
        format!("vendor root `{}` does not exist", vendor_root.display()),
        Some(
            "run `npm run assets:build` in mei-lang to refresh shared vendor bundles when chart/map components are used"
                .to_string(),
        ),
    );
    push_layout_check(
        &mut checks,
        "templates_root_exists",
        templates_root.is_dir(),
        "warning",
        format!("templates root `{}` does not exist", templates_root.display()),
        Some(
            "run `mei workspace materialize` or set paths.templates; scenes should reference `../.stock/templates/...`"
                .to_string(),
        ),
    );
    push_layout_check(
        &mut checks,
        "upload_root_exists",
        upload_root.is_dir(),
        "info",
        format!("upload root `{}` does not exist", upload_root.display()),
        Some("optional: create when the app uses upload-backed data sources".to_string()),
    );

    let ok = !checks.iter().any(|item| item.level == "error");
    SourceLayoutInspection {
        app_id: app_id.to_string(),
        roots: SourceLayoutRoots {
            source_root: source_root.display().to_string(),
            app_root: app_root.display().to_string(),
            components_root: components_root.display().to_string(),
            components_resolution,
            templates_root: templates_root.display().to_string(),
            vendor_root: vendor_root.display().to_string(),
            upload_root: upload_root.display().to_string(),
        },
        checks,
        ok,
    }
}

fn push_layout_check(
    checks: &mut Vec<LayoutCheck>,
    id: &str,
    passed: bool,
    level: &str,
    message: String,
    hint: Option<String>,
) {
    if passed {
        return;
    }
    checks.push(LayoutCheck {
        id: id.to_string(),
        level: level.to_string(),
        message,
        hint,
    });
}

fn compile_cache_key(source_root: &Path, app_id: &str, options: &CompileOptions) -> String {
    format!(
        "{}#{app_id}|v5|gen={COMPILE_SEMANTICS_GENERATION}|scene={}|focus={}",
        normalize_path(source_root),
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

fn default_scene_alias_keys(
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

fn coarse_compile_revision(source_root: &Path, app_id: &str, components_root: &Path) -> u128 {
    let app_root = resolve_app_root(source_root, app_id);
    if compile_revision_mode() == RevisionMode::Full {
        let app_mtime = directory_latest_full_modified_ms(&app_root).unwrap_or(0);
        let components_mtime = directory_latest_full_modified_ms(components_root).unwrap_or(0);
        return app_mtime.max(components_mtime);
    }
    let app_mtime = directory_latest_modified_ms(&app_root, RevisionScope::App).unwrap_or(0);
    let components_mtime =
        directory_latest_modified_ms(components_root, RevisionScope::Components).unwrap_or(0);
    app_mtime.max(components_mtime)
}

fn components_revision(components_root: &Path) -> u128 {
    if compile_revision_mode() == RevisionMode::Full {
        return directory_latest_full_modified_ms(components_root).unwrap_or(0);
    }
    directory_latest_modified_ms(components_root, RevisionScope::Components).unwrap_or(0)
}

fn compile_revision(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> CompileRevisionStamp {
    let app_root = resolve_app_root(source_root, app_id);
    if let Ok(plan) = compile_revision_plan_from_root_with_options(source_root, &app_root, options) {
        return CompileRevisionStamp {
            token: plan.token,
            scope: "focused_graph",
            watched_files: plan.watched_files,
            components_revision: plan.components_revision,
        };
    }
    compile_revision_fallback(&app_root, components_root)
}

fn compile_revision_fallback(app_root: &Path, components_root: &Path) -> CompileRevisionStamp {
    if compile_revision_mode() == RevisionMode::Full {
        let app_mtime = directory_latest_full_modified_ms(app_root).unwrap_or(0);
        let components_mtime = directory_latest_full_modified_ms(components_root).unwrap_or(0);
        return CompileRevisionStamp {
            token: app_mtime.max(components_mtime).to_string(),
            scope: "full_mtime",
            watched_files: Vec::new(),
            components_revision: components_mtime,
        };
    }
    let app_mtime = directory_latest_modified_ms(app_root, RevisionScope::App).unwrap_or(0);
    let components_mtime =
        directory_latest_modified_ms(components_root, RevisionScope::Components).unwrap_or(0);
    CompileRevisionStamp {
        token: app_mtime.max(components_mtime).to_string(),
        scope: "relevant_mtime",
        watched_files: Vec::new(),
        components_revision: components_mtime,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RevisionMode {
    Relevant,
    Full,
}

#[derive(Clone, Copy)]
enum RevisionScope {
    App,
    Components,
}

fn compile_revision_mode() -> RevisionMode {
    let raw = env::var("MEI_COMPILE_REVISION_MODE").unwrap_or_default();
    if raw.trim().eq_ignore_ascii_case("full") {
        RevisionMode::Full
    } else {
        RevisionMode::Relevant
    }
}

fn directory_latest_full_modified_ms(path: &Path) -> Option<u128> {
    if !path.exists() {
        return None;
    }
    let mut latest = std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(unix_timestamp_ms);
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !should_skip_dir(entry.path()))
        .flatten()
    {
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(unix_timestamp_ms);
        if modified > latest {
            latest = modified;
        }
    }
    latest
}

fn directory_latest_modified_ms(path: &Path, scope: RevisionScope) -> Option<u128> {
    if !path.exists() {
        return None;
    }
    let mut latest = std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(unix_timestamp_ms);
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !should_skip_dir(entry.path()))
        .flatten()
    {
        if !entry.file_type().is_file() || !is_revision_relevant(entry.path(), scope) {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(unix_timestamp_ms);
        if modified > latest {
            latest = modified;
        }
    }
    latest
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | "node_modules" | "target" | ".mei" | "__pycache__" | "dist" | "build"
            )
        })
}

fn is_revision_relevant(path: &Path, scope: RevisionScope) -> bool {
    match scope {
        RevisionScope::App => path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mei")),
        RevisionScope::Components => is_component_manifest(path),
    }
}

fn is_component_manifest(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    if !name.eq_ignore_ascii_case("component.manifest.json") {
        return false;
    }
    let normalized = normalize_path(path);
    normalized.contains("/_components/")
}

fn normalize_path(path: &Path) -> String {
    PathBuf::from(path).to_string_lossy().replace('\\', "/")
}

fn unix_timestamp_ms(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis())
}
