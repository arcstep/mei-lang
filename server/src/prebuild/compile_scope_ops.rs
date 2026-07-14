use super::prelude::*;
use super::*;
use crate::block::BlockOrchestrator;
use crate::graph::types::GraphNodeKind;

pub(crate) fn mcg_scene_payload_registered(
    source_root: &Path,
    app_id: &str,
    target_file: &str,
) -> bool {
    if !crate::graph::feature::graph_registry_dedup_enabled() {
        return true;
    }
    let registry = crate::graph::mcg::registry::McgRegistryWriter::load(source_root, app_id);
    mei_lang_kernel::app_source_rel_path_lookup_keys(target_file)
        .into_iter()
        .any(|key| {
            registry.nodes.iter().any(|node| {
                node.id.kind == GraphNodeKind::ScenePayload
                    && node.id.key == key
                    && node.state == crate::graph::types::MaterialState::Ready
            })
        })
}

fn scope_target_file(scope: &CompileScope, compiled: &mei_lang_kernel::CompiledApp) -> String {
    scope
        .canonicalized()
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| compiled.active_target_file.clone())
}

fn mcg_scene_payload_registered_for_scope(
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    compiled: &mei_lang_kernel::CompiledApp,
) -> bool {
    let target = scope_target_file(scope, compiled);
    if target.trim().is_empty() {
        return true;
    }
    mcg_scene_payload_registered(source_root, app_id, target.as_str())
}

fn is_home_assembly_target(target: &str) -> bool {
    let canonical = mei_lang_kernel::canonical_app_source_rel_path(target.trim());
    canonical.ends_with("home.mei")
}

pub(crate) fn is_board_target_file(target: &str) -> bool {
    target.trim().ends_with(".board.mei")
}

pub(crate) fn maybe_shrink_board_projection_outcome(
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    outcome: &mut SharedCompileOutcome,
) {
    let canonical = scope.canonicalized();
    let target = canonical
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if !target.is_some_and(is_board_target_file) {
        return;
    }
    if outcome.handle_only {
        return;
    }
    if !crate::graph::feature::graph_registry_dedup_enabled() {
        return;
    }
    if mcg_scene_payload_registered(source_root, app_id, target.unwrap_or_default()) {
        shrink_outcome_to_handle(outcome, Some(source_root), Some(app_id));
    }
}

pub(crate) fn assembly_base_matches_scope_target(
    scope: &CompileScope,
    base: &SharedCompileOutcome,
) -> bool {
    let canonical = scope.canonicalized();
    let scope_target = canonical
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(scope_target) = scope_target else {
        return true;
    };
    let base_target = base.compiled.active_target_file.trim();
    if base_target == scope_target {
        return true;
    }
    let base_canonical = mei_lang_kernel::canonical_app_source_rel_path(base_target);
    let scope_canonical = mei_lang_kernel::canonical_app_source_rel_path(scope_target);
    base_canonical == scope_canonical
}

fn enqueue_missing_embedded_capsule_scene_payloads(
    source_root: &Path,
    app_id: &str,
    parent_scope: &CompileScope,
    outcome: &SharedCompileOutcome,
    compile_session: &Mutex<PrebuildCompileSession>,
    seen_scopes: &mut BTreeSet<String>,
    pending: &mut std::collections::VecDeque<CompileScope>,
) {
    if !is_home_assembly_target(outcome.compiled.active_target_file.as_str()) {
        return;
    }
    let parent_scene = parent_scope
        .requested_scene_id
        .clone()
        .or_else(|| outcome.compiled.active_scene.clone());
    let capsules =
        crate::graph::embedded_capsule_target_files(source_root, app_id, outcome.compiled.as_ref());
    let mut candidate_scopes = Vec::new();
    for capsule in capsules {
        if mcg_scene_payload_registered(source_root, app_id, capsule.as_str()) {
            continue;
        }
        candidate_scopes.push(
            CompileScope {
                requested_scene_id: parent_scene.clone(),
                requested_target_file: Some(capsule),
            }
            .canonicalized(),
        );
    }
    if candidate_scopes.is_empty() {
        return;
    }
    let locked = compile_session
        .lock()
        .expect("prebuild compile session lock");
    let filtered = locked.filter_hot_only_discovered(candidate_scopes);
    let filtered = locked.filter_compile_scope(filtered);
    drop(locked);
    for scope in filtered {
        if seen_scopes.insert(scope.key()) {
            pending.push_back(scope);
        }
    }
}

