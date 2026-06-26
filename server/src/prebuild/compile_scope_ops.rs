use super::prelude::*;
use super::*;

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
        diagnostics
            .compile_preload_reuse_hits
            .fetch_add(1, Ordering::Relaxed);
        session
            .lock()
            .expect("prebuild compile session lock")
            .note_scope_alias(scope, &reused);
        return Ok(reused);
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
        if let Some((compiled, compile_revision)) = crate::graph::try_assemble_scope_from_scene_payload(
            source_root,
            app_id,
            scope.canonicalized().requested_scene_id.as_deref(),
            target,
        ) {
            let outcome = SharedCompileOutcome {
                compiled: Arc::new(compiled),
                cache_hit: true,
                artifact_cache_hit: false,
                compile_revision,
                cache_lookup_ms: 0,
                artifact_load_ms: 0,
                compile_ms: 0,
            };
            let mut locked = session.lock().expect("prebuild compile session lock");
            locked.register(source_root, app_id, scope, outcome.clone());
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
                ensure_compile_scope(source_root, app_id, scope, mode, components_root)?
            }
        }
        None => ensure_compile_scope(source_root, app_id, scope, mode, components_root)?,
    };
    if mode == PrebuildMode::Build && outcome.compile_ms > 0 {
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
        diagnostics
            .compile_postload_identity_collapses
            .fetch_add(1, Ordering::Relaxed);
        locked.register(source_root, app_id, scope, existing.clone());
        return Ok(mark_prebuild_session_reuse(&existing));
    }
    locked.register(source_root, app_id, scope, outcome.clone());
    Ok(outcome)
}

pub(crate) fn record_prebuild_scope_compile_with_discovered(
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
    if locked.should_discover(scope) {
        let discovered_iter = discovered_scopes
            .map(|scopes| scopes.to_vec())
            .unwrap_or_else(|| discovered_compile_scopes(scope, &outcome.compiled));
        let filtered = locked.filter_board_discovered_scopes(scope, discovered_iter.as_slice());
        drop(locked);
        for discovered in filtered {
            if seen_scopes.insert(discovered.key()) {
                pending.push_back(discovered);
            }
        }
    } else {
        drop(locked);
    }
    prepared_outcomes.push(PreparedCompileOutcome {
        scope: scope.clone(),
        outcome: outcome.clone(),
    });
    for _ in 1..observed_count.max(1) {
        compile_reports.push(scope_report_from_outcome(scope, outcome));
    }
}

pub(crate) fn record_prebuild_scope_compile(
    compile_session: &Mutex<PrebuildCompileSession>,
    scope: &CompileScope,
    outcome: &SharedCompileOutcome,
    seen_scopes: &mut BTreeSet<String>,
    pending: &mut std::collections::VecDeque<CompileScope>,
    prepared_outcomes: &mut Vec<PreparedCompileOutcome>,
    compile_reports: &mut Vec<PrebuildScopeReport>,
) {
    record_prebuild_scope_compile_with_discovered(
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
        let identity = compiled_scope_identity(&prepared.outcome);
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
    toolchain::publish_data_snapshots(source_root, app_id, refs.as_slice())
        .with_context(|| format!("publish data snapshots for app `{app_id}`"))
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

