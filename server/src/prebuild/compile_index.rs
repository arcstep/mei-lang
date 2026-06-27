use super::prelude::*;
use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) enum PrebuildRssPhase {
    AfterCompile,
    AfterArtifacts,
    AfterWarmup,
}

#[derive(Clone)]
pub(crate) struct MetricBuildTiming {
    pub(crate) kind: &'static str,
    pub(crate) dataset: String,
    pub(crate) metric: String,
    pub(crate) scene: String,
    pub(crate) ms: u64,
}

#[derive(Default)]
pub(crate) struct PrebuildDiagnostics {
    pub(crate) metric_builds: Mutex<Vec<MetricBuildTiming>>,
    pub(crate) peak_rss_bytes: AtomicUsize,
    pub(crate) empty_binary_baseline_bytes: AtomicUsize,
    pub(crate) rss_after_compile_bytes: AtomicUsize,
    pub(crate) rss_after_artifacts_bytes: AtomicUsize,
    pub(crate) rss_after_warmup_bytes: AtomicUsize,
    pub(crate) worker_peak_rss_bytes: AtomicUsize,
    pub(crate) compile_preload_reuse_hits: AtomicUsize,
    pub(crate) compile_postload_identity_collapses: AtomicUsize,
    pub(crate) compile_target_overlay_reuse_hits: AtomicUsize,
    pub(crate) mcg_assemble_only_count: AtomicUsize,
    pub(crate) session_peak_identity_entries: AtomicUsize,
    pub(crate) hydrate_reuse_hits: AtomicU64,
    pub(crate) compile_index_hits: AtomicUsize,
    pub(crate) compile_index_misses: AtomicUsize,
    pub(crate) compile_index_stale_entries: AtomicUsize,
    pub(crate) compile_fallback_loads: AtomicUsize,
    pub(crate) compile_manifest_probes: AtomicUsize,
    pub(crate) compile_manifest_stale_skips: AtomicUsize,
    pub(crate) compile_artifact_loads_avoided: AtomicUsize,
    pub(crate) mrg_eval_skips: AtomicUsize,
    pub(crate) dataframe_eval_skips: AtomicUsize,
}

impl PrebuildDiagnostics {
    pub(crate) fn sample_memory_peak(&self) {
        sample_peak_rss_bytes(&self.peak_rss_bytes);
    }