fn ensure_mcg_scene_payload_for_scope(
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    outcome: &SharedCompileOutcome,
    mode: PrebuildMode,
) {
    if mode != PrebuildMode::Build {
        return;
    }
    let target = scope_target_file(scope, outcome.compiled.as_ref());
    if target.trim().is_empty()
        || mcg_scene_payload_registered(source_root, app_id, target.as_str())
    {
        return;
    }
    if outcome.compiled.active_target_file.trim() != target.trim() {
        return;
    }
    let options = scope.to_options();
    let payloads = crate::graph::runtime_payloads_from_compiled(&outcome.compiled);
    crate::graph::maybe_update_graph_after_compile(
        source_root,
        app_id,
        &options,
        &outcome.compiled,
        outcome.compile_revision.as_str(),
        &payloads,
    );
}

pub(crate) fn ensure_compile_scope_for_prebuild(
    session: &Mutex<PrebuildCompileSession>,
    diagnostics: &PrebuildDiagnostics,
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    mode: PrebuildMode,
    components_root: &Path,
) -> Result<SharedCompileOutcome> {
    let reused = session_try_reuse(session, source_root, app_id, scope);
    if let Some(reused) = reused {
        if mcg_scene_payload_registered_for_scope(
            source_root,
            app_id,
            scope,
            reused.compiled.as_ref(),
        ) {
            diagnostics
                .compile_preload_reuse_hits
                .fetch_add(1, Ordering::Relaxed);
            session
                .lock()
                .expect("prebuild compile session lock")
                .note_scope_alias(scope, &reused);
            return Ok(reused);
        }
    }
    diagnostics
        .compile_fallback_loads
        .fetch_add(1, Ordering::Relaxed);

    if let Some(target) = scope
        .canonicalized()
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let base = session
            .lock()
            .expect("prebuild compile session lock")
            .try_reuse_base_for_target(target)
            .map(|outcome| outcome.clone());
        if let Some(base) = base {
            if !compile_outcome_matches_scope(scope, base.compiled.as_ref()) {
                let board_payload_ready = !is_board_target_file(target)
                    || mcg_scene_payload_registered(source_root, app_id, target);
                if assembly_base_matches_scope_target(scope, &base) && board_payload_ready {
                    let assembled = scope_assembled_outcome(
                        source_root,
                        app_id,
                        &base,
                        scope,
                        Some(diagnostics),
                    );
                    session
                        .lock()
                        .expect("prebuild compile session lock")
                        .register(source_root, app_id, scope, assembled.clone());
                    ensure_mcg_scene_payload_for_scope(
                        source_root,
                        app_id,
                        scope,
                        &assembled,
                        mode,
                    );
                    return Ok(assembled);
                }
            }
        }
    }

    if let Some(target) = scope
        .canonicalized()
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some((compiled, compile_revision)) =
            crate::graph::try_assemble_scope_from_scene_payload(
                source_root,
                app_id,
                scope.canonicalized().requested_scene_id.as_deref(),
                target,
            )
        {
            diagnostics
                .mcg_assemble_only_count
                .fetch_add(1, Ordering::Relaxed);
            let outcome = SharedCompileOutcome {
                compiled: Arc::new(compiled),
                cache_hit: true,
                artifact_cache_hit: false,
                assemble_only: true,
                compile_revision,
                cache_lookup_ms: 0,
                artifact_load_ms: 0,
                compile_ms: 0,
                handle_only: false,
                assembly_handle: None,
            };
            let mut locked = session.lock().expect("prebuild compile session lock");
            locked.register(source_root, app_id, scope, outcome.clone());
            ensure_mcg_scene_payload_for_scope(source_root, app_id, scope, &outcome, mode);
            return Ok(outcome);
        }
    }

    let outcome = match mode {
        PrebuildMode::Build | PrebuildMode::Verify => toolchain::load_compile_artifact_only_shared(
            source_root,
            app_id,
            &scope.to_options(),
            components_root,
        ),
    };
    let outcome = match outcome {
        Some(outcome) => {
            let outcome = SharedCompileOutcome::from_shared(outcome);
            if compile_outcome_matches_scope(scope, &outcome.compiled) {
                outcome
            } else {
                diagnostics
                    .compile_index_stale_entries
                    .fetch_add(1, Ordering::Relaxed);
                BlockOrchestrator::compile_scope(source_root, app_id, scope, mode, false)?
            }
        }
        None => BlockOrchestrator::compile_scope(source_root, app_id, scope, mode, false)?,
    };
    if mode == PrebuildMode::Build {
        let options = scope.to_options();
        let payloads = crate::graph::runtime_payloads_from_compiled(&outcome.compiled);
        crate::graph::maybe_update_graph_after_compile(
            source_root,
            app_id,
            &options,
            &outcome.compiled,
            outcome.compile_revision.as_str(),
            &payloads,
        );
    }
    let identity = compiled_scope_identity(&outcome);
    let mut locked = session.lock().expect("prebuild compile session lock");
    if let Some(existing) = locked.by_identity.get(&identity).cloned() {
        if mcg_scene_payload_registered_for_scope(
            source_root,
            app_id,
            scope,
            existing.compiled.as_ref(),
        ) {
            diagnostics
                .compile_postload_identity_collapses
                .fetch_add(1, Ordering::Relaxed);
            locked.register(source_root, app_id, scope, existing.clone());
            return Ok(mark_prebuild_session_reuse(&existing));
        }
    }
    locked.register(source_root, app_id, scope, outcome.clone());
    ensure_mcg_scene_payload_for_scope(source_root, app_id, scope, &outcome, mode);
    Ok(outcome)
}

