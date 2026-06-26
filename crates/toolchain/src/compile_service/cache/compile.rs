use super::prelude::*;
use super::*;

pub(crate) fn record_compile_failure(cache_key: &str) {
    if let Ok(mut guard) = compile_failure_latch().lock() {
        guard.insert(cache_key.to_string(), Instant::now());
    }
}

pub(crate) fn clear_compile_failure(cache_key: &str) {
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

pub fn load_compile_artifact_only(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<CompileWithCacheOutcome> {
    load_compile_artifact_only_shared(source_root, app_id, options, components_root)
        .map(CompileWithCacheOutcomeShared::into_owned)
}

pub fn apply_compile_options_scope(
    compiled: Arc<CompiledApp>,
    options: &CompileOptions,
) -> Arc<CompiledApp> {
    if !canonical_artifact_persist_enabled() {
        return compiled;
    }
    let requested_scene = options
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|scene| !scene.is_empty());
    let requested_target = options
        .preview_target
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty());
    if requested_scene.is_none() && requested_target.is_none() {
        return compiled;
    }
    let mut view = (*compiled).clone();
    if let Some(scene) = requested_scene {
        view.active_scene = Some(scene.to_string());
    }
    if let Some(target) = requested_target {
        view.active_target_file = target.to_string();
    }
    Arc::new(view)
}

pub fn load_compile_artifact_only_shared(
    source_root: &Path,
    app_id: &str,
    options: &CompileOptions,
    components_root: &Path,
) -> Option<CompileWithCacheOutcomeShared> {
    let cache_lookup_started = Instant::now();
    if let Some(hit) = peek_compile_cache_hit_shared(source_root, app_id, options, components_root)
    {
        return Some(CompileWithCacheOutcomeShared {
            compiled: apply_compile_options_scope(hit.compiled, options),
            cache_hit: true,
            artifact_cache_hit: false,
            compile_revision: hit.compile_revision,
            revision_scope: hit.revision_scope,
            cache_validation: hit.cache_validation,
            cache_lookup_ms: elapsed_ms(cache_lookup_started),
            artifact_load_ms: 0,
            compile_cache_lock_wait_ms: 0,
            compile_ms: 0,
        });
    }
    let cache_lookup_ms = elapsed_ms(cache_lookup_started);
    let (artifact_hit, artifact_load_ms) =
        maybe_load_compiled_app_artifact(source_root, app_id, options, components_root)?;
    Some(CompileWithCacheOutcomeShared {
        compiled: apply_compile_options_scope(artifact_hit.compiled, options),
        cache_hit: true,
        artifact_cache_hit: true,
        compile_revision: artifact_hit.compile_revision,
        revision_scope: artifact_hit.revision_scope,
        cache_validation: artifact_hit.cache_validation,
        cache_lookup_ms,
        artifact_load_ms,
        compile_cache_lock_wait_ms: 0,
        compile_ms: 0,
    })
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
                artifact_cache_hit: false,
                compile_revision: compile_revision(source_root, app_id, &options, components_root)
                    .token,
                revision_scope: "singleflight_wait".to_string(),
                cache_validation: "singleflight_wait".to_string(),
                cache_lookup_ms: elapsed_ms(singleflight_started),
                artifact_load_ms: 0,
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

pub(crate) fn compile_app_with_cache_uncached_path_shared(
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
                ensure_compiled_app_artifact_alias(
                    source_root,
                    app_id,
                    &options,
                    components_root,
                    entry.compiled.as_ref(),
                );
                cache_lookup_ms = elapsed_ms(lookup_started);
                return Ok(CompileWithCacheOutcomeShared {
                    compiled: entry.compiled.clone(),
                    cache_hit: true,
                    artifact_cache_hit: false,
                    compile_revision: hit.compile_revision,
                    revision_scope: hit.revision_scope,
                    cache_validation: hit.cache_validation,
                    cache_lookup_ms,
                    artifact_load_ms: 0,
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
    if let Some((artifact_hit, artifact_load_ms)) =
        maybe_load_compiled_app_artifact(source_root, app_id, &options, components_root)
    {
        ensure_compiled_app_artifact_alias(
            source_root,
            app_id,
            &options,
            components_root,
            artifact_hit.compiled.as_ref(),
        );
        return Ok(CompileWithCacheOutcomeShared {
            compiled: artifact_hit.compiled,
            cache_hit: true,
            artifact_cache_hit: true,
            compile_revision: artifact_hit.compile_revision,
            revision_scope: artifact_hit.revision_scope,
            cache_validation: artifact_hit.cache_validation,
            cache_lookup_ms,
            artifact_load_ms,
            compile_cache_lock_wait_ms,
            compile_ms: 0,
        });
    }
    let alias_options = options.clone();
    let compile_started = Instant::now();
    let (compiled, revision_stamp) = if had_cache_entry {
        let revision_stamp = compile_revision(source_root, app_id, &options, components_root);
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
        (compiled, revision_stamp)
    } else {
        match compile_app_with_options_and_revision(source_root, app_id, options) {
            Ok(artifacts) => (
                artifacts.compiled,
                revision::CompileRevisionStamp {
                    token: artifacts.revision_plan.token,
                    scope: "focused_graph",
                    watched_files: artifacts.revision_plan.watched_files,
                    components_revision: artifacts.revision_plan.components_revision,
                },
            ),
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
    let cache_compiled = if access_slim_artifacts_enabled() {
        Arc::new(slim_compiled_app_for_access(compiled.as_ref()))
    } else {
        compiled.clone()
    };
    store_compile_cache_entry(
        cache_key,
        source_root,
        app_id,
        &alias_options,
        &revision_stamp.token,
        &revision_stamp.watched_files,
        revision_stamp.components_revision,
        cache_compiled,
    );
    compile_cache_lock_wait_ms += elapsed_ms(write_lock_started);
    maybe_write_compiled_app_artifact(
        source_root,
        app_id,
        &alias_options,
        &revision_stamp,
        &compiled,
    );
    Ok(CompileWithCacheOutcomeShared {
        compiled,
        cache_hit: false,
        artifact_cache_hit: false,
        compile_revision: revision_stamp.token.clone(),
        revision_scope: revision_stamp.scope.to_string(),
        cache_validation: "miss".to_string(),
        cache_lookup_ms,
        artifact_load_ms: 0,
        compile_cache_lock_wait_ms,
        compile_ms,
    })

}
