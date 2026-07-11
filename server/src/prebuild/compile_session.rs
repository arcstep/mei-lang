use super::prelude::*;
use super::*;

#[derive(Default)]
pub(crate) struct PrebuildCompileSession {
    pub(crate) by_scope_key: BTreeMap<String, SharedCompileOutcome>,
    pub(crate) by_compile_cache_key: BTreeMap<String, SharedCompileOutcome>,
    pub(crate) by_identity: BTreeMap<String, SharedCompileOutcome>,
    /// One canonical compile outcome per `(target_file|compile_revision)`.
    pub(crate) by_target_identity: BTreeMap<String, SharedCompileOutcome>,
    pub(crate) discovered_scope_keys: BTreeSet<String>,
    pub(crate) peak_identity_entries: usize,
    /// Each `.board.mei` target is expanded at most once per prebuild compile phase.
    pub(crate) expanded_board_targets: BTreeSet<String>,
    /// When set, discover expansion only keeps scopes for these scene ids (+ target-only scopes).
    pub(crate) hot_only_scene_ids: Option<BTreeSet<String>>,
    /// When set, discover expansion is skipped (block-scoped MCG pass or compileScope.skipDiscover).
    pub(crate) skip_discover: bool,
    /// When set, discover expansion only keeps scopes allowed by workspace compileScope.
    pub(crate) compile_scope: Option<mei_lang_kernel::CompileScopeFilterConfig>,
    /// When false (default), discover only syncs MRG navigation — no pending compile enqueue.
    pub(crate) discover_enqueue_compile: bool,
}

pub(crate) fn prebuild_discover_enqueue_compile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("MEI_PREBUILD_DISCOVER_ENQUEUE")
            .ok()
            .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
            .unwrap_or(false)
    })
}

impl PrebuildCompileSession {
    pub(crate) fn register(
        &mut self,
        source_root: &Path,
        app_id: &str,
        scope: &CompileScope,
        outcome: SharedCompileOutcome,
    ) {
        let identity = compiled_scope_identity(&outcome);
        let cache_key = toolchain::compile_cache_key(source_root, app_id, &scope.to_options());
        self.by_scope_key
            .entry(scope.key())
            .or_insert_with(|| outcome.clone());
        self.by_compile_cache_key
            .entry(cache_key)
            .or_insert_with(|| outcome.clone());
        self.by_identity.entry(identity).or_insert(outcome.clone());
        let target_identity = compiled_target_identity(&outcome);
        if !target_identity.is_empty() {
            self.by_target_identity
                .entry(target_identity)
                .or_insert(outcome);
        }
        self.peak_identity_entries = self.peak_identity_entries.max(self.by_identity.len());
    }

    /// Drop identity/scope entries for other targets after a target group completes compile.
    pub(crate) fn evict_non_canonical_for_target(&mut self, keep_target: &str) {
        let keep_target = keep_target.trim();
        if keep_target.is_empty() {
            return;
        }
        let prefix = format!("{keep_target}|");
        self.by_identity.retain(|_, outcome| {
            let target_identity = compiled_target_identity(outcome);
            target_identity.is_empty() || target_identity.starts_with(prefix.as_str())
        });
        self.by_scope_key.retain(|_, outcome| {
            let target_identity = compiled_target_identity(outcome);
            target_identity.is_empty() || target_identity.starts_with(prefix.as_str())
        });
        self.by_compile_cache_key.retain(|_, outcome| {
            let target_identity = compiled_target_identity(outcome);
            target_identity.is_empty() || target_identity.starts_with(prefix.as_str())
        });
        self.peak_identity_entries = self.peak_identity_entries.max(self.by_identity.len());
    }

    pub(crate) fn try_reuse_base_for_target(
        &self,
        target_file: &str,
    ) -> Option<SharedCompileOutcome> {
        let target = target_file.trim();
        if target.is_empty() {
            return None;
        }
        let prefix = format!("{target}|");
        self.by_target_identity
            .iter()
            .find(|(key, _)| key.starts_with(prefix.as_str()))
            .map(|(_, outcome)| outcome.clone())
    }

    pub(crate) fn try_reuse(
        &self,
        source_root: &Path,
        app_id: &str,
        scope: &CompileScope,
    ) -> Option<SharedCompileOutcome> {
        let cache_key = toolchain::compile_cache_key(source_root, app_id, &scope.to_options());
        if let Some(outcome) = self.by_compile_cache_key.get(&cache_key) {
            if compile_outcome_matches_scope(scope, &outcome.compiled) {
                return Some(mark_prebuild_session_reuse(outcome));
            }
        }
        if let Some(outcome) = self.by_scope_key.get(&scope.key()) {
            if compile_outcome_matches_scope(scope, &outcome.compiled) {
                return Some(mark_prebuild_session_reuse(outcome));
            }
        }
        None
    }

    pub(crate) fn should_discover(&mut self, scope: &CompileScope) -> bool {
        self.discovered_scope_keys.insert(scope.key())
    }