pub(crate) fn record_prebuild_scope_compile_with_discovered(
    source_root: &Path,
    app_id: &str,
    compile_session: &Mutex<PrebuildCompileSession>,
    scope: &CompileScope,
    outcome: &SharedCompileOutcome,
    discovered_scopes: Option<&[CompileScope]>,
    observed_count: usize,
    seen_scopes: &mut BTreeSet<String>,
    pending: &mut std::collections::VecDeque<CompileScope>,
    prepared_outcomes: &mut Vec<PreparedCompileOutcome>,
    compile_reports: &mut Vec<PrebuildScopeReport>,
) {
    compile_reports.push(scope_report_from_outcome(scope, outcome));
    let mut locked = compile_session
        .lock()
        .expect("prebuild compile session lock");
    if locked.skip_discover {
        drop(locked);
    } else if locked.should_discover(scope) {
        let discovered_iter = discovered_scopes
            .map(|scopes| scopes.to_vec())
            .unwrap_or_else(|| discovered_compile_scopes(scope, &outcome.compiled));
        let filtered = locked.filter_board_discovered_scopes(scope, discovered_iter.as_slice());
        let filtered = locked.filter_hot_only_discovered(filtered);
        let filtered = locked.filter_compile_scope(filtered);
        let enqueue_discover = locked.discover_enqueue_compile;
        drop(locked);
        if enqueue_discover {
            for discovered in filtered {
                if seen_scopes.insert(discovered.key()) {
                    pending.push_back(discovered);
                }
            }
        } else if !filtered.is_empty() {
            let nav_scopes = filtered
                .iter()
                .filter_map(|discovered| compile_scope_to_nav(discovered, &outcome.compiled))
                .collect::<Vec<_>>();
            if !nav_scopes.is_empty() {
                if let Err(error) =
                    crate::graph::mrg::navigation::sync_navigation_for_compile_scopes(
                        source_root,
                        app_id,
                        nav_scopes.as_slice(),
                    )
                {
                    tracing::debug!(
                        app_id = %app_id,
                        error = %error,
                        "discover navigation sync skipped for {} scopes",
                        nav_scopes.len()
                    );
                }
            }
        }
    } else {
        drop(locked);
    }
    prepared_outcomes.push(PreparedCompileOutcome {
        scope: scope.clone(),
        outcome: outcome.clone(),
    });
    enqueue_missing_embedded_capsule_scene_payloads(
        source_root,
        app_id,
        scope,
        outcome,
        compile_session,
        seen_scopes,
        pending,
    );
    for _ in 1..observed_count.max(1) {
        compile_reports.push(scope_report_from_outcome(scope, outcome));
    }
}

