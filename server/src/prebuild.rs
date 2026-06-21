use std::collections::{BTreeMap, BTreeSet};
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::sync::{Condvar, Mutex};
use std::thread;
use std::time::Instant;

use anyhow::{Context, Result};
use mei_lang_datasets::{
    collect_all_query_options, evaluate_runtime_metrics_from_plan,
    load_metric_dataframe_result_artifact, load_metric_response_result_artifact,
    locate_runtime_metric_resource, metric_dataframe_result_cache_key,
    metric_request_revision_fingerprint_for_compiled, metric_response_cache_scope_key,
    metric_scope_cache_key, plan_access_metric_eval_for_ids, query_metric_dataframe, query_state_from_request,
    runtime_metric_workset, store_cached_metric_response, store_metric_response_result_artifact,
    store_metric_dataframe_result_artifact, DatasetQueryOptions, DatasetQueryResult,
    LoadedMetricResponseArtifact, RuntimeMetricEvalMode,
};
use mei_lang_kernel::{
    data_snapshot_import_manifest_path, data_snapshot_store_root, resolve_app_root,
    resolve_data_snapshot_import_entry, resolve_runtime_warmup_manifest, CompiledApp,
    CompileOptions, DatasetView, LoadedResource, RuntimeWarmupApp, RuntimeWarmupDatasetRequest,
    WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL,
};
use mei_lang_toolchain::{
    self as toolchain, CompileWithCacheOutcome, PublishDataSnapshotsReport,
};
use serde_json::Value;
use serde::Serialize;

const PREBUILD_REPORT_SCHEMA_VERSION: &str = "mei-prebuild-report-v1";
const PREBUILD_MAX_PARALLELISM: usize = 8;

