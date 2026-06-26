use super::prelude::*;
use super::*;

pub fn run_prebuild(source_root: &Path, options: &PrebuildOptions) -> Result<PrebuildReport> {
    let _progress_session = PrebuildProgressSession::begin();
    std::env::set_var("MEI_PREBUILD_ACTIVE", "1");
    if let Ok(package_root) = crate::cli::util::resolve_package_root() {
        let _ = mei_lang_toolchain::ensure_workspace_stock_materialized(
            source_root,
            package_root.as_path(),
        );
        if let Ok(doctor) =
            mei_lang_toolchain::doctor_workspace_stock(source_root, package_root.as_path())
        {
            if !doctor.ok {
                tracing::warn!(
                    missing_trees = ?doctor.missing_trees,
                    orphan_paths = ?doctor.orphan_paths,
                    manifest_drift = ?doctor.manifest_drift,
                    missing_component_previews = ?doctor.missing_component_previews,
                    catalog_app_drift = ?doctor.catalog_app_drift,
                    "workspace stock doctor reported issues before prebuild"
                );
            }
        }
    }
    let started = Instant::now();
    let manifest_path = source_root.join(WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL);
    let manifest_source = if manifest_path.is_file() {
        "runtime_manifest"
    } else {
        "workspace_config_fallback"
    };
    let Some(mut manifest) = resolve_runtime_warmup_manifest(source_root)? else {
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
    let mut report = PrebuildReport {
        schema_version: PREBUILD_REPORT_SCHEMA_VERSION.to_string(),
        mode: options.mode,
        scope_profile: options.scope_profile,
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
        if let Some(fingerprint_match) =
            crate::prebuild_fingerprint::try_match_prebuild_fingerprint(source_root)?
        {
            prebuild_emit_progress(format!(
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
    prebuild_emit_progress(&format!(
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
        Some(begin_prebuild_generation(source_root, &prebuild_app_ids)?)
    } else {
        None
    });
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
            let result =
                run_prebuild_for_app(source_root, &app, options.mode, options.scope_profile);
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
        && !options.clean
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
                    last_scope_profile: match options.scope_profile {
                        PrebuildScopeProfile::Full => "full".to_string(),
                        PrebuildScopeProfile::HotOnly => "hot_only".to_string(),
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
            prebuild_emit_progress(format!(
                "{} candidate buildId={}",
                ansi_wrap("STORE", "1;32"),
                gen.build_id
            ));
        }
    }
    Ok(report)
}

pub(crate) fn clear_app_artifacts(source_root: &Path, app_id: &str) -> Result<()> {
    let app_root = resolve_app_root(source_root, app_id);
    let _ = toolchain::clear_compile_cache_for_app(source_root, app_id);
    let _ = toolchain::clear_compiled_app_artifacts_for_app(source_root, app_id);
    let _ = mei_lang_datasets::clear_eval_artifact_store(app_root.as_path());
    let _ = mei_lang_datasets::clear_all_metric_caches();
    if data_snapshot_store_root(app_root.as_path()).exists() {
        fs::remove_dir_all(data_snapshot_store_root(app_root.as_path())).with_context(|| {
            format!(
                "remove data snapshot store {}",
                data_snapshot_store_root(app_root.as_path()).display()
            )
        })?;
    }
    Ok(())
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