fn compile_scope_to_nav(
    scope: &CompileScope,
    compiled: &mei_lang_kernel::CompiledApp,
) -> Option<crate::graph::mrg::navigation::CompileScopeNav> {
    let canonical = scope.canonicalized();
    let scene_id = canonical
        .requested_scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            compiled
                .active_scene
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })?;
    let target_file = canonical
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| compiled.active_target_file.clone());
    if target_file.trim().is_empty() {
        return None;
    }
    Some(crate::graph::mrg::navigation::CompileScopeNav {
        scene_id,
        target_file,
    })
}

pub(crate) fn fill_manifest_prepared_outcomes(
    source_root: &Path,
    app_id: &str,
    manifest_scopes: &[CompileScope],
    compile_session: &Mutex<PrebuildCompileSession>,
    diagnostics: &PrebuildDiagnostics,
    mode: PrebuildMode,
    components_root: &Path,
    prepared_outcomes: &mut Vec<PreparedCompileOutcome>,
    compile_reports: &mut Vec<PrebuildScopeReport>,
    seen_scopes: &mut BTreeSet<String>,
) {
    let prepared_keys = prepared_outcomes
        .iter()
        .map(|prepared| prepared.scope.key())
        .collect::<BTreeSet<_>>();
    let fallback_base = {
        let session = compile_session
            .lock()
            .expect("prebuild compile session lock");
        session
            .by_target_identity
            .values()
            .chain(session.by_identity.values())
            .max_by_key(|outcome| (outcome.compile_ms, !outcome.cache_hit))
            .cloned()
    };
    for scope in manifest_scopes {
        if prepared_keys.contains(&scope.key()) {
            continue;
        }
        if !seen_scopes.insert(scope.key()) {
            continue;
        }
        let canonical = scope.canonicalized();
        let scope_target = canonical
            .requested_target_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let outcome = if scope_target.is_some_and(is_board_target_file) {
            ensure_compile_scope_for_prebuild(
                compile_session,
                diagnostics,
                source_root,
                app_id,
                scope,
                mode,
                components_root,
            )
            .unwrap_or_else(|error| {
                tracing::warn!(
                    app_id = %app_id,
                    scope = %scope.key(),
                    error = %error,
                    "fill_manifest board compile failed; projection handle only"
                );
                let base = fallback_base
                    .clone()
                    .unwrap_or_else(|| SharedCompileOutcome {
                        compiled: Arc::new(mei_lang_kernel::CompiledApp {
                            app_id: app_id.to_string(),
                            title: String::new(),
                            app_root: String::new(),
                            active_scene: scope.requested_scene_id.clone(),
                            stage_registry: Default::default(),
                            stage_programs: Default::default(),
                            scene_slot_modules: Default::default(),
                            content_capabilities: Default::default(),
                            narration_catalogs: Default::default(),
                            active_target_file: scope_target.unwrap_or_default().to_string(),
                            file_tree: Vec::new(),
                            scene_routes: Vec::new(),
                            scene_contract: None,
                            scene_local_nav_by_target: Default::default(),
                            scene_bindings_by_id: Default::default(),
                            scene_examples_by_id: Default::default(),
                            scene_projection_assembly_by_id: Default::default(),
                            resources: Vec::new(),
                            world_metrics: Default::default(),
                            world_semantic_by_file: Default::default(),
                            component_assets: Vec::new(),
                            diagnostics: Vec::new(),
                            build_experience_index: Default::default(),
                            build_t2_page_index: Default::default(),
                            build_template_index: Default::default(),
                            ui_layout_index: Default::default(),
                        }),
                        cache_hit: true,
                        artifact_cache_hit: false,
                        assemble_only: true,
                        compile_revision: String::new(),
                        cache_lookup_ms: 0,
                        artifact_load_ms: 0,
                        compile_ms: 0,
                        handle_only: true,
                        assembly_handle: None,
                    });
                projection_handle_outcome(scope, &base, None)
            })
        } else {
            let Some(fallback_base) = fallback_base.as_ref() else {
                continue;
            };
            scope_assembled_outcome(source_root, app_id, fallback_base, scope, None)
        };
        compile_session
            .lock()
            .expect("prebuild compile session lock")
            .register(source_root, app_id, scope, outcome.clone());
        compile_reports.push(scope_report_from_outcome(scope, &outcome));
        prepared_outcomes.push(PreparedCompileOutcome {
            scope: scope.clone(),
            outcome,
        });
    }
}