    pub(crate) fn note_scope_alias(
        &mut self,
        scope: &CompileScope,
        outcome: &SharedCompileOutcome,
    ) {
        self.by_scope_key
            .entry(scope.key())
            .or_insert_with(|| outcome.clone());
    }

    pub(crate) fn clear_runtime_maps(&mut self) {
        self.by_scope_key.clear();
        self.by_compile_cache_key.clear();
        self.by_identity.clear();
        self.by_target_identity.clear();
    }

    pub(crate) fn filter_board_discovered_scopes(
        &mut self,
        scope: &CompileScope,
        discovered: &[CompileScope],
    ) -> Vec<CompileScope> {
        let board_target = scope
            .requested_target_file
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty() && target.ends_with(".board.mei"))
            .map(str::to_string)
            .or_else(|| {
                discovered.iter().find_map(|candidate| {
                    candidate
                        .requested_target_file
                        .as_deref()
                        .map(str::trim)
                        .filter(|target| !target.is_empty() && target.ends_with(".board.mei"))
                        .map(str::to_string)
                })
            });
        let Some(board_target) = board_target else {
            return discovered.to_vec();
        };
        if self.expanded_board_targets.contains(board_target.as_str()) {
            return discovered
                .iter()
                .filter(|candidate| !is_board_export_scope(candidate, board_target.as_str()))
                .cloned()
                .collect();
        }
        self.expanded_board_targets.insert(board_target.clone());
        discovered.to_vec()
    }

    pub(crate) fn filter_compile_scope(&self, discovered: Vec<CompileScope>) -> Vec<CompileScope> {
        let Some(config) = self.compile_scope.as_ref() else {
            return discovered;
        };
        discovered
            .into_iter()
            .filter(|scope| {
                mei_lang_kernel::compile_scope_entry_allowed(
                    Some(config),
                    scope.requested_scene_id.as_deref(),
                    scope.requested_target_file.as_deref(),
                )
            })
            .collect()
    }

    pub(crate) fn filter_hot_only_discovered(
        &self,
        discovered: Vec<CompileScope>,
    ) -> Vec<CompileScope> {
        let Some(hot_scenes) = self.hot_only_scene_ids.as_ref() else {
            return discovered;
        };
        discovered
            .into_iter()
            .filter(|scope| {
                match scope
                    .requested_scene_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    None => true,
                    Some(scene) => hot_scenes.contains(scene),
                }
            })
            .collect()
    }
}

pub(crate) fn is_board_export_scope(scope: &CompileScope, board_file: &str) -> bool {
    scope
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        == Some(board_file)
        && scope
            .requested_scene_id
            .as_deref()
            .map(str::trim)
            .is_some_and(|scene| !scene.is_empty())
}

pub(crate) fn group_scopes_by_compile_target(
    scopes: Vec<CompileScope>,
) -> Vec<(CompileScope, Vec<CompileScope>)> {
    let mut groups: BTreeMap<String, (CompileScope, Vec<CompileScope>)> = BTreeMap::new();
    for scope in scopes {
        let target_key = super::mcg_target_plan::compile_target_key(&scope);
        match groups.get_mut(&target_key) {
            Some((representative, aliases)) => {
                if representative.key() == scope.key() {
                    continue;
                }
                if compile_scope_specificity(&scope) < compile_scope_specificity(representative) {
                    aliases.push(representative.clone());
                    *representative = scope;
                } else {
                    aliases.push(scope);
                }
            }
            None => {
                groups.insert(target_key, (scope, Vec::new()));
            }
        }
    }
    groups.into_values().collect()
}

pub(crate) fn session_try_reuse(
    session: &Mutex<PrebuildCompileSession>,
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
) -> Option<SharedCompileOutcome> {
    session
        .lock()
        .expect("prebuild compile session lock")
        .try_reuse(source_root, app_id, scope)
}

pub(crate) struct PersistedCompileIndexReuse {
    pub(crate) outcome: SharedCompileOutcome,
    pub(crate) discovered_scopes: Vec<CompileScope>,
    pub(crate) observed_count: usize,
}

