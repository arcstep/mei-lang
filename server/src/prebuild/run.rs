use super::prelude::*;
use super::*;

fn prebuild_timed_step<T, F>(
    source_root: &Path,
    phase: &str,
    detail: &str,
    step: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    PrebuildPhaseTracker::global().set_phase(source_root, phase, None, Some(detail));
    let started = Instant::now();
    let result = step();
    let ms = started.elapsed().as_millis();
    match &result {
        Ok(_) => prebuild_emit_notice(format!("✓ {phase} | {detail} | {ms}ms")),
        Err(error) => prebuild_emit_notice(format!(
            "✗ {phase} | {detail} | {ms}ms | {error:#}"
        )),
    }
    result
}

pub fn run_prebuild(source_root: &Path, options: &PrebuildOptions) -> Result<PrebuildReport> {
    std::env::set_var("MEI_PREBUILD_ACTIVE", "1");
    if let Ok(package_root) = crate::cli::util::resolve_package_root() {
        prebuild_timed_step(source_root, "stock_materialize", "检查/同步 platform stock", || {
            mei_lang_toolchain::ensure_workspace_stock_materialized(
                source_root,
                package_root.as_path(),
            )
            .map(|_| ())
            .map_err(Into::into)
        })?;
        prebuild_timed_step(source_root, "stock_doctor", "workspace stock 一致性检查", || {
            if let Ok(doctor) =
                mei_lang_toolchain::doctor_workspace_stock(source_root, package_root.as_path())
            {
                if !doctor.ok {
                    tracing::warn!(
                        missing_trees = doctor.missing_trees.len(),
                        orphan_paths = doctor.orphan_paths.len(),
                        manifest_drift = doctor.manifest_drift.len(),
                        missing_component_previews = doctor.missing_component_previews.len(),
                        catalog_app_drift = doctor.catalog_app_drift.len(),
                        "workspace stock doctor reported issues before prebuild (run `mei-toolchain workspace stock doctor` for details)"
                    );
                    prebuild_emit_notice(format!(
                        "stock doctor: missing_trees={} orphan_paths={} manifest_drift={} missing_previews={} catalog_drift={}",
                        doctor.missing_trees.len(),
                        doctor.orphan_paths.len(),
                        doctor.manifest_drift.len(),
                        doctor.missing_component_previews.len(),
                        doctor.catalog_app_drift.len(),
                    ));
                }
            }
            Ok(())
        })?;
    }
    let started = Instant::now();
    let manifest_path = source_root.join(WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL);
    let manifest_source = if manifest_path.is_file() {
        "runtime_manifest"
    } else {
        "workspace_config_fallback"
    };
    let Some(mut manifest) = prebuild_timed_step(
        source_root,
        "warmup_manifest",
        &format!("加载 warmup manifest ({manifest_source})"),
        || resolve_runtime_warmup_manifest(source_root),
    )?
    else {
        return Ok(PrebuildReport {
            schema_version: PREBUILD_REPORT_SCHEMA_VERSION.to_string(),
            mode: options.mode,
            scope_profile: options.scope_profile,
            clean: options.clean,
            clean_wall_ms: 0,
            total_wall_ms: started.elapsed().as_millis() as u64,
            source_root: source_root.display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            manifest_source: manifest_source.to_string(),
            ok: true,
            succeeded_apps: Vec::new(),
            failed_apps: Vec::new(),
            error_summary: Vec::new(),
            diagnostics: PrebuildDiagnosticsReport::default(),
            apps: Vec::new(),
        });
    };
    if let Some(app_filter) = options
        .app_filter
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        manifest.apps.retain(|app| app.app_id.trim() == app_filter);
        if manifest.apps.is_empty() {
            anyhow::bail!("app `{app_filter}` not found in runtime warmup manifest");
        }
    }
    prebuild_emit_notice(format!(
        "warmup plan | apps={} | scope={:?} | compileScope={}",
        manifest
            .apps
            .iter()
            .map(|app| app.app_id.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        effective_prebuild_scope_profile(options),
        manifest
            .apps
            .iter()
            .filter_map(|app| {
                app.compile_scope
                    .as_ref()
                    .filter(|scope| scope.is_active())
                    .map(|_| app.app_id.as_str())
            })
            .collect::<Vec<_>>()
            .join(", ")
    ));
    let clean_started = Instant::now();
    if options.clean {
        for app in &manifest.apps {
            clear_app_artifacts(source_root, app.app_id.as_str())?;
        }
    }
    let clean_wall_ms = if options.clean {
        clean_started.elapsed().as_millis() as u64
    } else {
        0
    };
    let effective_profile = effective_prebuild_scope_profile(options);
    let mut report = PrebuildReport {
        schema_version: PREBUILD_REPORT_SCHEMA_VERSION.to_string(),
        mode: options.mode,
        scope_profile: effective_profile,
        clean: options.clean,
        clean_wall_ms,
        total_wall_ms: 0,
        source_root: source_root.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        manifest_source: manifest_source.to_string(),
        ok: true,
        succeeded_apps: Vec::new(),
        failed_apps: Vec::new(),
        error_summary: Vec::new(),
        diagnostics: PrebuildDiagnosticsReport::default(),
        apps: Vec::new(),
    };
    if !manifest.enabled {
        report.total_wall_ms = started.elapsed().as_millis() as u64;
        return Ok(report);
    }
    if options.mode == PrebuildMode::Build
        && !options.clean
        && !options.force_rebuild
        && options.app_filter.is_none()
    {
        if let Some(fingerprint_match) = prebuild_timed_step(
            source_root,
            "fingerprint_check",
            "对比 inputs fingerprint（决定是否跳过完整 prebuild）",
            || crate::prebuild_fingerprint::try_match_prebuild_fingerprint(source_root),
        )? {
            prebuild_emit_notice(format!(
                "{} | fingerprint={} | 跳过完整 prebuild（输入未变）",
                ansi_wrap("SKIP", "1;32"),
                fingerprint_match.stored.inputs_fingerprint
            ));
            report.succeeded_apps = fingerprint_match.stored.succeeded_apps.clone();
            report.diagnostics.fingerprint_skip = true;
            report.diagnostics.inputs_fingerprint =
                Some(fingerprint_match.stored.inputs_fingerprint.clone());
            report.total_wall_ms = started.elapsed().as_millis() as u64;
            return Ok(report);
        }
    }
    prebuild_emit_notice(&format!(
        "{} | workspace={} | apps={}",
        ansi_wrap(
            &format!(
                "START {}",
                match options.mode {
                    PrebuildMode::Build => "构建",
                    PrebuildMode::Verify => "校验",
                }
            ),
            "1;34"
        ),
        source_root.display(),
        manifest
            .apps
            .iter()
            .map(|app| app.app_id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    let prebuild_app_ids: Vec<String> = manifest.apps.iter().map(|app| app.app_id.clone()).collect();
    let build_generation = Arc::new(if options.mode == PrebuildMode::Build {
        Some(prebuild_timed_step(
            source_root,
            "build_generation",
            &format!(
                "创建 build store 代次（apps: {}）",
                prebuild_app_ids.join(", ")
            ),
            || begin_prebuild_generation(source_root, &prebuild_app_ids),
        )?)
    } else {
        None
    });
    PrebuildPhaseTracker::global().set_phase(
        source_root,
        "app_prebuild",
        None,
        Some(&format!(
            "并行 prebuild {} 个 app（profile={effective_profile:?}）",
            manifest.apps.len()
        )),
    );
    let app_results = run_limited_parallel_ordered(
        manifest.apps.clone(),
        prebuild_parallelism(manifest.apps.len()),
        |app| {
            let app_id = app.app_id.clone();
            let app_root = resolve_app_root(source_root, app.app_id.as_str());
            if let Some(ref gen) = *build_generation {
                if let Some(store) = gen.store_dirs.get(&app.app_id) {
                    set_prebuild_build_root_override(app_root.as_path(), Some(store.as_path()));
                }
            }
            let result = run_prebuild_for_app(
                source_root,
                &app,
                options.mode,
                options.scope_profile,
                options.dirty_only,
                options.block_node.clone(),
                options.diagnose_on_fail,
                options.continue_from.clone(),
            );
            clear_prebuild_build_root_override();
            (app_id, result)
        },
    );
    for (app_id, result) in app_results {
        match result {
            Ok(app_report) => {
                report.succeeded_apps.push(app_id);
                report.apps.push(app_report);
            }
            Err(error) => {
                report.ok = false;
                report.failed_apps.push(app_id.clone());
                report.error_summary.push(format!("{app_id}: {error:#}"));
            }
        }
    }
    report.diagnostics = aggregate_prebuild_diagnostics(report.apps.as_slice());
    report.total_wall_ms = started.elapsed().as_millis() as u64;
    if report.ok
        && options.mode == PrebuildMode::Build
        && options.app_filter.is_none()
    {
        let total_missing = report
            .apps
            .iter()
            .map(|app| app.coverage.total_missing_artifacts)
            .sum::<usize>();
        if total_missing == 0 {
            if let Ok(fingerprint) =
                crate::prebuild_fingerprint::compute_prebuild_inputs_fingerprint(source_root)
            {
                report.diagnostics.inputs_fingerprint = Some(fingerprint.clone());
                let state = crate::prebuild_fingerprint::PersistedPrebuildState {
                    schema_version: crate::prebuild_fingerprint::PREBUILD_STATE_SCHEMA_VERSION
                        .to_string(),
                    inputs_fingerprint: fingerprint,
                    last_ok_at_ms: now_epoch_ms(),
                    last_mode: "build".to_string(),
                    last_scope_profile: match effective_profile {
                        PrebuildScopeProfile::Full => "full".to_string(),
                        PrebuildScopeProfile::HotOnly => "hot_only".to_string(),
                        PrebuildScopeProfile::BlockScoped => "block_scoped".to_string(),
                    },
                    succeeded_apps: report.succeeded_apps.clone(),
                    artifact_coverage_summary:
                        crate::prebuild_fingerprint::PrebuildArtifactCoverageSummary {
                            total_missing_artifacts: 0,
                        },
                };
                let _ = crate::prebuild_fingerprint::persist_prebuild_state(source_root, &state);
            }
        }
    }
    if report.ok && options.mode == PrebuildMode::Build {
        if let Some(ref gen) = *build_generation {
            let stock_revision = mei_lang_toolchain::workspace_stock_revision(source_root);
            finish_prebuild_generation(
                source_root,
                gen,
                &prebuild_app_ids,
                None,
                stock_revision.as_deref(),
            )?;
            prebuild_emit_notice(format!(
                "{} candidate buildId={}",
                ansi_wrap("STORE", "1;32"),
                gen.build_id
            ));
            if report.failed_apps.is_empty() {
                match mei_lang_kernel::promote_build(source_root, Some(gen.build_id.as_str())) {
                    Ok(active_id) => {
                        prebuild_emit_notice(format!(
                            "{} build/active → store/{}",
                            ansi_wrap("PROMOTE", "1;32"),
                            active_id
                        ));
                    }
                    Err(error) => {
                        tracing::warn!(
                            build_id = %gen.build_id,
                            error = %error,
                            "prebuild finished but build/active promote failed; run `mei-toolchain workspace build promote`"
                        );
                        prebuild_emit_notice(format!(
                            "{} promote failed: {error:#}",
                            ansi_wrap("WARN", "1;33")
                        ));
                    }
                }
            }
        }
    }
    Ok(report)
}

pub fn persist_prebuild_report(source_root: &Path, report: &PrebuildReport) -> Result<PathBuf> {
    let runtime_root = mei_lang_kernel::resolve_workspace_runtime_root(source_root);
    fs::create_dir_all(&runtime_root)
        .with_context(|| format!("create runtime dir {}", runtime_root.display()))?;
    let path = runtime_root.join("prebuild-last.json");
    let payload =
        serde_json::to_string_pretty(report).context("serialize prebuild report to JSON")?;
    fs::write(&path, payload).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}

/// How long `prebuild-last.json` remains valid for skipping startup prebuild.
pub(crate) const RECENT_PREBUILD_SKIP_MAX_AGE_SECS: u64 = 4 * 3600;
/// Trust a fresh CLI prebuild report without re-checking landing gate (avoid duplicate ~100s serve build).
pub(crate) const RECENT_PREBUILD_TRUST_LANDING_SECS: u64 = 30 * 60;

#[derive(Debug, Clone, Serialize)]
pub struct AppColdStartCleanDetail {
    pub app_id: String,
    pub compile_cache_entries: usize,
    pub compiled_app_artifact_files: usize,
    pub eval_artifact_files: usize,
    pub removed_build_store: bool,
    pub removed_var_store: bool,
    pub removed_legacy_prebuild: bool,
    pub removed_graph_registry: bool,
    pub removed_legacy_mei: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanPrebuildArtifactsReport {
    pub source_root: String,
    pub cleaned_apps: Vec<String>,
    pub app_details: Vec<AppColdStartCleanDetail>,
    pub workspace_artifacts_removed: Vec<String>,
    pub build_links_reset: bool,
    pub clean_wall_ms: u64,
}

fn remove_path_if_exists(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    if path.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("remove dir {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("remove file {}", path.display()))?;
    }
    Ok(true)
}

fn resolve_clean_app_ids(source_root: &Path, app_filter: Option<&str>) -> Result<Vec<String>> {
    if let Some(app_filter) = app_filter.map(str::trim).filter(|value| !value.is_empty()) {
        let known = mei_lang_kernel::discover_apps(source_root)?
            .into_iter()
            .map(|app| app.id)
            .collect::<BTreeSet<_>>();
        if !known.contains(app_filter) {
            anyhow::bail!("app `{app_filter}` not found in workspace");
        }
        return Ok(vec![app_filter.to_string()]);
    }
    if let Some(manifest) = resolve_runtime_warmup_manifest(source_root)? {
        if !manifest.apps.is_empty() {
            return Ok(manifest
                .apps
                .iter()
                .map(|app| app.app_id.clone())
                .collect());
        }
    }
    Ok(mei_lang_kernel::discover_apps(source_root)?
        .into_iter()
        .map(|app| app.id)
        .collect())
}

fn reset_workspace_build_links(source_root: &Path) -> Result<bool> {
    let mut links = mei_lang_kernel::read_links_state(source_root)?;
    if links.build.active.is_none()
        && links.build.candidate.is_none()
        && links.build.previous.is_none()
    {
        return Ok(false);
    }
    links.build.active = None;
    links.build.candidate = None;
    links.build.previous = None;
    mei_lang_kernel::write_links_state(source_root, &links)?;
    Ok(true)
}

fn clear_workspace_runtime_prebuild_artifacts(source_root: &Path) -> Result<Vec<String>> {
    let runtime_root = mei_lang_kernel::resolve_workspace_runtime_root(source_root);
    let mut removed = Vec::new();
    for name in [
        "prebuild-last.json",
        "prebuild-progress.json",
        "prebuild-state.json",
    ] {
        let path = runtime_root.join(name);
        if remove_path_if_exists(path.as_path())? {
            removed.push(format!("runtime/{name}"));
        }
    }
    let cache_root = mei_lang_kernel::resolve_workspace_cache_root(source_root);
    if cache_root.exists() {
        fs::remove_dir_all(&cache_root)
            .with_context(|| format!("remove runtime cache {}", cache_root.display()))?;
        removed.push("runtime/cache".to_string());
    }
    Ok(removed)
}

/// 一次性清理编译缓存、build/var store、graph registry 与 prebuild 状态，用于冷启动基准。
pub fn clean_workspace_prebuild_artifacts(
    source_root: &Path,
    app_filter: Option<&str>,
) -> Result<CleanPrebuildArtifactsReport> {
    let started = Instant::now();
    let app_ids = resolve_clean_app_ids(source_root, app_filter)?;
    let mut app_details = Vec::new();
    let mut cleaned_apps = Vec::new();
    for app_id in app_ids {
        let detail = clear_app_cold_start_artifacts(source_root, app_id.as_str())?;
        cleaned_apps.push(app_id);
        app_details.push(detail);
    }
    let workspace_artifacts_removed = clear_workspace_runtime_prebuild_artifacts(source_root)?;
    let build_links_reset = reset_workspace_build_links(source_root)?;
    Ok(CleanPrebuildArtifactsReport {
        source_root: source_root.display().to_string(),
        cleaned_apps,
        app_details,
        workspace_artifacts_removed,
        build_links_reset,
        clean_wall_ms: started.elapsed().as_millis() as u64,
    })
}

pub fn recent_ok_prebuild_report(
    source_root: &Path,
) -> Result<Option<(PrebuildReport, std::time::Duration)>> {
    let path =
        mei_lang_kernel::resolve_workspace_runtime_root(source_root).join("prebuild-last.json");
    if !path.is_file() {
        return Ok(None);
    }
    let age = fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .unwrap_or(std::time::Duration::MAX);
    let Some(report) = load_prebuild_report(source_root)? else {
        return Ok(None);
    };
    if !report.ok || !report.failed_apps.is_empty() {
        return Ok(None);
    }
    Ok(Some((report, age)))
}

pub fn load_prebuild_report(source_root: &Path) -> Result<Option<PrebuildReport>> {
    let path = mei_lang_kernel::resolve_workspace_runtime_root(source_root).join("prebuild-last.json");
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read prebuild report {}", path.display()))?;
    let report = serde_json::from_str::<PrebuildReport>(&raw)
        .with_context(|| format!("parse prebuild report {}", path.display()))?;
    Ok(Some(report))
}

pub(crate) fn clear_app_artifacts(source_root: &Path, app_id: &str) -> Result<()> {
    clear_app_cold_start_artifacts(source_root, app_id).map(|_| ())
}

fn clear_app_cold_start_artifacts(
    source_root: &Path,
    app_id: &str,
) -> Result<AppColdStartCleanDetail> {
    let app_root = resolve_app_root(source_root, app_id);
    let compile_cache_entries = toolchain::clear_compile_cache_for_app(source_root, app_id);
    let compiled_app_artifact_files =
        toolchain::clear_compiled_app_artifacts_for_app(source_root, app_id);
    let eval_artifact_files = mei_lang_datasets::clear_eval_artifact_store(app_root.as_path());
    let _ = mei_lang_datasets::clear_all_metric_caches();
    if data_snapshot_store_root(app_root.as_path()).exists() {
        fs::remove_dir_all(data_snapshot_store_root(app_root.as_path())).with_context(|| {
            format!(
                "remove data snapshot store {}",
                data_snapshot_store_root(app_root.as_path()).display()
            )
        })?;
    }
    let removed_build_store = remove_path_if_exists(&app_root.join("build"))?;
    let removed_var_store = remove_path_if_exists(&app_root.join("var"))?;
    let removed_legacy_prebuild = remove_path_if_exists(&app_root.join("prebuild"))?;
    let removed_legacy_mei = remove_path_if_exists(&app_root.join(".mei"))?;
    let graph_dir = mei_lang_kernel::resolve_workspace_graph_root(source_root, app_id);
    let removed_graph_registry = remove_path_if_exists(graph_dir.as_path())?;
    Ok(AppColdStartCleanDetail {
        app_id: app_id.to_string(),
        compile_cache_entries,
        compiled_app_artifact_files,
        eval_artifact_files,
        removed_build_store,
        removed_var_store,
        removed_legacy_prebuild,
        removed_graph_registry,
        removed_legacy_mei,
    })
}

pub(crate) fn scope_report_from_outcome(
    scope: &CompileScope,
    outcome: &SharedCompileOutcome,
) -> PrebuildScopeReport {
    PrebuildScopeReport {
        requested_scene_id: scope.requested_scene_id.clone(),
        requested_target_file: scope.requested_target_file.clone(),
        active_scene_id: outcome.compiled.active_scene.clone(),
        active_target_file: outcome.compiled.active_target_file.clone(),
        cache_hit: outcome.cache_hit,
        artifact_cache_hit: outcome.artifact_cache_hit,
        assemble_only: outcome.assemble_only,
        compile_revision: outcome.compile_revision.clone(),
        cache_lookup_ms: outcome.cache_lookup_ms,
        artifact_load_ms: outcome.artifact_load_ms,
        compile_ms: outcome.compile_ms,
    }
}

pub(crate) fn merge_coverage(target: &mut PrebuildCoverageReport, delta: &PrebuildCoverageReport) {
    target.compile_artifacts_planned += delta.compile_artifacts_planned;
    target.compile_artifacts_ready += delta.compile_artifacts_ready;
    target.compile_artifacts_missing += delta.compile_artifacts_missing;
    target.dataset_import_artifacts_planned += delta.dataset_import_artifacts_planned;
    target.dataset_import_artifacts_ready += delta.dataset_import_artifacts_ready;
    target.dataset_import_artifacts_missing += delta.dataset_import_artifacts_missing;
    target.metric_response_artifacts_planned += delta.metric_response_artifacts_planned;
    target.metric_response_artifacts_ready += delta.metric_response_artifacts_ready;
    target.metric_response_artifacts_built += delta.metric_response_artifacts_built;
    target.metric_response_artifacts_skipped_bundle_unchanged += delta
        .metric_response_artifacts_skipped_bundle_unchanged;
    target.metric_response_artifacts_missing += delta.metric_response_artifacts_missing;
    target.metric_dataframe_artifacts_planned += delta.metric_dataframe_artifacts_planned;
    target.metric_dataframe_artifacts_ready += delta.metric_dataframe_artifacts_ready;
    target.metric_dataframe_artifacts_built += delta.metric_dataframe_artifacts_built;
    target.metric_dataframe_artifacts_missing += delta.metric_dataframe_artifacts_missing;
    target.total_missing_artifacts += delta.total_missing_artifacts;
}

pub(crate) fn finalize_coverage_report(coverage: &mut PrebuildCoverageReport) {
    coverage.compile_artifacts_missing = coverage
        .compile_artifacts_planned
        .saturating_sub(coverage.compile_artifacts_ready);
    coverage.dataset_import_artifacts_missing = coverage
        .dataset_import_artifacts_planned
        .saturating_sub(coverage.dataset_import_artifacts_ready);
    coverage.metric_response_artifacts_missing = coverage
        .metric_response_artifacts_planned
        .saturating_sub(coverage.metric_response_artifacts_ready);
    coverage.metric_dataframe_artifacts_missing = coverage
        .metric_dataframe_artifacts_planned
        .saturating_sub(coverage.metric_dataframe_artifacts_ready);
    coverage.total_missing_artifacts = coverage
        .compile_artifacts_missing
        .saturating_add(coverage.dataset_import_artifacts_missing)
        .saturating_add(coverage.metric_response_artifacts_missing)
        .saturating_add(coverage.metric_dataframe_artifacts_missing);
}