pub(crate) fn record_prebuild_scope_compile(
    source_root: &Path,
    app_id: &str,
    compile_session: &Mutex<PrebuildCompileSession>,
    scope: &CompileScope,
    outcome: &SharedCompileOutcome,
    seen_scopes: &mut BTreeSet<String>,
    pending: &mut std::collections::VecDeque<CompileScope>,
    prepared_outcomes: &mut Vec<PreparedCompileOutcome>,
    compile_reports: &mut Vec<PrebuildScopeReport>,
) {
    record_prebuild_scope_compile_with_discovered(
        source_root,
        app_id,
        compile_session,
        scope,
        outcome,
        None,
        1,
        seen_scopes,
        pending,
        prepared_outcomes,
        compile_reports,
    );
}

pub(crate) fn unique_prepared_outcomes_for_artifacts(
    prepared_outcomes: &[PreparedCompileOutcome],
) -> Vec<PreparedCompileOutcome> {
    let mut best_by_identity = BTreeMap::<String, PreparedCompileOutcome>::new();
    for prepared in prepared_outcomes {
        let identity = compiled_artifact_identity(&prepared.outcome);
        match best_by_identity.get(&identity) {
            Some(existing) => {
                if compile_scope_specificity(&prepared.scope)
                    > compile_scope_specificity(&existing.scope)
                {
                    best_by_identity.insert(identity, prepared.clone());
                }
            }
            None => {
                best_by_identity.insert(identity, prepared.clone());
            }
        }
    }
    best_by_identity.into_values().collect()
}

pub(crate) fn ensure_compile_scope(
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    mode: PrebuildMode,
    components_root: &Path,
) -> Result<SharedCompileOutcome> {
    let options = scope.to_options();
    match mode {
        PrebuildMode::Build => {
            toolchain::compile_app_with_cache_shared(source_root, app_id, options, components_root)
                .map(SharedCompileOutcome::from_shared)
                .map_err(|failure| failure.error)
                .with_context(|| {
                    format!(
                        "compile scope scene=`{}` target=`{}` for app `{app_id}`",
                        scope.requested_scene_id.as_deref().unwrap_or(""),
                        scope.requested_target_file.as_deref().unwrap_or("")
                    )
                })
        }
        PrebuildMode::Verify => toolchain::load_compile_artifact_only_shared(
            source_root,
            app_id,
            &options,
            components_root,
        )
        .map(SharedCompileOutcome::from_shared)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "missing compile artifact for app `{app_id}` scene=`{}` target=`{}`",
                scope.requested_scene_id.as_deref().unwrap_or(""),
                scope.requested_target_file.as_deref().unwrap_or("")
            )
        }),
    }
}