fn is_script_target(path: &str) -> bool {
    path.ends_with(".mei")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrebuildMode {
    Build,
    Verify,
}

#[derive(Debug, Clone)]
pub struct PrebuildOptions {
    pub app_filter: Option<String>,
    pub mode: PrebuildMode,
    pub clean: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildScopeReport {
    pub requested_scene_id: Option<String>,
    pub requested_target_file: Option<String>,
    pub active_scene_id: Option<String>,
    pub active_target_file: String,
    pub cache_hit: bool,
    pub artifact_cache_hit: bool,
    pub compile_revision: String,
    pub cache_lookup_ms: u64,
    pub artifact_load_ms: u64,
    pub compile_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildTimingReport {
    pub total_wall_ms: u64,
    pub compile_scopes_ms: u64,
    pub data_snapshots_ms: u64,
    pub scope_artifacts_ms: u64,
    pub warmup_requests_ms: u64,
    pub max_parallelism: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildCoverageReport {
    pub dataset_import_artifacts_ready: usize,
    pub metric_response_artifacts_ready: usize,
    pub metric_response_artifacts_built: usize,
    pub metric_dataframe_artifacts_ready: usize,
    pub metric_dataframe_artifacts_built: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildAppReport {
    pub app_id: String,
    pub compile_scopes: Vec<PrebuildScopeReport>,
    pub coverage: PrebuildCoverageReport,
    pub timings: PrebuildTimingReport,
    pub data_snapshots: Option<PublishDataSnapshotsReport>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildReport {
    pub schema_version: String,
    pub mode: PrebuildMode,
    pub clean: bool,
    pub clean_wall_ms: u64,
    pub total_wall_ms: u64,
    pub source_root: String,
    pub manifest_path: String,
    pub manifest_source: String,
    pub ok: bool,
    pub succeeded_apps: Vec<String>,
    pub failed_apps: Vec<String>,
    pub error_summary: Vec<String>,
    pub apps: Vec<PrebuildAppReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildScopeSummary {
    pub requested_scene_id: Option<String>,
    pub requested_target_file: Option<String>,
    pub active_scene_id: Option<String>,
    pub active_target_file: String,
    pub cache_hit: bool,
    pub artifact_cache_hit: bool,
    pub cache_lookup_ms: u64,
    pub artifact_load_ms: u64,
    pub compile_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildAppSummary {
    pub app_id: String,
    pub compile_scopes: Vec<PrebuildScopeSummary>,
    pub coverage: PrebuildCoverageReport,
    pub timings: PrebuildTimingReport,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildReportSummary {
    pub schema_version: String,
    pub mode: PrebuildMode,
    pub clean: bool,
    pub clean_wall_ms: u64,
    pub total_wall_ms: u64,
    pub source_root: String,
    pub manifest_path: String,
    pub manifest_source: String,
    pub ok: bool,
    pub succeeded_apps: Vec<String>,
    pub failed_apps: Vec<String>,
    pub error_summary: Vec<String>,
    pub apps: Vec<PrebuildAppSummary>,
}

impl PrebuildReport {
    pub fn summary(&self) -> PrebuildReportSummary {
        PrebuildReportSummary {
            schema_version: self.schema_version.clone(),
            mode: self.mode,
            clean: self.clean,
            clean_wall_ms: self.clean_wall_ms,
            total_wall_ms: self.total_wall_ms,
            source_root: self.source_root.clone(),
            manifest_path: self.manifest_path.clone(),
            manifest_source: self.manifest_source.clone(),
            ok: self.ok,
            succeeded_apps: self.succeeded_apps.clone(),
            failed_apps: self.failed_apps.clone(),
            error_summary: self.error_summary.clone(),
            apps: self
                .apps
                .iter()
                .map(|app| PrebuildAppSummary {
                    app_id: app.app_id.clone(),
                    compile_scopes: app
                        .compile_scopes
                        .iter()
                        .map(|scope| PrebuildScopeSummary {
                            requested_scene_id: scope.requested_scene_id.clone(),
                            requested_target_file: scope.requested_target_file.clone(),
                            active_scene_id: scope.active_scene_id.clone(),
                            active_target_file: scope.active_target_file.clone(),
                            cache_hit: scope.cache_hit,
                            artifact_cache_hit: scope.artifact_cache_hit,
                            cache_lookup_ms: scope.cache_lookup_ms,
                            artifact_load_ms: scope.artifact_load_ms,
                            compile_ms: scope.compile_ms,
                        })
                        .collect(),
                    coverage: app.coverage.clone(),
                    timings: app.timings.clone(),
                    warnings: app.warnings.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct CompileScope {
    requested_scene_id: Option<String>,
    requested_target_file: Option<String>,
}

#[derive(Debug, Clone)]
struct AggregatedWarmupRequest {
    scope: CompileScope,
    dataset_id: String,
    metric_ids: Vec<String>,
}

impl CompileScope {
    fn default_scope() -> Self {
        Self {
            requested_scene_id: None,
            requested_target_file: None,
        }
    }

    fn to_options(&self) -> CompileOptions {
        let canonical = self.canonicalized();
        CompileOptions {
            scene: canonical.requested_scene_id,
            preview_target: canonical.requested_target_file,
        }
    }

    fn key(&self) -> String {
        let canonical = self.canonicalized();
        format!(
            "{}|{}",
            canonical.requested_scene_id.as_deref().unwrap_or(""),
            canonical.requested_target_file.as_deref().unwrap_or("")
        )
    }

    fn canonicalized(&self) -> Self {
        let requested_scene_id = self
            .requested_scene_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let requested_target_file = self
            .requested_target_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .filter(|target| is_script_target(target))
            .map(str::to_string);
        Self {
            requested_scene_id,
            requested_target_file,
        }
    }
}

#[derive(Default)]
struct CoverageState {
    metric_response_jobs: ArtifactSingleflightState,
    metric_dataframe_jobs: ArtifactSingleflightState,
    metric_response_exact: Mutex<BTreeMap<String, LoadedMetricResponseArtifact>>,
    metric_response_shared: Mutex<BTreeMap<String, LoadedMetricResponseArtifact>>,
    metric_dataframe_exact: Mutex<BTreeMap<String, DatasetQueryResult>>,
    metric_dataframe_shared: Mutex<BTreeMap<String, DatasetQueryResult>>,
}

#[derive(Default)]
struct ArtifactSingleflightState {
    state: Mutex<ArtifactSingleflightInner>,
    ready: Condvar,
}

#[derive(Default)]
struct ArtifactSingleflightInner {
    inflight: BTreeSet<String>,
    completed: BTreeSet<String>,
}

enum ArtifactReservation {
    Reserved,
    Completed,
}

impl ArtifactSingleflightState {
    fn wait_or_reserve(&self, key: &str) -> ArtifactReservation {
        let mut state = self.state.lock().expect("lock prebuild singleflight");
        loop {
            if state.completed.contains(key) {
                return ArtifactReservation::Completed;
            }
            if state.inflight.insert(key.to_string()) {
                return ArtifactReservation::Reserved;
            }
            state = self.ready.wait(state).expect("wait prebuild singleflight");
        }
    }

    fn finish(&self, key: &str, success: bool) {
        let mut state = self.state.lock().expect("lock prebuild singleflight");
        state.inflight.remove(key);
        if success {
            state.completed.insert(key.to_string());
        }
        self.ready.notify_all();
    }
}

impl CoverageState {
    fn metric_response_exact(&self, key: &str) -> Option<LoadedMetricResponseArtifact> {
        self.metric_response_exact
            .lock()
            .expect("lock prebuild response exact cache")
            .get(key)
            .cloned()
    }

    fn metric_response_shared(&self, key: &str) -> Option<LoadedMetricResponseArtifact> {
        self.metric_response_shared
            .lock()
            .expect("lock prebuild response shared cache")
            .get(key)
            .cloned()
    }

    fn store_metric_response_exact(&self, key: &str, artifact: &LoadedMetricResponseArtifact) {
        self.metric_response_exact
            .lock()
            .expect("lock prebuild response exact cache")
            .insert(key.to_string(), artifact.clone());
    }

    fn store_metric_response_shared(&self, key: &str, artifact: &LoadedMetricResponseArtifact) {
        self.metric_response_shared
            .lock()
            .expect("lock prebuild response shared cache")
            .insert(key.to_string(), artifact.clone());
    }

    fn metric_dataframe_exact(&self, key: &str) -> Option<DatasetQueryResult> {
        self.metric_dataframe_exact
            .lock()
            .expect("lock prebuild dataframe exact cache")
            .get(key)
            .cloned()
    }

    fn metric_dataframe_shared(&self, key: &str) -> Option<DatasetQueryResult> {
        self.metric_dataframe_shared
            .lock()
            .expect("lock prebuild dataframe shared cache")
            .get(key)
            .cloned()
    }

    fn store_metric_dataframe_exact(&self, key: &str, result: &DatasetQueryResult) {
        self.metric_dataframe_exact
            .lock()
            .expect("lock prebuild dataframe exact cache")
            .insert(key.to_string(), result.clone());
    }

    fn store_metric_dataframe_shared(&self, key: &str, result: &DatasetQueryResult) {
        self.metric_dataframe_shared
            .lock()
            .expect("lock prebuild dataframe shared cache")
            .insert(key.to_string(), result.clone());
    }
}

pub fn run_prebuild(source_root: &Path, options: &PrebuildOptions) -> Result<PrebuildReport> {
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
        apps: Vec::new(),
    };
    if !manifest.enabled {
        report.total_wall_ms = started.elapsed().as_millis() as u64;
        return Ok(report);
    }
    let app_results = run_limited_parallel_ordered(
        manifest.apps.clone(),
        prebuild_parallelism(manifest.apps.len()),
        |app| {
            let app_id = app.app_id.clone();
            let result = run_prebuild_for_app(source_root, &app, options.mode);
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
                report
                    .error_summary
                    .push(format!("{app_id}: {error}"));
            }
        }
    }
    report.total_wall_ms = started.elapsed().as_millis() as u64;
    Ok(report)
}

fn clear_app_artifacts(source_root: &Path, app_id: &str) -> Result<()> {
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

fn scope_report_from_outcome(
    scope: &CompileScope,
    outcome: &CompileWithCacheOutcome,
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

fn merge_coverage(target: &mut PrebuildCoverageReport, delta: &PrebuildCoverageReport) {
    target.dataset_import_artifacts_ready += delta.dataset_import_artifacts_ready;
    target.metric_response_artifacts_ready += delta.metric_response_artifacts_ready;
    target.metric_response_artifacts_built += delta.metric_response_artifacts_built;
    target.metric_dataframe_artifacts_ready += delta.metric_dataframe_artifacts_ready;
    target.metric_dataframe_artifacts_built += delta.metric_dataframe_artifacts_built;
}

fn run_prebuild_for_app(
    source_root: &Path,
    app: &RuntimeWarmupApp,
    mode: PrebuildMode,
) -> Result<PrebuildAppReport> {
    let app_started = Instant::now();
    let components_root = toolchain::resolve_components_root(source_root);
    let app_root = resolve_app_root(source_root, app.app_id.as_str());
    let warmup_requests = aggregate_warmup_requests(app);
    let max_parallelism = prebuild_parallelism(
        compile_scopes_for_app(app)
            .len()
            .max(warmup_requests.len())
            .max(1),
    );
    let default_scope = CompileScope::default_scope();
    let compile_started = Instant::now();
    let default_outcome = ensure_compile_scope(
        source_root,
        app.app_id.as_str(),
        &default_scope,
        mode,
        components_root.as_path(),
    )?;
    let mut scopes = compile_scopes_for_app(app);
    scopes.retain(|scope| scope.key() != default_scope.key());
    let mut pending = VecDeque::from(scopes);
    let mut seen_scopes = pending.iter().map(CompileScope::key).collect::<BTreeSet<_>>();
    let mut compile_reports = vec![scope_report_from_outcome(&default_scope, &default_outcome)];
    let mut prepared_outcomes = vec![(default_scope.clone(), default_outcome)];
    let mut warnings = Vec::new();
    while !pending.is_empty() {
        let batch = pending.drain(..).collect::<Vec<_>>();
        let batch_results = run_limited_parallel_ordered(batch, max_parallelism, |scope| {
            let result = ensure_compile_scope(
                source_root,
                app.app_id.as_str(),
                &scope,
                mode,
                components_root.as_path(),
            );
            (scope, result)
        });
        for (scope, result) in batch_results {
            let outcome = match result {
                Ok(outcome) => outcome,
                Err(error) => {
                    if mode == PrebuildMode::Verify {
                        return Err(error);
                    }
                    warnings.push(format!(
                        "compile scope scene=`{}` target=`{}` failed: {error}",
                        scope.requested_scene_id.as_deref().unwrap_or(""),
                        scope.requested_target_file.as_deref().unwrap_or(""),
                    ));
                    continue;
                }
            };
            compile_reports.push(scope_report_from_outcome(&scope, &outcome));
            for discovered in discovered_compile_scopes(&scope, &outcome.compiled) {
                if seen_scopes.insert(discovered.key()) {
                    pending.push_back(discovered);
                }
            }
            prepared_outcomes.push((scope, outcome));
        }
    }
    let compile_scopes_ms = compile_started.elapsed().as_millis() as u64;
    let required_xlsx_sources = collect_required_xlsx_sources(
        app,
        prepared_outcomes
            .iter()
            .map(|(_, outcome)| &outcome.compiled),
    );
    let snapshot_started = Instant::now();
    let data_snapshots = match mode {
        PrebuildMode::Build => Some(publish_required_data_snapshots(
            source_root,
            app.app_id.as_str(),
            required_xlsx_sources.iter().cloned().collect(),
        )?),
        PrebuildMode::Verify => None,
    };
    let data_snapshots_ms = snapshot_started.elapsed().as_millis() as u64;
    verify_required_xlsx_sources(app_root.as_path(), &required_xlsx_sources)?;
    let mut coverage = PrebuildCoverageReport::default();
    coverage.dataset_import_artifacts_ready = required_xlsx_sources.len();
    let coverage_state = CoverageState::default();
    let artifact_outcomes = unique_prepared_outcomes_for_artifacts(&prepared_outcomes);
    let artifact_outcomes_for_warmup = artifact_outcomes.clone();
    let scope_artifacts_started = Instant::now();
    let scope_results = run_limited_parallel_ordered(
        artifact_outcomes,
        max_parallelism,
        |(scope, outcome)| {
            let mut local_coverage = PrebuildCoverageReport::default();
            let matching_requests = matching_warmup_requests_for_outcome(&warmup_requests, outcome);
            let result = ensure_scope_artifacts(
                app.app_id.as_str(),
                app_root.as_path(),
                scope,
                outcome,
                matching_requests.as_slice(),
                mode,
                &mut local_coverage,
                &coverage_state,
            );
            (scope, result, local_coverage)
        },
    );
    for (scope, result, local_coverage) in scope_results {
        if let Err(error) = result {
            if mode == PrebuildMode::Verify {
                return Err(error);
            }
            warnings.push(format!(
                "scope artifacts scene=`{}` target=`{}` failed: {error}",
                scope.requested_scene_id.as_deref().unwrap_or(""),
                scope.requested_target_file.as_deref().unwrap_or(""),
            ));
        } else {
            merge_coverage(&mut coverage, &local_coverage);
        }
    }
    let scope_artifacts_ms = scope_artifacts_started.elapsed().as_millis() as u64;
    let warmup_started = Instant::now();
    let warmup_results = run_limited_parallel_ordered(
        warmup_requests
            .iter()
            .filter(|request| {
                !artifact_outcomes_for_warmup
                    .iter()
                    .any(|(_, outcome)| warmup_request_matches_outcome(request, outcome))
            })
            .collect::<Vec<_>>(),
        max_parallelism,
        |request| {
            let scope = request.scope.clone();
            let mut local_coverage = PrebuildCoverageReport::default();
            let result = ensure_warmup_dataset_request_artifacts(
                source_root,
                app.app_id.as_str(),
                app_root.as_path(),
                request,
                mode,
                components_root.as_path(),
                &mut local_coverage,
                &coverage_state,
            );
            (scope, request.dataset_id.clone(), result, local_coverage)
        },
    );
    for (scope, dataset_id, result, local_coverage) in warmup_results {
        let scope = CompileScope {
            requested_scene_id: scope.requested_scene_id.clone(),
            requested_target_file: scope.requested_target_file.clone(),
        };
        if let Err(error) = result {
            if mode == PrebuildMode::Verify {
                return Err(error);
            }
            warnings.push(format!(
                "warmup request scene=`{}` target=`{}` dataset=`{}` failed: {error}",
                scope.requested_scene_id.as_deref().unwrap_or(""),
                scope.requested_target_file.as_deref().unwrap_or(""),
                dataset_id,
            ));
        } else {
            merge_coverage(&mut coverage, &local_coverage);
        }
    }
    let warmup_requests_ms = warmup_started.elapsed().as_millis() as u64;
    Ok(PrebuildAppReport {
        app_id: app.app_id.clone(),
        compile_scopes: compile_reports,
        coverage,
        timings: PrebuildTimingReport {
            total_wall_ms: app_started.elapsed().as_millis() as u64,
            compile_scopes_ms,
            data_snapshots_ms,
            scope_artifacts_ms,
            warmup_requests_ms,
            max_parallelism,
        },
        data_snapshots,
        warnings,
    })
}

fn compile_scopes_for_app(app: &RuntimeWarmupApp) -> Vec<CompileScope> {
    let mut scopes = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push_scope = |scope: CompileScope| {
        let scope = scope.canonicalized();
        if seen.insert(scope.key()) {
            scopes.push(scope);
        }
    };
    push_scope(CompileScope::default_scope());
    let scene_ids = explicit_scene_ids(app);
    let focus_targets = all_focus_targets(app);
    for scene_id in &scene_ids {
        push_scope(CompileScope {
            requested_scene_id: Some(scene_id.clone()),
            requested_target_file: None,
        });
    }
    for focus in &focus_targets {
        push_scope(CompileScope {
            requested_scene_id: None,
            requested_target_file: Some(focus.clone()),
        });
    }
    for scene_id in &scene_ids {
        for focus in &focus_targets {
            push_scope(CompileScope {
                requested_scene_id: Some(scene_id.clone()),
                requested_target_file: Some(focus.clone()),
            });
        }
    }
    for request in &app.datasets {
        push_scope(CompileScope {
            requested_scene_id: request.scene_id.clone(),
            requested_target_file: request.focus.clone(),
        });
    }
    scopes
}

fn explicit_scene_ids(app: &RuntimeWarmupApp) -> Vec<String> {
    let mut scene_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for scene_id in app
        .default_scene
        .iter()
        .chain(app.hot_scenes.iter())
        .chain(app.scenes.iter())
    {
        let scene_id = scene_id.trim();
        if scene_id.is_empty() || !seen.insert(scene_id.to_string()) {
            continue;
        }
        scene_ids.push(scene_id.to_string());
    }
    scene_ids
}

fn explicit_focus_targets(app: &RuntimeWarmupApp) -> Vec<String> {
    let mut targets = Vec::new();
    let mut seen = BTreeSet::new();
    for focus in &app.focuses {
        let focus = focus.trim();
        if focus.is_empty() || !seen.insert(focus.to_string()) {
            continue;
        }
        targets.push(focus.to_string());
    }
    targets
}

fn focus_targets_from_warmup_datasets(app: &RuntimeWarmupApp) -> Vec<String> {
    let mut targets = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push = |target: &str| {
        let target = target.trim();
        if target.is_empty() || !is_script_target(target) || !seen.insert(target.to_string()) {
            return;
        }
        targets.push(target.to_string());
    };
    for request in &app.datasets {
        if let Some(focus) = request.focus.as_deref() {
            push(focus);
        }
        for segment in request.dataset_id.split("::") {
            if segment.starts_with("scenes/") && segment.ends_with(".mei") {
                push(segment);
            }
        }
    }
    targets
}

fn all_focus_targets(app: &RuntimeWarmupApp) -> Vec<String> {
    let mut targets = explicit_focus_targets(app);
    let mut seen = targets.iter().cloned().collect::<BTreeSet<_>>();
    for focus in focus_targets_from_warmup_datasets(app) {
        if seen.insert(focus.clone()) {
            targets.push(focus);
        }
    }
    targets
}

fn collect_scene_file_refs(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(scene_file)) = map.get("scene_file") {
                if is_script_target(scene_file) {
                    out.insert(scene_file.clone());
                }
            }
            for child in map.values() {
                collect_scene_file_refs(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_scene_file_refs(item, out);
            }
        }
        _ => {}
    }
}

fn discover_overlay_preview_targets(compiled: &mei_lang_kernel::CompiledApp) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut push = |target: &str| {
        let target = target.trim();
        if target.is_empty() || !is_script_target(target) {
            return;
        }
        seen.insert(target.to_string());
    };
    for target in compiled.scene_local_nav_by_target.keys() {
        push(target.as_str());
    }
    for assembly in compiled.scene_projection_assembly_by_id.values() {
        if let Some(target) = assembly.get("target_file").and_then(Value::as_str) {
            push(target);
        }
    }
    if let Some(contract) = compiled.scene_contract.as_ref() {
        if let Ok(value) = serde_json::to_value(contract) {
            collect_scene_file_refs(&value, &mut seen);
        }
    }
    let mut targets = seen.into_iter().collect::<Vec<_>>();
    targets.sort();
    targets
}

fn aggregate_warmup_requests(app: &RuntimeWarmupApp) -> Vec<AggregatedWarmupRequest> {
    let mut aggregated = BTreeMap::<String, AggregatedWarmupRequest>::new();
    for request in &app.datasets {
        let scope = CompileScope {
            requested_scene_id: request.scene_id.clone(),
            requested_target_file: request.focus.clone(),
        }
        .canonicalized();
        let metric_ids = requested_metric_ids(request);
        let request_all_metrics = metric_ids.is_empty();
        let key = format!("{}|{}", scope.key(), request.dataset_id.trim());
        if let Some(entry) = aggregated.get_mut(&key) {
            if request_all_metrics || entry.metric_ids.is_empty() {
                entry.metric_ids.clear();
            } else {
                entry.metric_ids.extend(metric_ids);
                entry.metric_ids.sort();
                entry.metric_ids.dedup();
            }
            continue;
        }
        aggregated.insert(
            key,
            AggregatedWarmupRequest {
                scope,
                dataset_id: request.dataset_id.trim().to_string(),
                metric_ids,
            },
        );
    }
    aggregated.into_values().collect()
}

fn warmup_request_matches_outcome(
    request: &AggregatedWarmupRequest,
    outcome: &CompileWithCacheOutcome,
) -> bool {
    let req_scope = request.scope.canonicalized();
    let active_scene = outcome
        .compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    let active_target = outcome.compiled.active_target_file.as_str();
    if let Some(req_scene) = req_scope
        .requested_scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if req_scene != active_scene {
            return false;
        }
    }
    if let Some(req_target) = req_scope
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if req_target != active_target {
            return false;
        }
    }
    true
}

fn matching_warmup_requests_for_outcome<'a>(
    requests: &'a [AggregatedWarmupRequest],
    outcome: &CompileWithCacheOutcome,
) -> Vec<&'a AggregatedWarmupRequest> {
    requests
        .iter()
        .filter(|request| warmup_request_matches_outcome(request, outcome))
        .collect()
}

fn compile_scope_specificity(scope: &CompileScope) -> u8 {
    let canonical = scope.canonicalized();
    let mut score = 0u8;
    if canonical.requested_scene_id.is_some() {
        score = score.saturating_add(2);
    }
    if canonical.requested_target_file.is_some() {
        score = score.saturating_add(1);
    }
    score
}

fn discovered_compile_scopes(
    scope: &CompileScope,
    compiled: &mei_lang_kernel::CompiledApp,
) -> Vec<CompileScope> {
    let mut scopes = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push_scope = |candidate: CompileScope| {
        let candidate = candidate.canonicalized();
        if seen.insert(candidate.key()) {
            scopes.push(candidate);
        }
    };
    let active_scene = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|scene_id| !scene_id.is_empty())
        .map(str::to_string);
    let active_target = compiled.active_target_file.trim();
    if let Some(active_scene) = active_scene.clone() {
        push_scope(CompileScope {
            requested_scene_id: Some(active_scene.clone()),
            requested_target_file: None,
        });
        let target = scope
            .requested_target_file
            .as_deref()
            .map(str::trim)
            .filter(|target| !target.is_empty())
            .unwrap_or(active_target);
        if !target.is_empty() {
            push_scope(CompileScope {
                requested_scene_id: Some(active_scene.clone()),
                requested_target_file: Some(target.to_string()),
            });
        }
        for overlay_target in discover_overlay_preview_targets(compiled) {
            if overlay_target == active_target {
                continue;
            }
            push_scope(CompileScope {
                requested_scene_id: Some(active_scene.clone()),
                requested_target_file: Some(overlay_target),
            });
        }
        for entry in compiled.build_board_index.boards.values() {
            push_scope(CompileScope {
                requested_scene_id: Some(entry.scene_id.clone()),
                requested_target_file: Some(entry.board_file.clone()),
            });
        }
    }
    let native_target = scope
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .unwrap_or(active_target);
    if is_script_target(native_target) {
        for route in &compiled.scene_routes {
            if route.target_file != native_target {
                continue;
            }
            let scene_id = route.scene_id.trim();
            if scene_id.is_empty() || scene_id == "home" {
                continue;
            }
            push_scope(CompileScope {
                requested_scene_id: Some(scene_id.to_string()),
                requested_target_file: Some(native_target.to_string()),
            });
        }
    }
    scopes
}

fn compiled_scope_identity(outcome: &CompileWithCacheOutcome) -> String {
    format!(
        "{}|{}|{}",
        outcome.compiled.active_scene.as_deref().unwrap_or_default(),
        outcome.compiled.active_target_file,
        outcome.compile_revision
    )
}

fn unique_prepared_outcomes_for_artifacts<'a>(
    prepared_outcomes: &'a [(CompileScope, CompileWithCacheOutcome)],
) -> Vec<&'a (CompileScope, CompileWithCacheOutcome)> {
    let mut best_by_identity = BTreeMap::<String, &'a (CompileScope, CompileWithCacheOutcome)>::new();
    for prepared in prepared_outcomes {
        let identity = compiled_scope_identity(&prepared.1);
        match best_by_identity.get(&identity) {
            Some(existing) => {
                if compile_scope_specificity(&prepared.0) > compile_scope_specificity(&existing.0)
                {
                    best_by_identity.insert(identity, prepared);
                }
            }
            None => {
                best_by_identity.insert(identity, prepared);
            }
        }
    }
    best_by_identity.into_values().collect()
}

fn ensure_compile_scope(
    source_root: &Path,
    app_id: &str,
    scope: &CompileScope,
    mode: PrebuildMode,
    components_root: &Path,
) -> Result<CompileWithCacheOutcome> {
    let options = scope.to_options();
    match mode {
        PrebuildMode::Build => toolchain::compile_app_with_cache(
            source_root,
            app_id,
            options,
            components_root,
        )
        .map_err(|failure| failure.error)
        .with_context(|| {
            format!(
                "compile scope scene=`{}` target=`{}` for app `{app_id}`",
                scope.requested_scene_id.as_deref().unwrap_or(""),
                scope.requested_target_file.as_deref().unwrap_or("")
            )
        }),
        PrebuildMode::Verify => toolchain::load_compile_artifact_only(
            source_root,
            app_id,
            &options,
            components_root,
        )
        .ok_or_else(|| {
            anyhow::anyhow!(
                "missing compile artifact for app `{app_id}` scene=`{}` target=`{}`",
                scope.requested_scene_id.as_deref().unwrap_or(""),
                scope.requested_target_file.as_deref().unwrap_or("")
            )
        }),
    }
}

fn collect_required_xlsx_sources<'a>(
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

fn publish_required_data_snapshots(
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

fn verify_required_xlsx_sources(
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

fn ensure_scope_artifacts(
    app_id: &str,
    app_root: &Path,
    scope: &CompileScope,
    outcome: &CompileWithCacheOutcome,
    requests: &[&AggregatedWarmupRequest],
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    for request in requests {
        ensure_request_artifacts_for_compiled(
            app_id,
            app_root,
            outcome,
            request.dataset_id.as_str(),
            request.metric_ids.as_slice(),
            mode,
            coverage,
            state,
        )?;
    }
    if scope.key() == CompileScope::default_scope().key() {
        ensure_root_world_metrics_artifact(app_id, app_root, outcome, mode, coverage, state)?;
    }
    ensure_discovered_scope_metric_artifacts(
        app_id,
        app_root,
        scope,
        outcome,
        mode,
        coverage,
        state,
    )?;
    if should_auto_discover_scope_artifacts(scope, outcome) {
        ensure_scope_world_metrics_artifacts(app_id, app_root, outcome, mode, coverage, state)?;
    }
    Ok(())
}

fn ensure_scope_world_metrics_artifacts(
    app_id: &str,
    app_root: &Path,
    outcome: &CompileWithCacheOutcome,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    for resource in &outcome.compiled.resources {
        if !is_world_metrics_resource(resource.id.as_str()) {
            continue;
        }
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if !dataset.has_runtime_metric_defs() {
            continue;
        }
        ensure_metric_response_artifact_for_request(
            app_id,
            app_root,
            outcome,
            resource.id.as_str(),
            &[],
            mode,
            coverage,
            state,
        )?;
        let mut dataframe_metrics = dataframe_metric_ids(dataset);
        for metric_id in dataset.runtime_metric_defs.keys() {
            if metric_id.contains("__scalar_rowset__") {
                dataframe_metrics.push(metric_id.clone());
            }
        }
        dataframe_metrics.sort();
        dataframe_metrics.dedup();
        for metric_id in dataframe_metrics {
            for page_size in widget_dataframe_page_sizes() {
                ensure_metric_dataframe_artifact(
                    app_root,
                    outcome,
                    resource,
                    metric_id.as_str(),
                    *page_size,
                    mode,
                    coverage,
                    state,
                )?;
            }
        }
    }
    Ok(())
}

fn ensure_warmup_dataset_request_artifacts(
    source_root: &Path,
    app_id: &str,
    app_root: &Path,
    request: &AggregatedWarmupRequest,
    mode: PrebuildMode,
    components_root: &Path,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let scope = request.scope.clone();
    let outcome = ensure_compile_scope(
        source_root,
        app_id,
        &scope,
        mode,
        components_root,
    )?;
    ensure_request_artifacts_for_compiled(
        app_id,
        app_root,
        &outcome,
        request.dataset_id.as_str(),
        request.metric_ids.as_slice(),
        mode,
        coverage,
        state,
    )
}

fn ensure_root_world_metrics_artifact(
    app_id: &str,
    app_root: &Path,
    outcome: &CompileWithCacheOutcome,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let Some(root_world_metrics) = outcome.compiled.resources.iter().find(|resource| {
        resource.id == "__world_metrics__"
            && resource
                .dataset
                .as_ref()
                .is_some_and(|dataset| dataset.has_runtime_metric_defs())
    }) else {
        return Ok(());
    };
    let Some(dataset) = root_world_metrics.dataset.as_ref() else {
        return Ok(());
    };
    let metric_ids = response_metric_ids(&outcome.compiled, dataset);
    if metric_ids.is_empty() {
        return Ok(());
    }
    ensure_metric_response_artifact_for_request(
        app_id,
        app_root,
        outcome,
        "__world_metrics__",
        metric_ids.as_slice(),
        mode,
        coverage,
        state,
    )
}

fn ensure_request_artifacts_for_compiled(
    app_id: &str,
    app_root: &Path,
    outcome: &CompileWithCacheOutcome,
    dataset_selector: &str,
    metric_ids: &[String],
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let resource = mei_lang_kernel::locate_dataset_resource(&outcome.compiled, dataset_selector)
        .with_context(|| format!("locate warmup dataset `{dataset_selector}`"))?;
    let dataset = resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", resource.id))?;
    if metric_ids.is_empty() {
        let response_metric_ids = response_metric_ids(&outcome.compiled, dataset);
        if !response_metric_ids.is_empty() {
            let metric_groups =
                group_metric_ids_by_owner(&outcome.compiled, resource.id.as_str(), &response_metric_ids)?;
            for metric_ids in metric_groups.into_values() {
                ensure_metric_response_artifact_for_request(
                    app_id,
                    app_root,
                    outcome,
                    resource.id.as_str(),
                    metric_ids.as_slice(),
                    mode,
                    coverage,
                    state,
                )?;
            }
        } else {
            ensure_metric_response_artifact_for_request(
                app_id,
                app_root,
                outcome,
                resource.id.as_str(),
                metric_ids,
                mode,
                coverage,
                state,
            )?;
        }
    } else {
        ensure_metric_response_artifact_for_request(
            app_id,
            app_root,
            outcome,
            resource.id.as_str(),
            metric_ids,
            mode,
            coverage,
            state,
        )?;
    }
    if is_world_metrics_resource(resource.id.as_str()) {
        let mut dataframe_metrics = metric_ids
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        for metric_id in dataset.runtime_metric_defs.keys() {
            if metric_id.contains("__scalar_rowset__") {
                dataframe_metrics.push(metric_id.clone());
            }
        }
        dataframe_metrics.extend(dataframe_metric_ids(dataset));
        dataframe_metrics.sort();
        dataframe_metrics.dedup();
        for metric_id in dataframe_metrics {
            for page_size in widget_dataframe_page_sizes() {
                ensure_metric_dataframe_artifact(
                    app_root,
                    outcome,
                    resource,
                    metric_id.as_str(),
                    *page_size,
                    mode,
                    coverage,
                    state,
                )?;
            }
        }
        return Ok(());
    }
    for metric_id in dataframe_metric_ids(dataset) {
        for page_size in widget_dataframe_page_sizes() {
            ensure_metric_dataframe_artifact(
                app_root,
                outcome,
                &resource,
                metric_id.as_str(),
                *page_size,
                mode,
                coverage,
                state,
            )?;
        }
    }
    Ok(())
}

fn is_world_metrics_resource(resource_id: &str) -> bool {
    let resource_id = resource_id.trim();
    resource_id == "__world_metrics__" || resource_id.starts_with("__world_metrics__::")
}

fn response_metric_ids(compiled: &mei_lang_kernel::CompiledApp, dataset: &DatasetView) -> Vec<String> {
    let mut ids = BTreeSet::new();
    ids.extend(
        dataset
            .runtime_analysis_contracts
            .keys()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(str::to_string),
    );
    ids.extend(
        dataset
            .runtime_metric_defs
            .keys()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(str::to_string),
    );
    if ids.is_empty() {
        ids.extend(
            compiled
                .world_metrics
                .keys()
                .map(|id| id.trim())
                .filter(|id| !id.is_empty())
                .map(str::to_string),
        );
    }
    ids.into_iter().collect()
}

fn group_metric_ids_by_owner(
    compiled: &mei_lang_kernel::CompiledApp,
    dataset_id: &str,
    metric_ids: &[String],
) -> Result<BTreeMap<String, Vec<String>>> {
    let mut groups = BTreeMap::<String, Vec<String>>::new();
    for metric_id in metric_ids {
        let (owner, _) = locate_runtime_metric_resource(compiled, dataset_id, metric_id)?;
        groups
            .entry(owner.id.clone())
            .or_default()
            .push(metric_id.clone());
    }
    Ok(groups)
}

fn dataframe_metric_ids(dataset: &DatasetView) -> Vec<String> {
    let mut ids = BTreeSet::new();
    for contract in dataset.runtime_analysis_contracts.values() {
        collect_contract_metric_ids(contract, &mut ids);
    }
    ids.into_iter().collect()
}

fn collect_contract_metric_ids(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let is_metric_key = matches!(
                    key.as_str(),
                    "metric_id" | "table_metric_id" | "detail_table_metric_id" | "drilldown_table_metric_id"
                );
                if is_metric_key {
                    if let Some(metric_id) = child
                        .as_str()
                        .map(str::trim)
                        .filter(|metric_id| !metric_id.is_empty())
                    {
                        out.insert(metric_id.to_string());
                    }
                }
                collect_contract_metric_ids(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_contract_metric_ids(item, out);
            }
        }
        _ => {}
    }
}

fn requested_metric_ids(request: &RuntimeWarmupDatasetRequest) -> Vec<String> {
    let mut metric_ids = request
        .metric_ids
        .iter()
        .map(|metric_id| metric_id.trim())
        .filter(|metric_id| !metric_id.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if let Some(metric_id) = request
        .metric_id
        .as_deref()
        .map(str::trim)
        .filter(|metric_id| !metric_id.is_empty())
    {
        metric_ids.push(metric_id.to_string());
    }
    metric_ids.sort();
    metric_ids.dedup();
    metric_ids
}

fn empty_query_state() -> mei_lang_kernel::QueryState {
    let filters = BTreeMap::<String, String>::new();
    let normalized_filters = mei_lang_datasets::normalize_query_filters(&filters);
    let normalized_search = mei_lang_datasets::normalize_query_search(None);
    query_state_from_request(&normalized_filters, normalized_search.as_deref(), None)
}

fn artifact_scene_context(compiled: &CompiledApp) -> (String, Option<String>) {
    let scene_id = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|scene_id| !scene_id.is_empty())
        .map(str::to_string)
        .or_else(|| {
            compiled
                .scene_routes
                .iter()
                .find(|route| route.target_file == compiled.active_target_file)
                .map(|route| route.scene_id.clone())
        })
        .unwrap_or_else(|| "default".to_string());
    let scene_path = compiled.active_target_file.trim().to_string();
    let scene_path = if scene_path.is_empty() {
        None
    } else {
        Some(scene_path)
    };
    (scene_id, scene_path)
}

fn should_auto_discover_scope_artifacts(scope: &CompileScope, outcome: &CompileWithCacheOutcome) -> bool {
    let canonical = scope.canonicalized();
    if canonical
        .requested_scene_id
        .as_deref()
        .map(str::trim)
        .is_some_and(|scene_id| scene_id == "home")
    {
        return true;
    }
    let (scene_id, _) = artifact_scene_context(&outcome.compiled);
    canonical
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .is_some_and(|target| !target.is_empty())
        && scene_id != "home"
}

fn widget_dataframe_page_sizes() -> &'static [usize] {
    &[0, 8, 16, 20, 64]
}

fn widget_dataframe_query_options(page_size: usize) -> DatasetQueryOptions {
    DatasetQueryOptions {
        page: 1,
        page_size,
        collect_all: false,
        ..Default::default()
    }
}

fn equivalent_dataframe_metric_ids(
    compiled: &mei_lang_kernel::CompiledApp,
    resource_id: &str,
    resolved_metric_id: &str,
) -> Vec<String> {
    let mut ids = BTreeSet::new();
    ids.insert(resolved_metric_id.trim().to_string());
    let Ok((owner, resolved)) =
        locate_runtime_metric_resource(compiled, resource_id, resolved_metric_id)
    else {
        return ids.into_iter().collect();
    };
    let Some(dataset) = owner.dataset.as_ref() else {
        return ids.into_iter().collect();
    };
    for def_key in dataset.runtime_metric_defs.keys() {
        if let Ok((_, candidate)) = locate_runtime_metric_resource(compiled, resource_id, def_key) {
            if candidate == resolved {
                ids.insert(def_key.trim().to_string());
            }
        }
    }
    ids.into_iter().collect()
}

fn dataset_metric_identity_key(dataset: &DatasetView) -> String {
    let mut metric_keys = dataset
        .runtime_metric_defs
        .keys()
        .map(|metric_id| metric_id.as_str())
        .collect::<Vec<_>>();
    metric_keys.sort_unstable();
    let source_path = dataset.source.path.trim().replace('\\', "/");
    format!(
        "{}|{}",
        source_path,
        metric_keys.join(",")
    )
}

fn ensure_discovered_scope_metric_artifacts(
    app_id: &str,
    app_root: &Path,
    scope: &CompileScope,
    outcome: &CompileWithCacheOutcome,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    if !should_auto_discover_scope_artifacts(scope, outcome) {
        return Ok(());
    }
    let mut seen_identities = BTreeSet::<String>::new();
    for resource in &outcome.compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if !dataset.has_runtime_metric_defs() || is_world_metrics_resource(resource.id.as_str()) {
            continue;
        }
        let identity = dataset_metric_identity_key(dataset);
        if !seen_identities.insert(identity) {
            continue;
        }
        let response_metric_ids = response_metric_ids(&outcome.compiled, dataset);
        if !response_metric_ids.is_empty() {
            ensure_metric_response_artifact_for_request(
                app_id,
                app_root,
                outcome,
                resource.id.as_str(),
                &[],
                mode,
                coverage,
                state,
            )?;
        }
        let mut dataframe_metric_ids = dataframe_metric_ids(dataset);
        for metric_id in dataset.runtime_metric_defs.keys() {
            if metric_id.contains("__scalar_rowset__") {
                dataframe_metric_ids.push(metric_id.clone());
            }
        }
        dataframe_metric_ids.sort();
        dataframe_metric_ids.dedup();
        for metric_id in dataframe_metric_ids {
            for page_size in widget_dataframe_page_sizes() {
                ensure_metric_dataframe_artifact(
                    app_root,
                    outcome,
                    resource,
                    metric_id.as_str(),
                    *page_size,
                    mode,
                    coverage,
                    state,
                )?;
            }
        }
    }
    Ok(())
}

fn ensure_metric_response_artifact_for_request(
    app_id: &str,
    app_root: &Path,
    outcome: &CompileWithCacheOutcome,
    dataset_selector: &str,
    metric_ids: &[String],
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let request_all_metrics = metric_ids.is_empty();
    let access_plan = plan_access_metric_eval_for_ids(&outcome.compiled, dataset_selector, metric_ids)
        .with_context(|| format!("plan metric response artifact for dataset `{dataset_selector}`"))?;
    let runtime_workset = runtime_metric_workset(
        &access_plan.owner.id,
        &access_plan.request_metric_ids,
        access_plan.owner_dataset,
    );
    let covered_metric_ids = runtime_workset
        .eval_metric_ids
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root,
        &outcome.compiled,
        &access_plan.owner.id,
        &runtime_workset.defs_for_hydrate,
    );
    let query_state = empty_query_state();
    let query = collect_all_query_options(&query_state);
    let (scene_id, scene_path) = artifact_scene_context(&outcome.compiled);
    let response_cache_key = metric_response_cache_scope_key(
        app_id,
        scene_id.as_str(),
        scene_path.as_deref(),
        &access_plan.owner.id,
        &query,
        &outcome.compile_revision,
        &dependency_revision_key,
        &[],
    );
    let shared_cache_key = prebuild_metric_response_shared_key(
        app_id,
        &access_plan.owner.id,
        &query,
        &dependency_revision_key,
    );
    if let Some(artifact) = state.metric_response_exact(&response_cache_key) {
        let artifact_covers_request =
            metric_response_artifact_covers_request(&artifact, &covered_metric_ids, request_all_metrics);
        if artifact_covers_request {
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                access_plan.owner,
                &artifact,
                &query,
                &runtime_workset.defs_for_hydrate,
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
    }
    if let Some(artifact) = state.metric_response_shared(&shared_cache_key) {
        let artifact_covers_request =
            metric_response_artifact_covers_request(&artifact, &covered_metric_ids, request_all_metrics);
        if artifact_covers_request {
            materialize_metric_response_alias(app_root, &response_cache_key, &artifact)?;
            state.store_metric_response_exact(&response_cache_key, &artifact);
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                access_plan.owner,
                &artifact,
                &query,
                &runtime_workset.defs_for_hydrate,
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
    }
    if let Some((artifact, _)) =
        load_metric_response_result_artifact(app_root, &response_cache_key)?
    {
        let artifact_covers_request =
            metric_response_artifact_covers_request(&artifact, &covered_metric_ids, request_all_metrics);
        if artifact_covers_request {
            state.store_metric_response_exact(&response_cache_key, &artifact);
            state.store_metric_response_shared(&shared_cache_key, &artifact);
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                access_plan.owner,
                &artifact,
                &query,
                &runtime_workset.defs_for_hydrate,
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
        if mode == PrebuildMode::Verify {
            anyhow::bail!(
                "metric response artifact for dataset `{}` scope scene=`{}` target=`{}` does not cover all declared metrics",
                dataset_selector,
                scene_id,
                scene_path.as_deref().unwrap_or("")
            );
        }
    } else if mode == PrebuildMode::Verify {
        anyhow::bail!(
            "missing metric response artifact for dataset `{}` scope scene=`{}` target=`{}`",
            dataset_selector,
            scene_id,
            scene_path.as_deref().unwrap_or("")
        );
    }
    if let Some((artifact, _)) = load_metric_response_result_artifact(app_root, &shared_cache_key)? {
        let artifact_covers_request =
            metric_response_artifact_covers_request(&artifact, &covered_metric_ids, request_all_metrics);
        if artifact_covers_request {
            materialize_metric_response_alias(app_root, &response_cache_key, &artifact)?;
            state.store_metric_response_shared(&shared_cache_key, &artifact);
            state.store_metric_response_exact(&response_cache_key, &artifact);
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                access_plan.owner,
                &artifact,
                &query,
                &runtime_workset.defs_for_hydrate,
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
    }
    let reservation = state.metric_response_jobs.wait_or_reserve(&shared_cache_key);
    if let ArtifactReservation::Completed = reservation {
        if let Some(artifact) = state.metric_response_shared(&shared_cache_key) {
            let artifact_covers_request =
                metric_response_artifact_covers_request(&artifact, &covered_metric_ids, request_all_metrics);
            if artifact_covers_request {
                materialize_metric_response_alias(app_root, &response_cache_key, &artifact)?;
                state.store_metric_response_exact(&response_cache_key, &artifact);
                materialize_metric_response_sibling_aliases(
                    app_id,
                    app_root,
                    outcome,
                    access_plan.owner,
                    &artifact,
                    &query,
                    &runtime_workset.defs_for_hydrate,
                    state,
                )?;
                coverage.metric_response_artifacts_ready += 1;
                return Ok(());
            }
        }
    }
    let eval_outcome = evaluate_runtime_metrics_from_plan(
        &outcome.compiled,
        app_root,
        &access_plan,
        scene_id.as_str(),
        scene_path.as_deref(),
        &query_state,
        &[],
        RuntimeMetricEvalMode::WithDag,
        request_all_metrics,
    )
    .with_context(|| format!("build metric response artifact for dataset `{dataset_selector}`"));
    let eval_outcome = match eval_outcome {
        Ok(eval_outcome) => eval_outcome,
        Err(error) => {
            state.metric_response_jobs.finish(&shared_cache_key, false);
            return Err(error);
        }
    };
    let declared_metric_ids = access_plan
        .owner_dataset
        .runtime_metric_defs
        .keys()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let complete = request_all_metrics
        && !declared_metric_ids.is_empty()
        && declared_metric_ids
            .iter()
            .all(|metric_id| covered_metric_ids.contains(metric_id));
    let built_artifact = LoadedMetricResponseArtifact {
        total_rows: eval_outcome.total_rows,
        metrics_map: eval_outcome.metrics_map.clone(),
        covered_metric_ids: covered_metric_ids.clone(),
        complete,
    };
    let store_result = (|| -> Result<()> {
        store_cached_metric_response(
            shared_cache_key.clone(),
            eval_outcome.total_rows,
            &eval_outcome.metrics_map,
            &covered_metric_ids,
            complete,
        );
        store_metric_response_result_artifact(
            app_root,
            &shared_cache_key,
            eval_outcome.total_rows,
            &eval_outcome.metrics_map,
            &covered_metric_ids,
            complete,
        )?;
        materialize_metric_response_alias_parts(
            app_root,
            &response_cache_key,
            eval_outcome.total_rows,
            &eval_outcome.metrics_map,
            &covered_metric_ids,
            complete,
        )?;
        Ok(())
    })();
    state
        .metric_response_jobs
        .finish(&shared_cache_key, store_result.is_ok());
    if store_result.is_ok() {
        state.store_metric_response_shared(&shared_cache_key, &built_artifact);
        state.store_metric_response_exact(&response_cache_key, &built_artifact);
    }
    store_result?;
    materialize_metric_response_sibling_aliases(
        app_id,
        app_root,
        outcome,
        access_plan.owner,
        &built_artifact,
        &query,
        &runtime_workset.defs_for_hydrate,
        state,
    )?;
    coverage.metric_response_artifacts_built += 1;
    Ok(())
}

fn dataframe_scope_metric_token(
    compiled: &mei_lang_kernel::CompiledApp,
    resource_id: &str,
    metric_selector: &str,
) -> Option<String> {
    let (_, resolved_metric_id) =
        locate_runtime_metric_resource(compiled, resource_id, metric_selector).ok()?;
    Some(metric_scope_cache_key(std::slice::from_ref(&resolved_metric_id)))
}

fn ensure_metric_dataframe_artifact(
    app_root: &Path,
    outcome: &CompileWithCacheOutcome,
    resource: &LoadedResource,
    metric_id: &str,
    page_size: usize,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let Ok((owner_resource, resolved_metric_id)) =
        locate_runtime_metric_resource(&outcome.compiled, resource.id.as_str(), metric_id)
    else {
        return Ok(());
    };
    let owner_dataset = owner_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
    let runtime_workset = runtime_metric_workset(
        &owner_resource.id,
        &[resolved_metric_id.clone()],
        owner_dataset,
    );
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root,
        &outcome.compiled,
        &owner_resource.id,
        if owner_dataset.runtime_metric_defs.is_empty() {
            &runtime_workset.defs_for_hydrate
        } else {
            &owner_dataset.runtime_metric_defs
        },
    );
    let query_options = widget_dataframe_query_options(page_size);
    let (scene_id, scene_path) = artifact_scene_context(&outcome.compiled);
    let scope_metric_token = dataframe_scope_metric_token(
        &outcome.compiled,
        resource.id.as_str(),
        resolved_metric_id.as_str(),
    )
    .unwrap_or_else(|| metric_scope_cache_key(std::slice::from_ref(&resolved_metric_id)));
    let response_cache_key = metric_dataframe_result_cache_key(
        app_root,
        Some(scene_id.as_str()),
        scene_path.as_deref(),
        owner_resource.id.as_str(),
        scope_metric_token.as_str(),
        &query_options,
        &outcome.compile_revision,
        &dependency_revision_key,
        &[],
    );
    let shared_cache_key = prebuild_metric_dataframe_shared_key(
        owner_resource.id.as_str(),
        resolved_metric_id.as_str(),
        &query_options,
        &dependency_revision_key,
    );
    if state.metric_dataframe_exact(&response_cache_key).is_some() {
        if let Some(result) = state.metric_dataframe_exact(&response_cache_key) {
            materialize_metric_dataframe_sibling_aliases(
                app_root,
                outcome,
                owner_resource,
                resolved_metric_id.as_str(),
                &query_options,
                &runtime_workset.defs_for_hydrate,
                &result,
                state,
            )?;
            materialize_metric_dataframe_metric_aliases(
                app_root,
                outcome,
                resource.id.as_str(),
                resolved_metric_id.as_str(),
                &query_options,
                &runtime_workset.defs_for_hydrate,
                &result,
                state,
            )?;
        }
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if let Some((result, _)) = load_metric_dataframe_result_artifact(app_root, &response_cache_key)? {
        state.store_metric_dataframe_exact(&response_cache_key, &result);
        state.store_metric_dataframe_shared(&shared_cache_key, &result);
        materialize_metric_dataframe_sibling_aliases(
            app_root,
            outcome,
            owner_resource,
            resolved_metric_id.as_str(),
            &query_options,
            &runtime_workset.defs_for_hydrate,
            &result,
            state,
        )?;
        materialize_metric_dataframe_metric_aliases(
            app_root,
            outcome,
            resource.id.as_str(),
            resolved_metric_id.as_str(),
            &query_options,
            &runtime_workset.defs_for_hydrate,
            &result,
            state,
        )?;
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if mode == PrebuildMode::Verify {
        anyhow::bail!(
            "missing metric dataframe artifact for dataset `{}` metric `{}` scope scene=`{}` target=`{}`",
            resource.id,
            resolved_metric_id,
            scene_id,
            scene_path.as_deref().unwrap_or("")
        );
    }
    if let Some(result) = state.metric_dataframe_shared(&shared_cache_key) {
        store_metric_dataframe_result_artifact(app_root, &response_cache_key, &result)?;
        state.store_metric_dataframe_exact(&response_cache_key, &result);
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if let Some((result, _)) = load_metric_dataframe_result_artifact(app_root, &shared_cache_key)? {
        store_metric_dataframe_result_artifact(app_root, &response_cache_key, &result)?;
        state.store_metric_dataframe_shared(&shared_cache_key, &result);
        state.store_metric_dataframe_exact(&response_cache_key, &result);
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    let reservation = state.metric_dataframe_jobs.wait_or_reserve(&shared_cache_key);
    if let ArtifactReservation::Completed = reservation {
        if let Some(result) = state.metric_dataframe_shared(&shared_cache_key) {
            store_metric_dataframe_result_artifact(app_root, &response_cache_key, &result)?;
            state.store_metric_dataframe_exact(&response_cache_key, &result);
            coverage.metric_dataframe_artifacts_ready += 1;
            return Ok(());
        }
    }
    let result = query_metric_dataframe(
        &outcome.compiled,
        app_root,
        owner_resource.id.as_str(),
        resolved_metric_id.as_str(),
        Some(scene_id.as_str()),
        scene_path.as_deref(),
        &outcome.compile_revision,
        query_options.clone(),
        None,
        Vec::new(),
    )
    .with_context(|| {
        format!(
            "build metric dataframe artifact for dataset `{}` metric `{}`",
            resource.id, resolved_metric_id
        )
    });
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            state.metric_dataframe_jobs.finish(&shared_cache_key, false);
            return Err(error);
        }
    };
    let store_result = (|| -> Result<()> {
        store_metric_dataframe_result_artifact(app_root, &shared_cache_key, &result)?;
        if shared_cache_key != response_cache_key {
            store_metric_dataframe_result_artifact(app_root, &response_cache_key, &result)?;
        }
        Ok(())
    })();
    state
        .metric_dataframe_jobs
        .finish(&shared_cache_key, store_result.is_ok());
    if store_result.is_ok() {
        state.store_metric_dataframe_shared(&shared_cache_key, &result);
        state.store_metric_dataframe_exact(&response_cache_key, &result);
    }
    store_result?;
    materialize_metric_dataframe_sibling_aliases(
        app_root,
        outcome,
        owner_resource,
        resolved_metric_id.as_str(),
        &query_options,
        &runtime_workset.defs_for_hydrate,
        &result,
        state,
    )?;
    materialize_metric_dataframe_metric_aliases(
        app_root,
        outcome,
        resource.id.as_str(),
        resolved_metric_id.as_str(),
        &query_options,
        &runtime_workset.defs_for_hydrate,
        &result,
        state,
    )?;
    coverage.metric_dataframe_artifacts_built += 1;
    Ok(())
}

fn prebuild_metric_response_shared_key(
    app_id: &str,
    owner_dataset_id: &str,
    query: &DatasetQueryOptions,
    dependency_revision_key: &str,
) -> String {
    let group = serde_json::to_string(&query.group).unwrap_or_else(|_| "[]".to_string());
    let time_range =
        serde_json::to_string(&query.time_range).unwrap_or_else(|_| "null".to_string());
    format!(
        "prebuild|response|app={app_id}|dataset={owner_dataset_id}|dependency={dependency_revision_key}|search={}|filters={}|group={group}|time_range={time_range}",
        query.search.as_deref().unwrap_or(""),
        serde_json::to_string(&query.filters).unwrap_or_else(|_| "{}".to_string()),
    )
}

fn prebuild_metric_dataframe_shared_key(
    dataset_id: &str,
    metric_id: &str,
    query: &DatasetQueryOptions,
    dependency_revision_key: &str,
) -> String {
    let group = serde_json::to_string(&query.group).unwrap_or_else(|_| "[]".to_string());
    let time_range =
        serde_json::to_string(&query.time_range).unwrap_or_else(|_| "null".to_string());
    format!(
        "prebuild|dataframe|dataset={dataset_id}|metric={metric_id}|dependency={dependency_revision_key}|search={}|filters={}|group={group}|time_range={time_range}",
        query.search.as_deref().unwrap_or(""),
        serde_json::to_string(&query.filters).unwrap_or_else(|_| "{}".to_string()),
    )
}

fn metric_response_artifact_covers_request(
    artifact: &mei_lang_datasets::LoadedMetricResponseArtifact,
    covered_metric_ids: &BTreeSet<String>,
    request_all_metrics: bool,
) -> bool {
    if request_all_metrics {
        artifact.complete
    } else {
        covered_metric_ids
            .iter()
            .all(|metric_id| artifact.covered_metric_ids.contains(metric_id))
    }
}

fn materialize_metric_response_sibling_aliases(
    app_id: &str,
    app_root: &Path,
    outcome: &CompileWithCacheOutcome,
    owner_resource: &LoadedResource,
    artifact: &LoadedMetricResponseArtifact,
    query: &DatasetQueryOptions,
    metric_defs: &BTreeMap<String, Value>,
    state: &CoverageState,
) -> Result<()> {
    let owner_dataset = owner_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
    let identity = dataset_metric_identity_key(owner_dataset);
    let (scene_id, scene_path) = artifact_scene_context(&outcome.compiled);
    for resource in &outcome.compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if dataset_metric_identity_key(dataset) != identity {
            continue;
        }
        let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
            app_root,
            &outcome.compiled,
            resource.id.as_str(),
            metric_defs,
        );
        let response_cache_key = metric_response_cache_scope_key(
            app_id,
            scene_id.as_str(),
            scene_path.as_deref(),
            resource.id.as_str(),
            query,
            &outcome.compile_revision,
            &dependency_revision_key,
            &[],
        );
        if state.metric_response_exact(&response_cache_key).is_some() {
            continue;
        }
        if load_metric_response_result_artifact(app_root, &response_cache_key)?.is_some() {
            continue;
        }
        materialize_metric_response_alias(app_root, &response_cache_key, artifact)?;
        state.store_metric_response_exact(&response_cache_key, artifact);
    }
    Ok(())
}

fn materialize_metric_dataframe_metric_aliases(
    app_root: &Path,
    outcome: &CompileWithCacheOutcome,
    resource_id: &str,
    resolved_metric_id: &str,
    query_options: &DatasetQueryOptions,
    metric_defs: &BTreeMap<String, Value>,
    result: &DatasetQueryResult,
    state: &CoverageState,
) -> Result<()> {
    let (scene_id, scene_path) = artifact_scene_context(&outcome.compiled);
    for metric_selector in
        equivalent_dataframe_metric_ids(&outcome.compiled, resource_id, resolved_metric_id)
    {
        let Ok((owner_resource, canonical_metric_id)) =
            locate_runtime_metric_resource(&outcome.compiled, resource_id, metric_selector.as_str())
        else {
            continue;
        };
        let owner_dataset = owner_resource
            .dataset
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
        let runtime_workset = runtime_metric_workset(
            &owner_resource.id,
            &[canonical_metric_id.clone()],
            owner_dataset,
        );
        let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
            app_root,
            &outcome.compiled,
            &owner_resource.id,
            &runtime_workset.defs_for_hydrate,
        );
        let scope_metric_token = dataframe_scope_metric_token(
            &outcome.compiled,
            owner_resource.id.as_str(),
            metric_selector.as_str(),
        )
        .unwrap_or_else(|| {
            metric_scope_cache_key(std::slice::from_ref(&canonical_metric_id))
        });
        let response_cache_key = metric_dataframe_result_cache_key(
            app_root,
            Some(scene_id.as_str()),
            scene_path.as_deref(),
            owner_resource.id.as_str(),
            scope_metric_token.as_str(),
            query_options,
            &outcome.compile_revision,
            &dependency_revision_key,
            &[],
        );
        if state.metric_dataframe_exact(&response_cache_key).is_some() {
            continue;
        }
        if load_metric_dataframe_result_artifact(app_root, &response_cache_key)?.is_some() {
            continue;
        }
        store_metric_dataframe_result_artifact(app_root, &response_cache_key, result)?;
        state.store_metric_dataframe_exact(&response_cache_key, result);
    }
    let _ = metric_defs;
    Ok(())
}

fn materialize_metric_dataframe_sibling_aliases(
    app_root: &Path,
    outcome: &CompileWithCacheOutcome,
    owner_resource: &LoadedResource,
    resolved_metric_id: &str,
    query_options: &DatasetQueryOptions,
    metric_defs: &BTreeMap<String, Value>,
    result: &DatasetQueryResult,
    state: &CoverageState,
) -> Result<()> {
    let owner_dataset = owner_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
    let identity = dataset_metric_identity_key(owner_dataset);
    let (scene_id, scene_path) = artifact_scene_context(&outcome.compiled);
    for resource in &outcome.compiled.resources {
        let Some(dataset) = resource.dataset.as_ref() else {
            continue;
        };
        if dataset_metric_identity_key(dataset) != identity {
            continue;
        }
        let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
            app_root,
            &outcome.compiled,
            resource.id.as_str(),
            metric_defs,
        );
        let scope_metric_token = dataframe_scope_metric_token(
            &outcome.compiled,
            resource.id.as_str(),
            resolved_metric_id,
        )
        .unwrap_or_else(|| {
            metric_scope_cache_key(std::slice::from_ref(&resolved_metric_id.to_string()))
        });
        let response_cache_key = metric_dataframe_result_cache_key(
            app_root,
            Some(scene_id.as_str()),
            scene_path.as_deref(),
            resource.id.as_str(),
            scope_metric_token.as_str(),
            query_options,
            &outcome.compile_revision,
            &dependency_revision_key,
            &[],
        );
        if state.metric_dataframe_exact(&response_cache_key).is_some() {
            continue;
        }
        if load_metric_dataframe_result_artifact(app_root, &response_cache_key)?.is_some() {
            continue;
        }
        store_metric_dataframe_result_artifact(app_root, &response_cache_key, result)?;
        state.store_metric_dataframe_exact(&response_cache_key, result);
    }
    Ok(())
}

fn materialize_metric_response_alias(
    app_root: &Path,
    response_cache_key: &str,
    artifact: &mei_lang_datasets::LoadedMetricResponseArtifact,
) -> Result<()> {
    materialize_metric_response_alias_parts(
        app_root,
        response_cache_key,
        artifact.total_rows,
        &artifact.metrics_map,
        &artifact.covered_metric_ids,
        artifact.complete,
    )
}

fn materialize_metric_response_alias_parts(
    app_root: &Path,
    response_cache_key: &str,
    total_rows: usize,
    metrics_map: &BTreeMap<String, mei_lang_kernel::MetricContract>,
    covered_metric_ids: &BTreeSet<String>,
    complete: bool,
) -> Result<()> {
    store_cached_metric_response(
        response_cache_key.to_string(),
        total_rows,
        metrics_map,
        covered_metric_ids,
        complete,
    );
    store_metric_response_result_artifact(
        app_root,
        response_cache_key,
        total_rows,
        metrics_map,
        covered_metric_ids,
        complete,
    )
}

fn prebuild_parallelism(job_count: usize) -> usize {
    if job_count <= 1 {
        return 1;
    }
    thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1)
        .min(PREBUILD_MAX_PARALLELISM)
        .min(job_count)
        .max(1)
}

fn run_limited_parallel_ordered<T, R, F>(
    items: Vec<T>,
    max_parallelism: usize,
    job: F,
) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> R + Sync,
{
    if items.len() <= 1 || max_parallelism <= 1 {
        return items.into_iter().map(job).collect();
    }
    let worker_count = max_parallelism.min(items.len()).max(1);
    let mut buckets = (0..worker_count)
        .map(|_| Vec::<(usize, T)>::new())
        .collect::<Vec<_>>();
    for (index, item) in items.into_iter().enumerate() {
        buckets[index % worker_count].push((index, item));
    }
    thread::scope(|scope| {
        let job_ref = &job;
        let mut handles = Vec::new();
        for bucket in buckets.into_iter().filter(|bucket| !bucket.is_empty()) {
            handles.push(scope.spawn(move || {
                let mut output = Vec::with_capacity(bucket.len());
                for (index, item) in bucket {
                    output.push((index, job_ref(item)));
                }
                output
            }));
        }
        let mut output = Vec::new();
        for handle in handles {
            output.extend(handle.join().expect("prebuild parallel worker panicked"));
        }
        output.sort_by_key(|(index, _)| *index);
        output.into_iter().map(|(_, result)| result).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::CompiledApp;

    fn test_outcome(active_scene: &str, active_target_file: &str) -> CompileWithCacheOutcome {
        CompileWithCacheOutcome {
            compiled: CompiledApp {
                app_id: "demo".to_string(),
                title: "demo".to_string(),
                app_root: "/tmp/demo".to_string(),
                active_scene: Some(active_scene.to_string()),
                active_target_file: active_target_file.to_string(),
                scene_routes: Vec::new(),
                file_tree: Vec::new(),
                resources: Vec::new(),
                world_metrics: BTreeMap::new(),
                world_semantic_by_file: BTreeMap::new(),
                component_assets: Vec::new(),
                diagnostics: Vec::new(),
                scene_contract: None,
                scene_local_nav_by_target: BTreeMap::new(),
                scene_bindings_by_id: BTreeMap::new(),
                scene_examples_by_id: BTreeMap::new(),
                scene_projection_assembly_by_id: BTreeMap::new(),
                build_experience_index: Default::default(),
                build_board_index: Default::default(),
                build_template_index: Default::default(),
            },
            cache_hit: true,
            artifact_cache_hit: true,
            compile_revision: "rev-a".to_string(),
            revision_scope: "test".to_string(),
            cache_validation: "test".to_string(),
            cache_lookup_ms: 0,
            artifact_load_ms: 0,
            compile_cache_lock_wait_ms: 0,
            compile_ms: 0,
        }
    }

    #[test]
    fn prebuild_report_summary_omits_compile_revision() {
        let report = PrebuildReport {
            schema_version: "mei-prebuild-report-v1".to_string(),
            mode: PrebuildMode::Verify,
            clean: false,
            clean_wall_ms: 0,
            total_wall_ms: 1200,
            source_root: "/tmp/ws".to_string(),
            manifest_path: "/tmp/ws/.mei/runtime/warmup-manifest.json".to_string(),
            manifest_source: "workspace_config_fallback".to_string(),
            ok: true,
            succeeded_apps: vec!["zhifa".to_string()],
            failed_apps: Vec::new(),
            error_summary: Vec::new(),
            apps: vec![PrebuildAppReport {
                app_id: "zhifa".to_string(),
                compile_scopes: vec![PrebuildScopeReport {
                    requested_scene_id: Some("home".to_string()),
                    requested_target_file: Some("scenes/01-执法要素.mei".to_string()),
                    active_scene_id: Some("home".to_string()),
                    active_target_file: "scenes/01-执法要素.mei".to_string(),
                    cache_hit: true,
                    artifact_cache_hit: true,
                    compile_revision: "very-long-revision-token".to_string(),
                    cache_lookup_ms: 0,
                    artifact_load_ms: 12,
                    compile_ms: 0,
                }],
                coverage: PrebuildCoverageReport::default(),
                timings: PrebuildTimingReport::default(),
                data_snapshots: None,
                warnings: Vec::new(),
            }],
        };
        let json = serde_json::to_string(&report.summary()).expect("serialize summary");
        assert!(!json.contains("compile_revision"));
        assert!(!json.contains("very-long-revision-token"));
        assert!(json.contains("scenes/01-执法要素.mei"));
    }

    #[test]
    fn compile_scopes_follow_explicit_manifest_closure() {
        let app = RuntimeWarmupApp {
            app_id: "demo".to_string(),
            default_scene: Some("home".to_string()),
            hot_scenes: vec!["dashboard".to_string()],
            scenes: vec!["home".to_string()],
            focuses: vec!["scenes/02-inspection.mei".to_string()],
            datasets: vec![RuntimeWarmupDatasetRequest {
                scene_id: Some("details".to_string()),
                focus: Some("scenes/details.mei".to_string()),
                dataset_id: "demo_ds".to_string(),
                metric_id: None,
                metric_ids: Vec::new(),
            }],
            xlsx_sources: Vec::new(),
        };
        let scope_keys = compile_scopes_for_app(&app)
            .into_iter()
            .map(|scope| scope.key())
            .collect::<BTreeSet<_>>();

        assert!(scope_keys.contains("|"));
        assert!(scope_keys.contains("home|"));
        assert!(scope_keys.contains("dashboard|"));
        assert!(scope_keys.contains("|scenes/02-inspection.mei"));
        assert!(scope_keys.contains("details|scenes/details.mei"));
        assert!(scope_keys.contains("home|scenes/02-inspection.mei"));
        assert!(scope_keys.contains("dashboard|scenes/02-inspection.mei"));
    }

    #[test]
    fn focus_targets_from_warmup_datasets_extracts_scene_paths() {
        let app = RuntimeWarmupApp {
            app_id: "demo".to_string(),
            default_scene: None,
            hot_scenes: Vec::new(),
            scenes: Vec::new(),
            focuses: Vec::new(),
            datasets: vec![RuntimeWarmupDatasetRequest {
                scene_id: Some("home".to_string()),
                focus: None,
                dataset_id: "__world_metrics__::scenes/05-监督预警.mei::metrics".to_string(),
                metric_id: None,
                metric_ids: Vec::new(),
            }],
            xlsx_sources: Vec::new(),
        };
        assert_eq!(
            focus_targets_from_warmup_datasets(&app),
            vec!["scenes/05-监督预警.mei".to_string()]
        );
    }

    #[test]
    fn requested_metric_ids_merge_scalar_and_list_fields() {
        let request = RuntimeWarmupDatasetRequest {
            scene_id: Some("home".to_string()),
            focus: None,
            dataset_id: "demo_ds".to_string(),
            metric_id: Some("total".to_string()),
            metric_ids: vec!["delta".to_string(), "total".to_string()],
        };

        assert_eq!(
            requested_metric_ids(&request),
            vec!["delta".to_string(), "total".to_string()]
        );
    }

    #[test]
    fn unique_prepared_outcomes_prefers_scene_scoped_compile_scope() {
        let home_scope = CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: None,
        };
        let default_scope = CompileScope::default_scope();
        let prepared = vec![
            (default_scope, test_outcome("home", "scenes/home.mei")),
            (home_scope, test_outcome("home", "scenes/home.mei")),
        ];
        let unique = unique_prepared_outcomes_for_artifacts(&prepared);
        assert_eq!(unique.len(), 1);
        assert_eq!(
            unique[0].0.requested_scene_id.as_deref(),
            Some("home")
        );
    }

    #[test]
    fn warmup_request_matches_active_scene_without_exact_scope_key() {
        let request = AggregatedWarmupRequest {
            scope: CompileScope {
                requested_scene_id: Some("home".to_string()),
                requested_target_file: None,
            },
            dataset_id: "penalty_result_dashboard_ds".to_string(),
            metric_ids: vec!["penalties_total_count::__scalar_rowset__".to_string()],
        };
        let outcome = test_outcome("home", "scenes/home.mei");
        assert!(warmup_request_matches_outcome(&request, &outcome));
        assert_eq!(
            matching_warmup_requests_for_outcome(&[request], &outcome).len(),
            1
        );
    }

    #[test]
    fn parallel_runner_preserves_input_order() {
        let values = run_limited_parallel_ordered(vec![1, 2, 3, 4], 4, |value| value * 10);
        assert_eq!(values, vec![10, 20, 30, 40]);
    }
}
