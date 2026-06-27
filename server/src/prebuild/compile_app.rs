use super::prelude::*;
use super::*;
use crate::block::{parse_block_id, BlockOrchestrator};
use crate::graph::types::GraphNodeKind;

pub(crate) fn run_prebuild_for_app(
    source_root: &Path,
    app: &RuntimeWarmupApp,
    mode: PrebuildMode,
    scope_profile: PrebuildScopeProfile,
    dirty_only: bool,
    block_node: Option<String>,
    diagnose_on_fail: bool,
    continue_from: Option<String>,
) -> Result<PrebuildAppReport> {
    let effective_profile = if block_node.is_some() {
        PrebuildScopeProfile::BlockScoped
    } else {
        scope_profile
    };
    let block_scoped = effective_profile == PrebuildScopeProfile::BlockScoped;
    let app_started = Instant::now();
    PrebuildPhaseTracker::global().set_phase(
        source_root,
        "mcg_plan",
        Some(app.app_id.as_str()),
        Some(&format!("hot-only={}", effective_profile == PrebuildScopeProfile::HotOnly)),
    );
    let components_root = toolchain::resolve_components_root(source_root);
    let app_root = resolve_app_root(source_root, app.app_id.as_str());
    let compile_index = load_prebuild_compile_index(app_root.as_path()).unwrap_or_else(|error| {
        tracing::warn!(
            app_id = %app.app_id,
            error = %error,
            "load prebuild compile index failed; fallback to baseline compile flow"
        );
        None
    });
    let diagnostics = Arc::new(PrebuildDiagnostics::default());
    let skip_discover = block_scoped
        || app
            .compile_scope
            .as_ref()
            .is_some_and(|scope| scope.should_skip_discover());
    let compile_session = Arc::new(Mutex::new(PrebuildCompileSession {
        hot_only_scene_ids: if effective_profile == PrebuildScopeProfile::HotOnly {
            Some(
                hot_scene_ids(app)
                    .into_iter()
                    .collect::<BTreeSet<_>>(),
            )
        } else {
            None
        },
        skip_discover,
        compile_scope: app.compile_scope.clone(),
        discover_enqueue_compile: prebuild_discover_enqueue_compile_enabled(),
        ..PrebuildCompileSession::default()
    }));
    let manifest_plan = build_prebuild_manifest_plan(app, effective_profile);
    let warmup_requests = manifest_plan.warmup_requests.clone();
    prebuild_emit_progress(format!(
        "[{}] 计划 | manifest scope {} (hot {} / deferred {}) | warmup 条目 {}",
        app.app_id,
        manifest_plan.initial_scope_count,
        manifest_plan.hot_scopes.len(),
        manifest_plan.deferred_scopes.len(),
        warmup_requests.len()
    ));
    let max_parallelism = prebuild_parallelism(
        manifest_plan
            .initial_scope_count
            .max(warmup_requests.len())
            .max(1),
    );
    let default_scope = CompileScope::default_scope();
    let pre_mcg_bundle_revisions =
        crate::graph::bundle_unchanged_owners(source_root, app.app_id.as_str());
    let compile_started = Instant::now();
    let initial_scope_count = manifest_plan.initial_scope_count;
    prebuild_emit_progress(&format!(
        "[{}] ── [MCG pass] 编译 .mei ── 约 {initial_scope_count} 个 manifest scope（request-scope 闭包 + 结果复用）",
        app.app_id
    ));
    let hot_scopes = order_scopes_target_first(manifest_plan.hot_scopes.clone());
    let deferred_scopes = order_scopes_target_first(manifest_plan.deferred_scopes.clone());
    let mcg_scope_union = hot_scopes
        .iter()
        .cloned()
        .chain(deferred_scopes.iter().cloned())
        .collect::<Vec<_>>();
    let mcg_target_plan = build_mcg_target_plan(mcg_scope_union.as_slice());
    prebuild_emit_progress(format!(
        "[{}] MCG target plan | unique targets {} | manifest scopes {}",
        app.app_id,
        mcg_target_plan.unique_targets.len(),
        manifest_plan.initial_scope_count
    ));
    PrebuildPhaseTracker::global().set_phase(
        source_root,
        "mcg_compile",
        Some(app.app_id.as_str()),
        Some(&format!(
            "default_scope + {} hot targets",
            mcg_target_plan.unique_targets.len()
        )),
    );
    let default_started = Instant::now();
    prebuild_emit_progress(format!(
        "[{}] compiling default_scope…",
        app.app_id
    ));
    let default_reuse = try_reuse_compile_scope_before_load(
        compile_session.as_ref(),
        diagnostics.as_ref(),
        compile_index.as_ref(),
        source_root,
        app.app_id.as_str(),
        &default_scope,
        components_root.as_path(),
    );
    let default_outcome = match default_reuse.as_ref() {
        Some(reuse) => reuse.outcome.clone(),
        None => BlockOrchestrator::compile_scope_for_prebuild(
            compile_session.as_ref(),
            diagnostics.as_ref(),
            source_root,
            app.app_id.as_str(),
            &default_scope,
            mode,
            components_root.as_path(),
        )?,
    };
    prebuild_emit_progress(&format!(
        "[{}] 默认 scope {:.1}s | cache={} | active={}",
        app.app_id,
        default_started.elapsed().as_secs_f64(),
        if default_outcome.cache_hit {
            "命中"
        } else {
            "未命中"
        },
        default_outcome.compiled.active_target_file
    ));
    let mut pending = std::collections::VecDeque::new();
    let mut seen_scopes = BTreeSet::new();
    let mut compile_reports = Vec::new();
    let mut prepared_outcomes = Vec::new();
    record_prebuild_scope_compile_with_discovered(
        source_root,
        app.app_id.as_str(),
        compile_session.as_ref(),
        &default_scope,
        &default_outcome,
        default_reuse
            .as_ref()
            .filter(|reuse| !reuse.discovered_scopes.is_empty())
            .map(|reuse| reuse.discovered_scopes.as_slice()),
        default_reuse
            .as_ref()
            .map(|reuse| reuse.observed_count)
            .unwrap_or(1),
        &mut seen_scopes,
        &mut pending,
        &mut prepared_outcomes,
        &mut compile_reports,
    );
    let mut warnings = Vec::new();
    if block_scoped {
        if let Some(node) = block_node.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            compile_block_scoped_node(
                source_root,
                app,
                node,
                mode,
                compile_session.as_ref(),
                diagnostics.as_ref(),
                components_root.as_path(),
                &mut seen_scopes,
                &mut pending,
                &mut prepared_outcomes,
                &mut compile_reports,
                &mut warnings,
            )?;
        }
    }
    let hot_total = manifest_plan.hot_scopes.len();
    if !block_scoped {
    for (idx, scope) in hot_scopes.into_iter().enumerate() {
        if !seen_scopes.insert(scope.key()) {
            continue;
        }
        let scene = scope.requested_scene_id.clone().unwrap_or_default();
        let target = scope.requested_target_file.clone().unwrap_or_default();
        let hot_started = Instant::now();
        match try_reuse_compile_scope_before_load(
            compile_session.as_ref(),
            diagnostics.as_ref(),
            compile_index.as_ref(),
            source_root,
            app.app_id.as_str(),
            &scope,
            components_root.as_path(),
        )
        .map(Ok)
        .unwrap_or_else(|| {
            BlockOrchestrator::compile_scope_for_prebuild(
                compile_session.as_ref(),
                diagnostics.as_ref(),
                source_root,
                app.app_id.as_str(),
                &scope,
                mode,
                components_root.as_path(),
            )
            .map(|outcome| PersistedCompileIndexReuse {
                outcome,
                discovered_scopes: Vec::new(),
                observed_count: 1,
            })
        }) {
            Ok(reuse) => {
                let PersistedCompileIndexReuse {
                    outcome,
                    discovered_scopes,
                    observed_count,
                } = reuse;
                if !outcome.cache_hit {
                    let file = format_scope_file(
                        scene.as_str(),
                        target.as_str(),
                        Some(outcome.compiled.active_target_file.as_str()),
                    );
                    prebuild_emit_progress(&format!(
                        "[{}] 编译 {:.1}s | hot {}/{} | scene={scene} | file={file}",
                        app.app_id,
                        hot_started.elapsed().as_secs_f64(),
                        idx + 1,
                        hot_total
                    ));
                }
                record_prebuild_scope_compile_with_discovered(
                    source_root,
                    app.app_id.as_str(),
                    compile_session.as_ref(),
                    &scope,
                    &outcome,
                    Some(discovered_scopes.as_slice()),
                    observed_count,
                    &mut seen_scopes,
                    &mut pending,
                    &mut prepared_outcomes,
                    &mut compile_reports,
                );
            }
            Err(error) => {
                if mode == PrebuildMode::Verify {
                    return Err(error);
                }
                warnings.push(build_prebuild_warning(
                    "compile_scope",
                    scope.requested_scene_id.as_deref(),
                    scope.requested_target_file.as_deref(),
                    None,
                    None,
                    None,
                    None,
                    error.to_string(),
                ));
            }
        }
    }
    let deferred_total = deferred_scopes.len();
    if effective_profile == PrebuildScopeProfile::Full {
        for (idx, scope) in deferred_scopes.into_iter().enumerate() {
            if seen_scopes.insert(scope.key()) {
                tracing::debug!(
                    "prebuild compile deferred scope queued app_id={} idx={}/{} scene={} target={}",
                    app.app_id,
                    idx + 1,
                    deferred_total,
                    scope.requested_scene_id.as_deref().unwrap_or(""),
                    scope.requested_target_file.as_deref().unwrap_or("")
                );
                pending.push_back(scope);
            }
        }
    } else if deferred_total > 0 {
        prebuild_emit_progress(format!(
            "[{}] hot-only 跳过 deferred manifest scope {deferred_total} 个（不进入 discover 队列）",
            app.app_id
        ));
    }
    let mut batch_idx = 0usize;
    if !pending.is_empty() {
        prebuild_emit_progress(format!(
            "[{}] scope 队列就绪 | 已完成 {} | 待处理 {}（含 discover 展开）",
            app.app_id,
            compile_reports.len(),
            pending.len()
        ));
    }
    while !pending.is_empty() {
        batch_idx += 1;
        let queue_depth = pending.len();
        let batch = pending.drain(..).collect::<Vec<_>>();
        let batch_size = batch.len();
        let scopes_completed_before_batch = compile_reports.len();
        let mut session_hits = Vec::new();
        let mut to_compile = Vec::new();
        {
            let session = compile_session
                .lock()
                .expect("prebuild compile session lock");
            for scope in batch {
                if let Some(outcome) = session.try_reuse(source_root, app.app_id.as_str(), &scope) {
                    session_hits.push((scope, outcome));
                } else {
                    to_compile.push(scope);
                }
            }
        }
        let session_hit_count = session_hits.len();
        for (scope, outcome) in session_hits {
            diagnostics
                .compile_preload_reuse_hits
                .fetch_add(1, Ordering::Relaxed);
            compile_session
                .lock()
                .expect("prebuild compile session lock")
                .note_scope_alias(&scope, &outcome);
            record_prebuild_scope_compile(
                source_root,
                app.app_id.as_str(),
                compile_session.as_ref(),
                &scope,
                &outcome,
                &mut seen_scopes,
                &mut pending,
                &mut prepared_outcomes,
                &mut compile_reports,
            );
        }
        let mut index_hits = Vec::new();
        let mut to_compile_after_index = Vec::new();
        for scope in to_compile {
            if let Some(outcome) = try_reuse_persisted_compile_index(
                compile_session.as_ref(),
                diagnostics.as_ref(),
                compile_index.as_ref(),
                source_root,
                app.app_id.as_str(),
                &scope,
                components_root.as_path(),
            ) {
                index_hits.push((scope, outcome));
            } else {
                to_compile_after_index.push(scope);
            }
        }
        let index_hit_count = index_hits.len();
        for (scope, reuse) in index_hits {
            record_prebuild_scope_compile_with_discovered(
                source_root,
                app.app_id.as_str(),
                compile_session.as_ref(),
                &scope,
                &reuse.outcome,
                Some(reuse.discovered_scopes.as_slice()),
                reuse.observed_count,
                &mut seen_scopes,
                &mut pending,
                &mut prepared_outcomes,
                &mut compile_reports,
            );
        }
        let compile_groups = group_scopes_by_compile_cache_key(
            source_root,
            app.app_id.as_str(),
            to_compile_after_index,
        );
        let unique_keys = compile_groups.len();
        prebuild_emit_progress(&format!(
            "[{}] 编译 batch-{batch_idx} | 本批 {batch_size} scope | 入队深度 {queue_depth} | 累计已完成 {scopes_completed_before_batch} | session 复用 {session_hit_count} | index 复用 {index_hit_count} | 唯一 cache key {unique_keys}",
            app.app_id,
        ));
        let batch_started = Instant::now();
        let batch_done = Arc::new(AtomicUsize::new(0));
        let batch_new_compile = Arc::new(AtomicUsize::new(0));
        let batch_cache_hits = Arc::new(AtomicUsize::new(0));
        let last_progress_emit = Arc::new(Mutex::new(Instant::now()));
        let representatives = compile_groups
            .iter()
            .map(|(scope, _)| scope.clone())
            .collect::<Vec<_>>();
        let app_id_for_hook = app.app_id.clone();
        let batch_done_hook = Arc::clone(&batch_done);
        let batch_new_hook = Arc::clone(&batch_new_compile);
        let batch_cache_hook = Arc::clone(&batch_cache_hits);
        let last_emit_hook = Arc::clone(&last_progress_emit);
        let last_emit_for_progress = Arc::clone(&last_emit_hook);
        let unique_key_total = representatives.len();
        let batch_results = run_limited_parallel_ordered_with_hook(
            representatives.clone(),
            max_parallelism,
            |scope| {
                BlockOrchestrator::compile_scope_for_prebuild(
                    compile_session.as_ref(),
                    diagnostics.as_ref(),
                    source_root,
                    app.app_id.as_str(),
                    &scope,
                    mode,
                    components_root.as_path(),
                )
            },
            move |_, outcome| {
                let done = batch_done_hook.fetch_add(1, Ordering::Relaxed) + 1;
                match &outcome {
                    Ok(outcome) if outcome.cache_hit => {
                        batch_cache_hook.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(outcome) if outcome.compile_ms > 0 => {
                        batch_new_hook.fetch_add(1, Ordering::Relaxed);
                    }
                    _ => {}
                }
                emit_compile_batch_progress(
                    app_id_for_hook.as_str(),
                    batch_idx,
                    done,
                    unique_key_total,
                    batch_started,
                    scopes_completed_before_batch,
                    unique_key_total.saturating_sub(done),
                    batch_new_hook.load(Ordering::Relaxed),
                    batch_cache_hook.load(Ordering::Relaxed),
                    false,
                    last_emit_for_progress.as_ref(),
                );
            },
        );
        let mut batch_compiled = 0usize;
        let mut batch_cache_hit = 0usize;
        let mut outcomes_by_key = BTreeMap::<String, SharedCompileOutcome>::new();
        for (scope, outcome) in representatives.into_iter().zip(batch_results) {
            let outcome = match outcome {
                Ok(outcome) => outcome,
                Err(error) => {
                    if mode == PrebuildMode::Verify {
                        return Err(error);
                    }
                    warnings.push(build_prebuild_warning(
                        "compile_scope",
                        scope.requested_scene_id.as_deref(),
                        scope.requested_target_file.as_deref(),
                        None,
                        None,
                        None,
                        None,
                        error.to_string(),
                    ));
                    continue;
                }
            };
            if outcome.cache_hit {
                batch_cache_hit += 1;
            } else if outcome.compile_ms > 0 {
                batch_compiled += 1;
            }
            outcomes_by_key.insert(scope.key(), outcome);
        }
        for (representative, aliases) in compile_groups {
            let Some(outcome) = outcomes_by_key.get(&representative.key()) else {
                continue;
            };
            record_prebuild_scope_compile(
                source_root,
                app.app_id.as_str(),
                compile_session.as_ref(),
                &representative,
                outcome,
                &mut seen_scopes,
                &mut pending,
                &mut prepared_outcomes,
                &mut compile_reports,
            );
            for alias in aliases {
                let alias_outcome =
                    scope_assembled_outcome(source_root, app.app_id.as_str(), outcome, &alias);
                compile_session
                    .lock()
                    .expect("prebuild compile session lock")
                    .register(source_root, app.app_id.as_str(), &alias, alias_outcome.clone());
                record_prebuild_scope_compile_with_discovered(
                    source_root,
                    app.app_id.as_str(),
                    compile_session.as_ref(),
                    &alias,
                    &alias_outcome,
                    Some(&[] as &[CompileScope]),
                    1,
                    &mut seen_scopes,
                    &mut pending,
                    &mut prepared_outcomes,
                    &mut compile_reports,
                );
            }
        }
        emit_compile_batch_progress(
            app.app_id.as_str(),
            batch_idx,
            unique_key_total,
            unique_key_total,
            batch_started,
            scopes_completed_before_batch,
            pending.len(),
            batch_compiled,
            batch_cache_hit,
            true,
            last_emit_hook.as_ref(),
        );
    }
    }
    if mode == PrebuildMode::Build {
        let index = build_prebuild_compile_index(
            source_root,
            app.app_id.as_str(),
            prepared_outcomes.as_slice(),
            compile_reports.as_slice(),
        );
        if let Err(error) = write_prebuild_compile_index(app_root.as_path(), &index) {
            tracing::warn!(
                app_id = %app.app_id,
                error = %error,
                "write prebuild compile index failed"
            );
        }
    }
    if !block_scoped {
        let mut manifest_scopes = vec![default_scope.clone()];
        manifest_scopes.extend(manifest_plan.hot_scopes.iter().cloned());
        manifest_scopes.extend(manifest_plan.deferred_scopes.iter().cloned());
        fill_manifest_prepared_outcomes(
            source_root,
            app.app_id.as_str(),
            manifest_scopes.as_slice(),
            compile_session.as_ref(),
            &mut prepared_outcomes,
            &mut compile_reports,
            &mut seen_scopes,
        );
    }
    let compile_scopes_ms = compile_started.elapsed().as_millis() as u64;
    diagnostics.sample_memory_peak();
    prebuild_emit_progress(&format!(
        "[{}] ── 1/3 编译完成 {:.1}s | 共 {} scope ──",
        app.app_id,
        compile_scopes_ms as f64 / 1000.0,
        compile_reports.len()
    ));

    finish_run_prebuild_for_app(
        source_root,
        app,
        mode,
        PrebuildAppAfterCompile {
            scope_profile: effective_profile,
            app_started,
            app_root,
            components_root,
            diagnostics,
            manifest_plan,
            warmup_requests,
            max_parallelism,
            pre_mcg_bundle_revisions,
            initial_scope_count,
            compile_scopes_ms,
            compile_reports,
            prepared_outcomes,
            compile_session,
            warnings,
            dirty_only,
            block_node,
            diagnose_on_fail,
            continue_from,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn compile_block_scoped_node(
    source_root: &Path,
    app: &RuntimeWarmupApp,
    block_node: &str,
    mode: PrebuildMode,
    compile_session: &Mutex<PrebuildCompileSession>,
    diagnostics: &PrebuildDiagnostics,
    components_root: &Path,
    seen_scopes: &mut BTreeSet<String>,
    pending: &mut std::collections::VecDeque<CompileScope>,
    prepared_outcomes: &mut Vec<PreparedCompileOutcome>,
    compile_reports: &mut Vec<PrebuildScopeReport>,
    warnings: &mut Vec<PrebuildWarningReport>,
) -> Result<()> {
    let block_id = parse_block_id(block_node)?;
    match block_id.kind {
        GraphNodeKind::ScenePayload => {
            let scope = CompileScope {
                requested_scene_id: block_id.scope_key,
                requested_target_file: Some(block_id.key),
            }
            .canonicalized();
            if !seen_scopes.insert(scope.key()) {
                return Ok(());
            }
            let started = Instant::now();
            match BlockOrchestrator::compile_scope_for_prebuild(
                compile_session,
                diagnostics,
                source_root,
                app.app_id.as_str(),
                &scope,
                mode,
                components_root,
            ) {
                Ok(outcome) => {
                    prebuild_emit_progress(format!(
                        "[{}] block-scoped 编译 {:.1}s | scene={} | file={}",
                        app.app_id,
                        started.elapsed().as_secs_f64(),
                        scope.requested_scene_id.as_deref().unwrap_or(""),
                        scope
                            .requested_target_file
                            .as_deref()
                            .unwrap_or("")
                    ));
                    record_prebuild_scope_compile_with_discovered(
                        source_root,
                        app.app_id.as_str(),
                        compile_session,
                        &scope,
                        &outcome,
                        Some(&[]),
                        1,
                        seen_scopes,
                        pending,
                        prepared_outcomes,
                        compile_reports,
                    );
                }
                Err(error) => {
                    if mode == PrebuildMode::Verify {
                        return Err(error);
                    }
                    warnings.push(build_prebuild_warning(
                        "compile_scope",
                        scope.requested_scene_id.as_deref(),
                        scope.requested_target_file.as_deref(),
                        None,
                        None,
                        None,
                        None,
                        error.to_string(),
                    ));
                }
            }
        }
        _ => {
            BlockOrchestrator::compile(source_root, app.app_id.as_str(), &block_id, false)?;
        }
    }
    Ok(())
}