pub(crate) fn collect_required_xlsx_sources<'a>(
    app: &RuntimeWarmupApp,
    compiled_apps: impl Iterator<Item = &'a mei_lang_kernel::CompiledApp>,
) -> BTreeSet<(String, Option<String>, usize)> {
    let mut out = BTreeSet::new();
    for source in &app.xlsx_sources {
        let path = source.path.trim();
        if path.is_empty() {
            continue;
        }
        out.insert((
            path.to_string(),
            source
                .sheet
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            source.header_row.unwrap_or(1).max(1),
        ));
    }
    for compiled in compiled_apps {
        for resource in &compiled.resources {
            let Some(dataset) = resource.dataset.as_ref() else {
                continue;
            };
            if !matches!(dataset.source.kind.trim(), "xlsx" | "xls") {
                continue;
            }
            out.insert((
                dataset.source.path.trim().to_string(),
                dataset
                    .source
                    .sheet
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string),
                dataset.source.header_row.unwrap_or(1).max(1) as usize,
            ));
        }
    }
    out
}

pub(crate) fn publish_required_data_snapshots(
    source_root: &Path,
    app_id: &str,
    required_sources: Vec<(String, Option<String>, usize)>,
) -> Result<PublishDataSnapshotsReport> {
    let app_root = resolve_app_root(source_root, app_id);
    let all_ready = required_sources.iter().all(|(path, sheet, header_row)| {
        resolve_data_snapshot_import_entry(
            app_root.as_path(),
            path.as_str(),
            sheet.as_deref(),
            *header_row,
        )
        .is_some()
    });
    if all_ready {
        let discovered_sources = required_sources
            .iter()
            .map(|(path, sheet, header_row)| {
                format!(
                    "{}|sheet={}|header_row={}",
                    path,
                    sheet.as_deref().unwrap_or(""),
                    header_row
                )
            })
            .collect::<Vec<_>>();
        return Ok(PublishDataSnapshotsReport {
            app_id: app_id.to_string(),
            discovered_sources,
            written: Vec::new(),
            manifest_path: data_snapshot_import_manifest_path(app_root.as_path())
                .display()
                .to_string(),
        });
    }
    let refs = required_sources
        .iter()
        .map(|(path, sheet, header_row)| (path.as_str(), sheet.as_deref(), *header_row))
        .collect::<Vec<_>>();
    let report = toolchain::publish_data_snapshots(source_root, app_id, refs.as_slice())
        .with_context(|| format!("publish data snapshots for app `{app_id}`"))?;
    for source_key in &report.written {
        let _ = crate::graph::mrg::eval_nodes::persist_data_source_node(
            source_root,
            app_id,
            source_key.as_str(),
            "ds:published",
            source_key.as_str(),
        );
    }
    Ok(report)
}

pub(crate) fn verify_required_xlsx_sources(
    app_root: &Path,
    required_sources: &BTreeSet<(String, Option<String>, usize)>,
) -> Result<()> {
    for (path, sheet, header_row) in required_sources {
        if resolve_data_snapshot_import_entry(
            app_root,
            path.as_str(),
            sheet.as_deref(),
            *header_row,
        )
        .is_none()
        {
            anyhow::bail!(
                "missing import snapshot for `{}` (sheet=`{}`, header_row={})",
                path,
                sheet.as_deref().unwrap_or(""),
                header_row
            );
        }
    }
    Ok(())
}

pub(crate) fn shrink_prepared_outcomes_with_mcg_handles(
    source_root: &Path,
    app_id: &str,
    prepared_outcomes: &mut [PreparedCompileOutcome],
) {
    if !crate::graph::feature::graph_registry_dedup_enabled() {
        return;
    }
    for prepared in prepared_outcomes.iter_mut() {
        let target = prepared.outcome.compiled.active_target_file.as_str();
        if target.trim().is_empty() {
            continue;
        }
        if mcg_scene_payload_registered(source_root, app_id, target)
            && !prepared.outcome.handle_only
        {
            shrink_outcome_to_handle(&mut prepared.outcome, Some(source_root), Some(app_id));
        }
    }
}