    pub(crate) fn record_empty_binary_baseline(&self) {
        let Some(rss) = current_process_rss_bytes() else {
            return;
        };
        let _ = self.empty_binary_baseline_bytes.compare_exchange(
            0,
            rss as usize,
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
        sample_peak_rss_bytes(&self.peak_rss_bytes);
    }

    pub(crate) fn record_phase_rss(&self, phase: PrebuildRssPhase) {
        let Some(rss) = current_process_rss_bytes() else {
            return;
        };
        sample_peak_rss_bytes(&self.peak_rss_bytes);
        let target = match phase {
            PrebuildRssPhase::AfterCompile => &self.rss_after_compile_bytes,
            PrebuildRssPhase::AfterArtifacts => &self.rss_after_artifacts_bytes,
            PrebuildRssPhase::AfterWarmup => &self.rss_after_warmup_bytes,
        };
        target.store(rss as usize, Ordering::Relaxed);
    }

    pub(crate) fn note_worker_peak_rss(&self, bytes: u64) {
        let current = bytes as usize;
        let mut prev = self.worker_peak_rss_bytes.load(Ordering::Relaxed);
        while current > prev {
            match self.worker_peak_rss_bytes.compare_exchange_weak(
                prev,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(next) => prev = next,
            }
        }
    }

    pub(crate) fn note_session_identity_peak(&self, count: usize) {
        let mut prev = self.session_peak_identity_entries.load(Ordering::Relaxed);
        while count > prev {
            match self.session_peak_identity_entries.compare_exchange_weak(
                prev,
                count,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(next) => prev = next,
            }
        }
    }

    pub(crate) fn record_metric_build(
        &self,
        kind: &'static str,
        dataset: &str,
        metric: &str,
        scene: &str,
        ms: u64,
    ) {
        self.metric_builds
            .lock()
            .expect("lock prebuild diagnostics")
            .push(MetricBuildTiming {
                kind,
                dataset: short_dataset_id(dataset),
                metric: short_metric_id(metric).to_string(),
                scene: scene.to_string(),
                ms,
            });
    }
}

pub(crate) const PREBUILD_COMPILE_INDEX_SCHEMA_V6: &str = "mei-prebuild-compile-index-v6";
pub(crate) const PREBUILD_COMPILE_INDEX_SCHEMA_V7: &str = "mei-prebuild-compile-index-v7";
pub(crate) const PREBUILD_COMPILE_INDEX_SCHEMA_V8: &str = "mei-prebuild-compile-index-v8";

pub(crate) fn default_observed_count() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedCompileScopeRef {
    pub(crate) requested_scene_id: Option<String>,
    pub(crate) requested_target_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedPrebuildCompileIndexEntry {
    pub(crate) scope_key: String,
    pub(crate) requested_scene_id: Option<String>,
    pub(crate) requested_target_file: Option<String>,
    pub(crate) compile_cache_key: String,
    pub(crate) canonical_scope_key: String,
    pub(crate) canonical_requested_scene_id: Option<String>,
    pub(crate) canonical_requested_target_file: Option<String>,
    pub(crate) canonical_compile_cache_key: String,
    pub(crate) identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) scene_payload_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) assembly_view_revision: Option<String>,
    #[serde(default)]
    pub(crate) discovered_scopes: Vec<PersistedCompileScopeRef>,
    #[serde(default = "default_observed_count")]
    pub(crate) observed_count: usize,
    pub(crate) generated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedPrebuildCompileIndex {
    pub(crate) schema_version: String,
    pub(crate) generated_at_ms: u64,
    pub(crate) entries: Vec<PersistedPrebuildCompileIndexEntry>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PrebuildCompileIndex {
    pub(crate) entries_by_scope_key: BTreeMap<String, PersistedPrebuildCompileIndexEntry>,
}

pub(crate) fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn prebuild_compile_index_path(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_build_root(app_root)
        .join("prebuild")
        .join("compile-index.json")
}

pub(crate) fn write_prebuild_compile_index(app_root: &Path, index: &PrebuildCompileIndex) -> Result<()> {
    let path = prebuild_compile_index_path(app_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create prebuild compile index dir {}", parent.display()))?;
    }
    let persisted = PersistedPrebuildCompileIndex {
        schema_version: PREBUILD_COMPILE_INDEX_SCHEMA_V8.to_string(),
        generated_at_ms: now_epoch_ms(),
        entries: index.entries_by_scope_key.values().cloned().collect(),
    };
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, serde_json::to_string_pretty(&persisted)?)
        .with_context(|| format!("write prebuild compile index {}", tmp_path.display()))?;
    fs::rename(&tmp_path, &path)
        .with_context(|| format!("rename prebuild compile index {}", path.display()))?;
    Ok(())
}

pub(crate) fn load_prebuild_compile_index(app_root: &Path) -> Result<Option<PrebuildCompileIndex>> {
    let path = prebuild_compile_index_path(app_root);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read prebuild compile index {}", path.display()))?;
    let persisted = serde_json::from_str::<PersistedPrebuildCompileIndex>(&raw)
        .with_context(|| format!("parse prebuild compile index {}", path.display()))?;
    if persisted.schema_version != PREBUILD_COMPILE_INDEX_SCHEMA_V6
        && persisted.schema_version != PREBUILD_COMPILE_INDEX_SCHEMA_V7
        && persisted.schema_version != PREBUILD_COMPILE_INDEX_SCHEMA_V8
    {
        return Ok(None);
    }
    Ok(Some(PrebuildCompileIndex {
        entries_by_scope_key: persisted
            .entries
            .into_iter()
            .map(|entry| (entry.scope_key.clone(), entry))
            .collect(),
    }))
}

pub(crate) fn compile_scope_from_parts(
    requested_scene_id: Option<String>,
    requested_target_file: Option<String>,
) -> CompileScope {
    CompileScope {
        requested_scene_id,
        requested_target_file,
    }
    .canonicalized()
}

pub(crate) fn build_prebuild_compile_index(
    source_root: &Path,
    app_id: &str,
    prepared_outcomes: &[PreparedCompileOutcome],
    compile_reports: &[PrebuildScopeReport],
) -> PrebuildCompileIndex {
    let mut observed_counts = BTreeMap::<String, usize>::new();
    for report in compile_reports {
        let scope_key = compile_scope_key_from_parts(
            report.requested_scene_id.as_deref(),
            report.requested_target_file.as_deref(),
        );
        *observed_counts.entry(scope_key).or_insert(0) += 1;
    }
    let mut best_scope_by_identity = BTreeMap::<String, &PreparedCompileOutcome>::new();
    for prepared in prepared_outcomes {
        let identity = compiled_artifact_identity(&prepared.outcome);
        match best_scope_by_identity.get(&identity) {
            Some(existing) => {
                if compile_scope_specificity(&prepared.scope)
                    > compile_scope_specificity(&existing.scope)
                {
                    best_scope_by_identity.insert(identity, prepared);
                }
            }
            None => {
                best_scope_by_identity.insert(identity, prepared);
            }
        }
    }
    let mcg_registry = if crate::graph::feature::graph_registry_dedup_enabled() {
        Some(crate::graph::mcg::registry::McgRegistryWriter::load(source_root, app_id))
    } else {
        None
    };
    let mut entries_by_scope_key = BTreeMap::new();
    for prepared in prepared_outcomes {
        let scope = &prepared.scope;
        let outcome = &prepared.outcome;
        let artifact_identity = compiled_artifact_identity(outcome);
        let Some(canonical) = best_scope_by_identity.get(&artifact_identity) else {
            continue;
        };
        let identity = compiled_scope_identity(outcome);
        let scene_payload_revision = scope
            .canonicalized()
            .requested_target_file
            .as_deref()
            .and_then(|target| {
                mcg_registry
                    .as_ref()
                    .and_then(|registry| registry.node_revision("scene_payload", target))
            });
        let assembly_view_revision = mcg_registry.as_ref().and_then(|registry| {
            registry.node_revision(
                "assembly_view",
                &assembly_view_index_key(
                    canonical.scope.canonicalized().requested_scene_id.as_deref(),
                    canonical.scope.canonicalized().requested_target_file.as_deref(),
                    outcome.compile_revision.as_str(),
                ),
            )
        });
        let entry = PersistedPrebuildCompileIndexEntry {
            scope_key: scope.key(),
            requested_scene_id: scope.canonicalized().requested_scene_id,
            requested_target_file: scope.canonicalized().requested_target_file,
            compile_cache_key: toolchain::compile_cache_key(
                source_root,
                app_id,
                &scope.to_options(),
            ),
            canonical_scope_key: canonical.scope.key(),
            canonical_requested_scene_id: canonical.scope.canonicalized().requested_scene_id,
            canonical_requested_target_file: canonical.scope.canonicalized().requested_target_file,
            canonical_compile_cache_key: toolchain::compile_cache_key(
                source_root,
                app_id,
                &canonical.scope.to_options(),
            ),
            identity,
            scene_payload_revision,
            assembly_view_revision,
            discovered_scopes: discovered_compile_scopes(scope, &outcome.compiled)
                .into_iter()
                .map(|scope| PersistedCompileScopeRef {
                    requested_scene_id: scope.requested_scene_id,
                    requested_target_file: scope.requested_target_file,
                })
                .collect(),
            observed_count: observed_counts.get(&scope.key()).copied().unwrap_or(1),
            generated_at_ms: now_epoch_ms(),
        };
        entries_by_scope_key.insert(entry.scope_key.clone(), entry);
    }
    PrebuildCompileIndex {
        entries_by_scope_key,
    }
}

pub(crate) fn patch_prebuild_compile_index_entry(
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    outcome: &SharedCompileOutcome,
) -> Result<()> {
    let app_root = resolve_app_root(source_root, app_id);
    let mut index = load_prebuild_compile_index(app_root.as_path())?.unwrap_or_default();
    let mcg_registry = if crate::graph::feature::graph_registry_dedup_enabled() {
        Some(crate::graph::mcg::registry::McgRegistryWriter::load(source_root, app_id))
    } else {
        None
    };
    let identity = compiled_scope_identity(outcome);
    let scene_payload_revision = scope
        .canonicalized()
        .requested_target_file
        .as_deref()
        .and_then(|target| {
            mcg_registry
                .as_ref()
                .and_then(|registry| registry.node_revision("scene_payload", target))
        });
    let entry = PersistedPrebuildCompileIndexEntry {
        scope_key: scope.key(),
        requested_scene_id: scope.canonicalized().requested_scene_id,
        requested_target_file: scope.canonicalized().requested_target_file,
        compile_cache_key: toolchain::compile_cache_key(source_root, app_id, &scope.to_options()),
        canonical_scope_key: scope.key(),
        canonical_requested_scene_id: scope.canonicalized().requested_scene_id,
        canonical_requested_target_file: scope.canonicalized().requested_target_file,
        canonical_compile_cache_key: toolchain::compile_cache_key(
            source_root,
            app_id,
            &scope.to_options(),
        ),
        identity,
        scene_payload_revision,
        assembly_view_revision: None,
        discovered_scopes: discovered_compile_scopes(scope, &outcome.compiled)
            .into_iter()
            .map(|scope| PersistedCompileScopeRef {
                requested_scene_id: scope.requested_scene_id,
                requested_target_file: scope.requested_target_file,
            })
            .collect(),
        observed_count: 1,
        generated_at_ms: now_epoch_ms(),
    };
    index
        .entries_by_scope_key
        .insert(entry.scope_key.clone(), entry);
    write_prebuild_compile_index(app_root.as_path(), &index)
}

pub(crate) fn assembly_view_index_key(
    requested_scene_id: Option<&str>,
    requested_target_file: Option<&str>,
    compile_revision: &str,
) -> String {
    let scene = requested_scene_id.unwrap_or("default").trim();
    let target = requested_target_file.unwrap_or("").trim();
    if target.is_empty() {
        format!("{scene}@{compile_revision}")
    } else {
        format!("{scene}:{target}@{compile_revision}")
    }
}

pub(crate) fn scope_assembled_outcome(
    source_root: &Path,
    app_id: &str,
    base: &SharedCompileOutcome,
    scope: &CompileScope,
    diagnostics: Option<&PrebuildDiagnostics>,
) -> SharedCompileOutcome {
    if compile_outcome_matches_scope(scope, &base.compiled) {
        return base.clone();
    }
    let canonical = scope.canonicalized();
    if crate::graph::feature::graph_registry_dedup_enabled() {
        if let Some(target) = canonical
            .requested_target_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Some((mut compiled, compile_revision)) =
                crate::graph::try_assemble_scope_from_scene_payload(
                    source_root,
                    app_id,
                    canonical.requested_scene_id.as_deref(),
                    target,
                )
            {
                let _ = crate::graph::hydrate_compiled_for_prebuild_eval(
                    source_root,
                    app_id,
                    &mut compiled,
                    &[],
                    &[],
                );
                if let Some(diag) = diagnostics {
                    diag.mcg_assemble_only_count
                        .fetch_add(1, Ordering::Relaxed);
                }
                return SharedCompileOutcome {
                    compiled: Arc::new(compiled),
                    cache_hit: true,
                    artifact_cache_hit: false,
                    assemble_only: true,
                    compile_revision,
                    cache_lookup_ms: 0,
                    artifact_load_ms: 0,
                    compile_ms: 0,
                    handle_only: false,
                };
            }
        }
    }
    let scene = canonical.requested_scene_id.as_deref();
    let target = canonical
        .requested_target_file
        .as_deref()
        .filter(|value| !value.is_empty());
    if crate::graph::feature::graph_registry_dedup_enabled() {
        if let Some(target_file) = target {
            if let Some((mut compiled, compile_revision)) =
                crate::graph::try_assemble_scope_from_scene_payload(
                    source_root,
                    app_id,
                    scene,
                    target_file,
                )
            {
                let _ = crate::graph::hydrate_compiled_for_prebuild_eval(
                    source_root,
                    app_id,
                    &mut compiled,
                    &[],
                    &[],
                );
                if let Some(diag) = diagnostics {
                    diag.mcg_assemble_only_count
                        .fetch_add(1, Ordering::Relaxed);
                    diag.compile_target_overlay_reuse_hits
                        .fetch_add(1, Ordering::Relaxed);
                }
                return SharedCompileOutcome {
                    compiled: Arc::new(compiled),
                    cache_hit: true,
                    artifact_cache_hit: false,
                    assemble_only: true,
                    compile_revision,
                    cache_lookup_ms: 0,
                    artifact_load_ms: 0,
                    compile_ms: 0,
                    handle_only: false,
                };
            }
        }
    }
    let compiled = match Arc::try_unwrap(Arc::clone(&base.compiled)) {
        Ok(mut owned) => {
            crate::graph::mcg::assemble::apply_scope_to_compiled_app(
                &mut owned,
                scene,
                target,
            );
            owned
        }
        Err(shared) => crate::graph::mcg::assemble::assemble_scope_view(
            (*shared).clone(),
            scene,
            target,
        ),
    };
    let mut hydrated = compiled;
    let _ = crate::graph::hydrate_compiled_for_prebuild_eval(source_root, app_id, &mut hydrated, &[], &[]);
    if let Some(diag) = diagnostics {
        diag.mcg_assemble_only_count
            .fetch_add(1, Ordering::Relaxed);
        diag.compile_target_overlay_reuse_hits
            .fetch_add(1, Ordering::Relaxed);
    }
    SharedCompileOutcome {
        compiled: Arc::new(hydrated),
        cache_hit: true,
        artifact_cache_hit: base.artifact_cache_hit,
        assemble_only: true,
        compile_revision: base.compile_revision.clone(),
        cache_lookup_ms: base.cache_lookup_ms,
        artifact_load_ms: base.artifact_load_ms,
        compile_ms: 0,
        handle_only: false,
    }
}

pub(crate) fn compile_active_identity(report: &PrebuildScopeReport) -> String {
    format!(
        "{}|{}",
        report.active_scene_id.as_deref().unwrap_or(""),
        report.active_target_file
    )
}

pub(crate) fn disk_usage_report(summary: DirSizeSummary) -> PrebuildDiskUsageReport {
    PrebuildDiskUsageReport {
        files: summary.files,
        bytes: summary.bytes,
    }
}