pub(crate) fn try_reuse_persisted_compile_index(
    compile_session: &Mutex<PrebuildCompileSession>,
    diagnostics: &PrebuildDiagnostics,
    compile_index: Option<&PrebuildCompileIndex>,
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    components_root: &Path,
) -> Option<PersistedCompileIndexReuse> {
    let Some(index) = compile_index else {
        return None;
    };
    let scope_key = scope.key();
    let Some(entry) = index.entries_by_scope_key.get(&scope_key) else {
        diagnostics
            .compile_index_misses
            .fetch_add(1, Ordering::Relaxed);
        return None;
    };
    let canonical_scope = compile_scope_from_parts(
        entry.canonical_requested_scene_id.clone(),
        entry.canonical_requested_target_file.clone(),
    );
    {
        let session = compile_session
            .lock()
            .expect("prebuild compile session lock");
        if let Some(outcome) = session.by_identity.get(&entry.identity).cloned() {
            if compile_outcome_matches_scope(&canonical_scope, &outcome.compiled) {
                diagnostics
                    .compile_index_hits
                    .fetch_add(1, Ordering::Relaxed);
                diagnostics
                    .compile_artifact_loads_avoided
                    .fetch_add(1, Ordering::Relaxed);
                drop(session);
                let mut locked = compile_session
                    .lock()
                    .expect("prebuild compile session lock");
                locked.register(source_root, app_id, scope, outcome.clone());
                locked.register(source_root, app_id, &canonical_scope, outcome.clone());
                return Some(PersistedCompileIndexReuse {
                    outcome: mark_prebuild_session_reuse(&outcome),
                    discovered_scopes: Vec::new(),
                    observed_count: entry.observed_count.max(1),
                });
            }
        }
    }
    if let Some(outcome) = session_try_reuse(compile_session, source_root, app_id, &canonical_scope)
    {
        diagnostics
            .compile_index_hits
            .fetch_add(1, Ordering::Relaxed);
        compile_session
            .lock()
            .expect("prebuild compile session lock")
            .register(source_root, app_id, scope, outcome.clone());
        return Some(PersistedCompileIndexReuse {
            outcome: mark_prebuild_session_reuse(&outcome),
            discovered_scopes: Vec::new(),
            observed_count: entry.observed_count.max(1),
        });
    }
    diagnostics
        .compile_manifest_probes
        .fetch_add(1, Ordering::Relaxed);
    let manifest_identity = toolchain::probe_compiled_app_manifest_identity(
        source_root,
        app_id,
        &canonical_scope.to_world_scope(),
    );
    match manifest_identity.as_deref() {
        Some(manifest_identity) if manifest_identity == entry.identity.as_str() => {}
        Some(_) => {
            diagnostics
                .compile_manifest_stale_skips
                .fetch_add(1, Ordering::Relaxed);
            diagnostics
                .compile_index_stale_entries
                .fetch_add(1, Ordering::Relaxed);
            return None;
        }
        None => {
            diagnostics
                .compile_index_stale_entries
                .fetch_add(1, Ordering::Relaxed);
            return None;
        }
    }
    let Some(outcome) = toolchain::load_compile_artifact_only_shared(
        source_root,
        app_id,
        &canonical_scope.to_options(),
        components_root,
    ) else {
        diagnostics
            .compile_index_stale_entries
            .fetch_add(1, Ordering::Relaxed);
        return None;
    };
    let outcome = SharedCompileOutcome::from_shared(outcome);
    if compiled_scope_identity(&outcome) != entry.identity
        || !compile_outcome_matches_scope(&canonical_scope, &outcome.compiled)
    {
        diagnostics
            .compile_index_stale_entries
            .fetch_add(1, Ordering::Relaxed);
        return None;
    }
    diagnostics
        .compile_index_hits
        .fetch_add(1, Ordering::Relaxed);
    let mut locked = compile_session
        .lock()
        .expect("prebuild compile session lock");
    locked.register(source_root, app_id, &canonical_scope, outcome.clone());
    locked.register(source_root, app_id, scope, outcome.clone());
    Some(PersistedCompileIndexReuse {
        outcome: mark_prebuild_session_reuse(&outcome),
        discovered_scopes: Vec::new(),
        observed_count: entry.observed_count.max(1),
    })
}

pub(crate) fn try_reuse_compile_scope_before_load(
    session: &Mutex<PrebuildCompileSession>,
    diagnostics: &PrebuildDiagnostics,
    compile_index: Option<&PrebuildCompileIndex>,
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    components_root: &Path,
) -> Option<PersistedCompileIndexReuse> {
    let reused = session_try_reuse(session, source_root, app_id, scope);
    if let Some(reused) = reused {
        diagnostics
            .compile_preload_reuse_hits
            .fetch_add(1, Ordering::Relaxed);
        session
            .lock()
            .expect("prebuild compile session lock")
            .note_scope_alias(scope, &reused);
        return Some(PersistedCompileIndexReuse {
            outcome: reused,
            discovered_scopes: Vec::new(),
            observed_count: compile_index
                .and_then(|index| index.entries_by_scope_key.get(&scope.key()))
                .map(|entry| entry.observed_count.max(1))
                .unwrap_or(1),
        });
    }
    try_reuse_persisted_compile_index(
        session,
        diagnostics,
        compile_index,
        source_root,
        app_id,
        scope,
        components_root,
    )
}

pub(crate) fn mark_prebuild_session_reuse(outcome: &SharedCompileOutcome) -> SharedCompileOutcome {
    SharedCompileOutcome {
        compiled: Arc::clone(&outcome.compiled),
        cache_hit: true,
        artifact_cache_hit: true,
        assemble_only: outcome.assemble_only,
        compile_revision: outcome.compile_revision.clone(),
        cache_lookup_ms: 0,
        artifact_load_ms: 0,
        compile_ms: 0,
        handle_only: outcome.handle_only,
        assembly_handle: outcome.assembly_handle.clone(),
    }
}
