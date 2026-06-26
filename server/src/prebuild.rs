use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mei_lang_datasets::{
    collect_all_query_options, evaluate_runtime_metrics_from_plan,
    load_metric_dataframe_result_artifact, load_metric_response_result_artifact,
    locate_runtime_metric_resource, metric_dataframe_result_artifact_exists,
    metric_dataframe_result_cache_key, metric_request_revision_fingerprint_for_compiled,
    metric_response_cache_scope_key, metric_response_prebuild_shared_key,
    metric_response_result_artifact_exists, metric_scope_cache_key,
    plan_access_metric_eval_for_ids, prebuild_metric_response_index_covers_key,
    query_metric_dataframe, query_state_from_request, runtime_metric_workset,
    store_cached_metric_response, store_metric_dataframe_result_artifact,
    store_metric_response_result_artifact, DatasetQueryOptions, DatasetQueryResult,
    AccessMetricEvalPlan, LoadedMetricResponseArtifact, RuntimeMetricEvalMode,
};
use mei_lang_kernel::{
    begin_prebuild_generation, clear_prebuild_build_root_override, data_snapshot_import_manifest_path,
    data_snapshot_store_root, finish_prebuild_generation, resolve_app_root,
    resolve_data_snapshot_import_entry, resolve_runtime_warmup_manifest, set_prebuild_build_root_override,
    CompileOptions, CompiledApp, DatasetView, LoadedResource, RuntimeWarmupApp,
    RuntimeWarmupDatasetRequest, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL,
};
use mei_lang_toolchain::{self as toolchain, PublishDataSnapshotsReport, WorldScope};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

const PREBUILD_REPORT_SCHEMA_VERSION: &str = "mei-prebuild-report-v1";
const PREBUILD_MAX_PARALLELISM: usize = 16;
const CANONICAL_PREBUILD_NODE_BUDGET: usize = 90;
const STARTUP_PREBUILD_WALL_MS_BUDGET_MS: u64 = 60_000;

fn prebuild_max_parallelism_cap() -> usize {
    static CAP: OnceLock<usize> = OnceLock::new();
    *CAP.get_or_init(|| {
        std::env::var("MEI_PREBUILD_MAX_PARALLELISM")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(PREBUILD_MAX_PARALLELISM)
    })
}

fn prebuild_disk_diagnostics_enabled() -> bool {
    std::env::var("MEI_PREBUILD_DISK_DIAGNOSTICS")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

fn prebuild_progress_every_n() -> usize {
    static EVERY_N: OnceLock<usize> = OnceLock::new();
    *EVERY_N.get_or_init(|| {
        std::env::var("MEI_PREBUILD_PROGRESS_EVERY_N")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(5)
    })
}

fn prebuild_progress_heartbeat_secs() -> u64 {
    static HEARTBEAT_SECS: OnceLock<u64> = OnceLock::new();
    *HEARTBEAT_SECS.get_or_init(|| {
        std::env::var("MEI_PREBUILD_PROGRESS_HEARTBEAT_SECS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value >= 5)
            .unwrap_or(30)
    })
}

fn format_eta_secs(secs: f64) -> String {
    if !secs.is_finite() || secs <= 0.0 {
        return "—".to_string();
    }
    if secs < 60.0 {
        return format!("{:.0}s", secs);
    }
    let mins = secs / 60.0;
    if mins < 60.0 {
        return format!("{:.1}min", mins);
    }
    format!("{:.1}h", mins / 60.0)
}

fn emit_compile_batch_progress(
    app_id: &str,
    batch_idx: usize,
    done: usize,
    unique_total: usize,
    batch_started: Instant,
    scopes_completed_before_batch: usize,
    pending_after_discover: usize,
    new_compile: usize,
    cache_hit: usize,
    force: bool,
    last_emit: &Mutex<Instant>,
) {
    if unique_total == 0 {
        return;
    }
    let every_n = prebuild_progress_every_n();
    let heartbeat = std::time::Duration::from_secs(prebuild_progress_heartbeat_secs());
    let should_emit = force
        || done == unique_total
        || (done > 0 && done % every_n == 0)
        || last_emit
            .lock()
            .map(|guard| guard.elapsed() >= heartbeat)
            .unwrap_or(false);
    if !should_emit {
        return;
    }
    let elapsed = batch_started.elapsed().as_secs_f64().max(0.1);
    let rate = done as f64 / elapsed;
    let remaining = unique_total.saturating_sub(done);
    let eta_secs = if done > 0 {
        remaining as f64 / rate.max(0.01)
    } else {
        f64::NAN
    };
    let scopes_done_total = scopes_completed_before_batch.saturating_add(done);
    let queue_hint = if pending_after_discover > 0 {
        format!(" | 待发现队列 {pending_after_discover}")
    } else {
        String::new()
    };
    prebuild_emit_progress(format!(
        "[{app_id}] batch-{batch_idx} 进度 {done}/{unique_total} key | 本批已用 {} | 约 {} 剩余 | 累计 scope ~{scopes_done_total}{queue_hint} | 新编译 {new_compile} 缓存 {cache_hit}",
        format_eta_secs(elapsed),
        format_eta_secs(eta_secs),
    ));
    if let Ok(mut guard) = last_emit.lock() {
        *guard = Instant::now();
    }
}

fn compile_scope_key_from_parts(
    requested_scene_id: Option<&str>,
    requested_target_file: Option<&str>,
) -> String {
    CompileScope {
        requested_scene_id: requested_scene_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        requested_target_file: requested_target_file
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    }
    .canonicalized()
    .key()
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.2} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

fn current_process_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let statm = fs::read_to_string("/proc/self/statm").ok()?;
        let resident_pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
        return Some(resident_pages * 4096);
    }
    #[cfg(target_os = "macos")]
    {
        let pid = std::process::id();
        let output = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        let kb: u64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?;
        return Some(kb * 1024);
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = ();
        None
    }
}

fn sample_peak_rss_bytes(peak: &AtomicUsize) {
    let Some(rss) = current_process_rss_bytes() else {
        return;
    };
    let current = rss as usize;
    let mut prev = peak.load(Ordering::Relaxed);
    while current > prev {
        match peak.compare_exchange_weak(prev, current, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => prev = next,
        }
    }
}

struct DirSizeSummary {
    files: usize,
    bytes: u64,
}

fn dir_size_summary(root: &Path) -> DirSizeSummary {
    if !root.is_dir() {
        return DirSizeSummary { files: 0, bytes: 0 };
    }
    let mut files = 0usize;
    let mut bytes = 0u64;
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let entry_type = entry.file_type();
        if entry_type.is_file() {
            files += 1;
            bytes += entry.metadata().map(|meta| meta.len()).unwrap_or(0);
        }
    }
    DirSizeSummary { files, bytes }
}

fn prebuild_progress_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn prebuild_progress_origin() -> &'static Mutex<Option<Instant>> {
    static ORIGIN: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    ORIGIN.get_or_init(|| Mutex::new(None))
}

struct PrebuildProgressSession;

impl PrebuildProgressSession {
    fn begin() -> Self {
        if let Ok(mut guard) = prebuild_progress_origin().lock() {
            *guard = Some(Instant::now());
        }
        Self
    }
}

impl Drop for PrebuildProgressSession {
    fn drop(&mut self) {
        if let Ok(mut guard) = prebuild_progress_origin().lock() {
            *guard = None;
        }
    }
}

fn supports_ansi_stderr() -> bool {
    std::io::stderr().is_terminal()
}

fn ansi_wrap(text: &str, code: &str) -> String {
    if supports_ansi_stderr() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn prebuild_emit_progress(message: impl AsRef<str>) {
    let _guard = prebuild_progress_lock()
        .lock()
        .expect("prebuild progress lock");
    let elapsed = prebuild_progress_origin()
        .lock()
        .ok()
        .and_then(|guard| *guard)
        .map(|started| started.elapsed().as_millis() as u64);
    let prefix = match elapsed {
        Some(ms) if ms < 1000 => format!("[PREBUILD +{ms}ms]"),
        Some(ms) => format!("[PREBUILD +{:.1}s]", ms as f64 / 1000.0),
        None => "[PREBUILD]".to_string(),
    };
    eprintln!("{} {}", ansi_wrap(&prefix, "1;36"), message.as_ref());
    let _ = std::io::stderr().flush();
}

fn format_scope_file(scene: &str, requested_target: &str, active_target: Option<&str>) -> String {
    if !requested_target.is_empty() {
        return requested_target.to_string();
    }
    if let Some(target) = active_target
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return format!("{target} (推导)");
    }
    if scene.is_empty() {
        "app 默认入口".to_string()
    } else {
        format!("{scene}/场景入口")
    }
}

fn short_metric_id(metric_id: &str) -> &str {
    metric_id.rsplit("::").next().unwrap_or(metric_id)
}

fn short_dataset_id(dataset_id: &str) -> String {
    let tail = dataset_id
        .rsplit("::")
        .next()
        .unwrap_or(dataset_id)
        .rsplit('/')
        .next()
        .unwrap_or(dataset_id);
    if dataset_id.contains("::") {
        format!("{tail}")
    } else {
        dataset_id.to_string()
    }
}

fn emit_slow_compile_report(_app_id: &str, reports: &[PrebuildScopeReport]) {
    let mut slow: Vec<&PrebuildScopeReport> = reports
        .iter()
        .filter(|report| !report.cache_hit && report.compile_ms > 0)
        .collect();
    if slow.is_empty() {
        return;
    }
    slow.sort_by_key(|report| std::cmp::Reverse(report.compile_ms));
    for report in slow.into_iter().take(8) {
        let scene = report
            .requested_scene_id
            .as_deref()
            .or(report.active_scene_id.as_deref())
            .unwrap_or("-");
        let file = report
            .requested_target_file
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(report.active_target_file.as_str());
        prebuild_emit_progress(format!(
            "  {:.1}s | scene={scene} | file={file}",
            report.compile_ms as f64 / 1000.0
        ));
    }
}

#[derive(Clone)]
struct MetricBuildTiming {
    kind: &'static str,
    dataset: String,
    metric: String,
    scene: String,
    ms: u64,
}

#[derive(Default)]
struct PrebuildDiagnostics {
    metric_builds: Mutex<Vec<MetricBuildTiming>>,
    peak_rss_bytes: AtomicUsize,
    compile_preload_reuse_hits: AtomicUsize,
    compile_postload_identity_collapses: AtomicUsize,
    compile_index_hits: AtomicUsize,
    compile_index_misses: AtomicUsize,
    compile_index_stale_entries: AtomicUsize,
    compile_fallback_loads: AtomicUsize,
    compile_manifest_probes: AtomicUsize,
    compile_manifest_stale_skips: AtomicUsize,
    compile_artifact_loads_avoided: AtomicUsize,
    mrg_eval_skips: AtomicUsize,
    dataframe_eval_skips: AtomicUsize,
}

impl PrebuildDiagnostics {
    fn sample_memory_peak(&self) {
        sample_peak_rss_bytes(&self.peak_rss_bytes);
    }

    fn record_metric_build(
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

const PREBUILD_COMPILE_INDEX_SCHEMA_V6: &str = "mei-prebuild-compile-index-v6";
const PREBUILD_COMPILE_INDEX_SCHEMA_V7: &str = "mei-prebuild-compile-index-v7";
const PREBUILD_COMPILE_INDEX_SCHEMA_V8: &str = "mei-prebuild-compile-index-v8";

fn default_observed_count() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedCompileScopeRef {
    requested_scene_id: Option<String>,
    requested_target_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedPrebuildCompileIndexEntry {
    scope_key: String,
    requested_scene_id: Option<String>,
    requested_target_file: Option<String>,
    compile_cache_key: String,
    canonical_scope_key: String,
    canonical_requested_scene_id: Option<String>,
    canonical_requested_target_file: Option<String>,
    canonical_compile_cache_key: String,
    identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scene_payload_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    assembly_view_revision: Option<String>,
    #[serde(default)]
    discovered_scopes: Vec<PersistedCompileScopeRef>,
    #[serde(default = "default_observed_count")]
    observed_count: usize,
    generated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedPrebuildCompileIndex {
    schema_version: String,
    generated_at_ms: u64,
    entries: Vec<PersistedPrebuildCompileIndexEntry>,
}

#[derive(Debug, Clone, Default)]
struct PrebuildCompileIndex {
    entries_by_scope_key: BTreeMap<String, PersistedPrebuildCompileIndexEntry>,
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}

fn prebuild_compile_index_path(app_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_app_build_root(app_root)
        .join("prebuild")
        .join("compile-index.json")
}

fn write_prebuild_compile_index(app_root: &Path, index: &PrebuildCompileIndex) -> Result<()> {
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

fn load_prebuild_compile_index(app_root: &Path) -> Result<Option<PrebuildCompileIndex>> {
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

fn compile_scope_from_parts(
    requested_scene_id: Option<String>,
    requested_target_file: Option<String>,
) -> CompileScope {
    CompileScope {
        requested_scene_id,
        requested_target_file,
    }
    .canonicalized()
}

fn build_prebuild_compile_index(
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
        let identity = compiled_scope_identity(&prepared.outcome);
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
        let identity = compiled_scope_identity(outcome);
        let Some(canonical) = best_scope_by_identity.get(&identity) else {
            continue;
        };
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

fn assembly_view_index_key(
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

fn scope_assembled_outcome(
    base: &SharedCompileOutcome,
    scope: &CompileScope,
) -> SharedCompileOutcome {
    if compile_outcome_matches_scope(scope, &base.compiled) {
        return base.clone();
    }
    let canonical = scope.canonicalized();
    let assembled = crate::graph::mcg::assemble::assemble_scope_view(
        (*base.compiled).clone(),
        canonical.requested_scene_id.as_deref(),
        canonical
            .requested_target_file
            .as_deref()
            .filter(|value| !value.is_empty()),
    );
    SharedCompileOutcome {
        compiled: Arc::new(assembled),
        cache_hit: base.cache_hit,
        artifact_cache_hit: base.artifact_cache_hit,
        compile_revision: base.compile_revision.clone(),
        cache_lookup_ms: base.cache_lookup_ms,
        artifact_load_ms: base.artifact_load_ms,
        compile_ms: 0,
    }
}

fn compile_active_identity(report: &PrebuildScopeReport) -> String {
    format!(
        "{}|{}",
        report.active_scene_id.as_deref().unwrap_or(""),
        report.active_target_file
    )
}

fn disk_usage_report(summary: DirSizeSummary) -> PrebuildDiskUsageReport {
    PrebuildDiskUsageReport {
        files: summary.files,
        bytes: summary.bytes,
    }
}

fn build_prebuild_diagnostics_report(
    app_root: &Path,
    reports: &[PrebuildScopeReport],
    diagnostics: &PrebuildDiagnostics,
    plan_nodes: PrebuildPlanNodeStatsReport,
    canonical_identity_count: usize,
    session_entries_before_clear: (usize, usize, usize),
    session_entries_after_clear: (usize, usize, usize),
    warmup_reuse_hits: usize,
    critical_warmup_total_count: usize,
    critical_warmup_executed_count: usize,
    critical_warmup_cache_hit_count: usize,
    critical_warmup_ms: u64,
    critical_warmup_ok: bool,
    deferred_warmup_total_count: usize,
    deferred_warmup_executed_count: usize,
    deferred_warmup_cache_hit_count: usize,
    deferred_warmup_ms: u64,
    deferred_warmup_ok: bool,
) -> PrebuildDiagnosticsReport {
    let total_scope_checks = reports.len();
    let real_compile_count = reports.iter().filter(|report| !report.cache_hit).count();
    let cache_hit_count = reports.iter().filter(|report| report.cache_hit).count();
    let cache_probe_ms: u64 = reports
        .iter()
        .filter(|report| report.cache_hit)
        .map(|report| {
            report
                .cache_lookup_ms
                .saturating_add(report.artifact_load_ms)
        })
        .sum();
    let compile_miss_ms: u64 = reports
        .iter()
        .filter(|report| !report.cache_hit)
        .map(|report| report.compile_ms)
        .sum();
    let unique_compile_result_count = reports
        .iter()
        .map(compile_active_identity)
        .collect::<BTreeSet<_>>()
        .len();
    let redundant_scope_checks = total_scope_checks.saturating_sub(unique_compile_result_count);
    let expansion_ratio = if unique_compile_result_count > 0 {
        total_scope_checks as f64 / unique_compile_result_count as f64
    } else {
        1.0
    };
    let preload_reuse_hits = diagnostics
        .compile_preload_reuse_hits
        .load(Ordering::Relaxed);
    let postload_identity_collapses = diagnostics
        .compile_postload_identity_collapses
        .load(Ordering::Relaxed);
    let compile_index_hits = diagnostics.compile_index_hits.load(Ordering::Relaxed);
    let compile_index_misses = diagnostics.compile_index_misses.load(Ordering::Relaxed);
    let compile_index_stale_entries = diagnostics
        .compile_index_stale_entries
        .load(Ordering::Relaxed);
    let compile_fallback_loads = diagnostics.compile_fallback_loads.load(Ordering::Relaxed);
    let manifest_probes = diagnostics.compile_manifest_probes.load(Ordering::Relaxed);
    let manifest_stale_skips = diagnostics
        .compile_manifest_stale_skips
        .load(Ordering::Relaxed);
    let artifact_loads_avoided = diagnostics
        .compile_artifact_loads_avoided
        .load(Ordering::Relaxed);
    let mrg_eval_skips = diagnostics.mrg_eval_skips.load(Ordering::Relaxed);
    let dataframe_eval_skips = diagnostics.dataframe_eval_skips.load(Ordering::Relaxed);
    let eval_root = mei_lang_kernel::resolve_app_var_root(app_root).join("eval-results");
    let response_dir = eval_root.join("results").join("metric-response");
    let dataframe_dir = eval_root.join("results").join("metric-dataframe");
    let current_rss_bytes = current_process_rss_bytes();
    let peak_rss_bytes = diagnostics.peak_rss_bytes.load(Ordering::Relaxed) as u64;

    let mut slow_scopes = reports
        .iter()
        .filter(|report| !report.cache_hit && report.compile_ms > 0)
        .map(|report| PrebuildSlowScopeDiagnostic {
            scene_id: report
                .requested_scene_id
                .clone()
                .or(report.active_scene_id.clone()),
            target_file: report
                .requested_target_file
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| report.active_target_file.clone()),
            compile_ms: report.compile_ms,
        })
        .collect::<Vec<_>>();
    slow_scopes.sort_by_key(|entry| std::cmp::Reverse(entry.compile_ms));
    slow_scopes.truncate(8);

    let mut slow_metrics = diagnostics
        .metric_builds
        .lock()
        .expect("lock prebuild diagnostics")
        .iter()
        .map(|entry| PrebuildSlowMetricDiagnostic {
            kind: entry.kind.to_string(),
            dataset: entry.dataset.clone(),
            metric: entry.metric.clone(),
            scene: entry.scene.clone(),
            ms: entry.ms,
        })
        .collect::<Vec<_>>();
    slow_metrics.sort_by_key(|entry| std::cmp::Reverse(entry.ms));
    slow_metrics.truncate(8);

    PrebuildDiagnosticsReport {
        total_scope_checks,
        real_compile_count,
        cache_hit_count,
        unique_compile_result_count,
        canonical_identity_count,
        redundant_scope_checks,
        expansion_ratio,
        cache_probe_ms,
        compile_miss_ms,
        current_rss_bytes,
        peak_rss_bytes,
        eval_artifacts_disk: PrebuildEvalArtifactDiskReport {
            total: disk_usage_report(dir_size_summary(eval_root.as_path())),
            metric_response: disk_usage_report(dir_size_summary(response_dir.as_path())),
            metric_dataframe: disk_usage_report(dir_size_summary(dataframe_dir.as_path())),
        },
        compile_index: PrebuildCompileIndexStatsReport {
            preload_reuse_hits,
            postload_identity_collapses,
            hits: compile_index_hits,
            misses: compile_index_misses,
            stale_entries: compile_index_stale_entries,
            fallback_loads: compile_fallback_loads,
            manifest_probes,
            manifest_stale_skips,
            artifact_loads_avoided,
            mrg_eval_skips,
            dataframe_eval_skips,
        },
        session_before_clear: PrebuildSessionEntryStatsReport {
            scope_entries: session_entries_before_clear.0,
            cache_entries: session_entries_before_clear.1,
            identity_entries: session_entries_before_clear.2,
        },
        session_after_clear: PrebuildSessionEntryStatsReport {
            scope_entries: session_entries_after_clear.0,
            cache_entries: session_entries_after_clear.1,
            identity_entries: session_entries_after_clear.2,
        },
        warmup_reuse_hits,
        plan_nodes,
        critical_warmup: PrebuildWarmupDiagnosticReport {
            total_request_count: critical_warmup_total_count,
            executed_request_count: critical_warmup_executed_count,
            cache_hit_count: critical_warmup_cache_hit_count,
            total_ms: critical_warmup_ms,
            ok: critical_warmup_ok,
        },
        deferred_warmup: PrebuildWarmupDiagnosticReport {
            total_request_count: deferred_warmup_total_count,
            executed_request_count: deferred_warmup_executed_count,
            cache_hit_count: deferred_warmup_cache_hit_count,
            total_ms: deferred_warmup_ms,
            ok: deferred_warmup_ok,
        },
        slow_scopes,
        slow_metrics,
        fingerprint_skip: false,
        inputs_fingerprint: None,
    }
}

fn aggregate_prebuild_diagnostics(apps: &[PrebuildAppReport]) -> PrebuildDiagnosticsReport {
    let mut aggregate = PrebuildDiagnosticsReport::default();
    let mut slow_scopes = Vec::new();
    let mut slow_metrics = Vec::new();
    for app in apps {
        let diagnostics = &app.diagnostics;
        aggregate.total_scope_checks += diagnostics.total_scope_checks;
        aggregate.real_compile_count += diagnostics.real_compile_count;
        aggregate.cache_hit_count += diagnostics.cache_hit_count;
        aggregate.unique_compile_result_count += diagnostics.unique_compile_result_count;
        aggregate.canonical_identity_count += diagnostics.canonical_identity_count;
        aggregate.redundant_scope_checks += diagnostics.redundant_scope_checks;
        aggregate.cache_probe_ms = aggregate
            .cache_probe_ms
            .saturating_add(diagnostics.cache_probe_ms);
        aggregate.compile_miss_ms = aggregate
            .compile_miss_ms
            .saturating_add(diagnostics.compile_miss_ms);
        aggregate.current_rss_bytes =
            match (aggregate.current_rss_bytes, diagnostics.current_rss_bytes) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            };
        aggregate.peak_rss_bytes = aggregate.peak_rss_bytes.max(diagnostics.peak_rss_bytes);
        aggregate.eval_artifacts_disk.total.files += diagnostics.eval_artifacts_disk.total.files;
        aggregate.eval_artifacts_disk.total.bytes += diagnostics.eval_artifacts_disk.total.bytes;
        aggregate.eval_artifacts_disk.metric_response.files +=
            diagnostics.eval_artifacts_disk.metric_response.files;
        aggregate.eval_artifacts_disk.metric_response.bytes +=
            diagnostics.eval_artifacts_disk.metric_response.bytes;
        aggregate.eval_artifacts_disk.metric_dataframe.files +=
            diagnostics.eval_artifacts_disk.metric_dataframe.files;
        aggregate.eval_artifacts_disk.metric_dataframe.bytes +=
            diagnostics.eval_artifacts_disk.metric_dataframe.bytes;
        aggregate.compile_index.preload_reuse_hits += diagnostics.compile_index.preload_reuse_hits;
        aggregate.compile_index.postload_identity_collapses +=
            diagnostics.compile_index.postload_identity_collapses;
        aggregate.compile_index.hits += diagnostics.compile_index.hits;
        aggregate.compile_index.misses += diagnostics.compile_index.misses;
        aggregate.compile_index.stale_entries += diagnostics.compile_index.stale_entries;
        aggregate.compile_index.fallback_loads += diagnostics.compile_index.fallback_loads;
        aggregate.compile_index.manifest_probes += diagnostics.compile_index.manifest_probes;
        aggregate.compile_index.manifest_stale_skips +=
            diagnostics.compile_index.manifest_stale_skips;
        aggregate.compile_index.artifact_loads_avoided +=
            diagnostics.compile_index.artifact_loads_avoided;
        aggregate.compile_index.mrg_eval_skips += diagnostics.compile_index.mrg_eval_skips;
        aggregate.compile_index.dataframe_eval_skips +=
            diagnostics.compile_index.dataframe_eval_skips;
        aggregate.session_before_clear.scope_entries +=
            diagnostics.session_before_clear.scope_entries;
        aggregate.session_before_clear.cache_entries +=
            diagnostics.session_before_clear.cache_entries;
        aggregate.session_before_clear.identity_entries +=
            diagnostics.session_before_clear.identity_entries;
        aggregate.session_after_clear.scope_entries +=
            diagnostics.session_after_clear.scope_entries;
        aggregate.session_after_clear.cache_entries +=
            diagnostics.session_after_clear.cache_entries;
        aggregate.session_after_clear.identity_entries +=
            diagnostics.session_after_clear.identity_entries;
        aggregate.warmup_reuse_hits += diagnostics.warmup_reuse_hits;
        aggregate.plan_nodes.manifest_compile_scope_nodes +=
            diagnostics.plan_nodes.manifest_compile_scope_nodes;
        aggregate.plan_nodes.hot_compile_scope_nodes +=
            diagnostics.plan_nodes.hot_compile_scope_nodes;
        aggregate.plan_nodes.deferred_compile_scope_nodes +=
            diagnostics.plan_nodes.deferred_compile_scope_nodes;
        aggregate.plan_nodes.planned_warmup_request_nodes +=
            diagnostics.plan_nodes.planned_warmup_request_nodes;
        aggregate.plan_nodes.planned_warmup_scope_nodes +=
            diagnostics.plan_nodes.planned_warmup_scope_nodes;
        aggregate.plan_nodes.planned_metric_workset_nodes +=
            diagnostics.plan_nodes.planned_metric_workset_nodes;
        aggregate.plan_nodes.planned_response_artifact_nodes +=
            diagnostics.plan_nodes.planned_response_artifact_nodes;
        aggregate.plan_nodes.planned_dataframe_artifact_nodes +=
            diagnostics.plan_nodes.planned_dataframe_artifact_nodes;
        aggregate.plan_nodes.planned_total_nodes += diagnostics.plan_nodes.planned_total_nodes;
        aggregate.plan_nodes.canonical_prebuild_nodes +=
            diagnostics.plan_nodes.canonical_prebuild_nodes;
        aggregate.plan_nodes.budget.canonical_node_limit =
            diagnostics.plan_nodes.budget.canonical_node_limit;
        aggregate.plan_nodes.budget.startup_wall_ms_limit =
            diagnostics.plan_nodes.budget.startup_wall_ms_limit;
        aggregate.plan_nodes.budget.over_canonical_node_limit =
            aggregate.plan_nodes.budget.over_canonical_node_limit
                || diagnostics.plan_nodes.budget.over_canonical_node_limit;
        aggregate.critical_warmup.total_request_count +=
            diagnostics.critical_warmup.total_request_count;
        aggregate.critical_warmup.executed_request_count +=
            diagnostics.critical_warmup.executed_request_count;
        aggregate.critical_warmup.cache_hit_count += diagnostics.critical_warmup.cache_hit_count;
        aggregate.critical_warmup.total_ms += diagnostics.critical_warmup.total_ms;
        aggregate.critical_warmup.ok =
            aggregate.critical_warmup.ok || diagnostics.critical_warmup.ok;
        aggregate.deferred_warmup.total_request_count +=
            diagnostics.deferred_warmup.total_request_count;
        aggregate.deferred_warmup.executed_request_count +=
            diagnostics.deferred_warmup.executed_request_count;
        aggregate.deferred_warmup.cache_hit_count += diagnostics.deferred_warmup.cache_hit_count;
        aggregate.deferred_warmup.total_ms += diagnostics.deferred_warmup.total_ms;
        aggregate.deferred_warmup.ok =
            aggregate.deferred_warmup.ok || diagnostics.deferred_warmup.ok;
        slow_scopes.extend(diagnostics.slow_scopes.clone());
        slow_metrics.extend(diagnostics.slow_metrics.clone());
    }
    aggregate.expansion_ratio = if aggregate.unique_compile_result_count > 0 {
        aggregate.total_scope_checks as f64 / aggregate.unique_compile_result_count as f64
    } else {
        1.0
    };
    slow_scopes.sort_by_key(|entry| std::cmp::Reverse(entry.compile_ms));
    slow_scopes.truncate(8);
    slow_metrics.sort_by_key(|entry| std::cmp::Reverse(entry.ms));
    slow_metrics.truncate(8);
    aggregate.slow_scopes = slow_scopes;
    aggregate.slow_metrics = slow_metrics;
    aggregate
}

fn emit_prebuild_optimization_report(
    app_id: &str,
    app_root: &Path,
    reports: &[PrebuildScopeReport],
    coverage: &PrebuildCoverageReport,
    diagnostics: &PrebuildDiagnostics,
    plan_nodes: &PrebuildPlanNodeStatsReport,
    compile_phase_ms: u64,
    artifacts_phase_ms: u64,
    max_parallelism: usize,
    warning_count: usize,
    canonical_identity_count: usize,
    session_entries_before_clear: (usize, usize, usize),
    session_entries_after_clear: (usize, usize, usize),
    warmup_reuse_hits: usize,
) {
    diagnostics.sample_memory_peak();
    prebuild_emit_progress(format!("[{app_id}] ══ 优化诊断（重复 vs 耗时）══"));

    let total_checks = reports.len();
    let real_compiles = reports.iter().filter(|report| !report.cache_hit).count();
    let cache_hits = reports.iter().filter(|report| report.cache_hit).count();
    let cache_probe_ms: u64 = reports
        .iter()
        .filter(|report| report.cache_hit)
        .map(|report| {
            report
                .cache_lookup_ms
                .saturating_add(report.artifact_load_ms)
        })
        .sum();
    let compile_miss_ms: u64 = reports
        .iter()
        .filter(|report| !report.cache_hit)
        .map(|report| report.compile_ms)
        .sum();

    prebuild_emit_progress(format!(
        "■ 汇总 | scope 检查 {total_checks} | 真实编译 {real_compiles} | 缓存命中 {cache_hits} | 编译阶段 {compile_phase_s:.1}s | 产物阶段 {artifacts_phase_s:.1}s",
        compile_phase_s = compile_phase_ms as f64 / 1000.0,
        artifacts_phase_s = artifacts_phase_ms as f64 / 1000.0,
    ));
    prebuild_emit_progress(format!(
        "  时间构成 | 真实编译 {compile_miss_s:.1}s | 缓存探测约 {cache_probe_s:.1}s",
        compile_miss_s = compile_miss_ms as f64 / 1000.0,
        cache_probe_s = cache_probe_ms as f64 / 1000.0,
    ));
    prebuild_emit_progress(format!(
        "  产物 | response 就绪 {} (新建 {}) | dataframe 就绪 {} (本次计算 {})",
        coverage.metric_response_artifacts_ready,
        coverage.metric_response_artifacts_built,
        coverage.metric_dataframe_artifacts_ready,
        coverage.metric_dataframe_artifacts_built,
    ));

    let mut by_active: BTreeMap<String, (usize, usize, u64)> = BTreeMap::new();
    for report in reports {
        let entry = by_active
            .entry(compile_active_identity(report))
            .or_insert((0, 0, 0));
        entry.0 += 1;
        if report.cache_hit {
            entry.2 += report
                .cache_lookup_ms
                .saturating_add(report.artifact_load_ms);
        } else {
            entry.1 += 1;
            entry.2 += report.compile_ms;
        }
    }
    let unique_active = by_active.len();
    let expansion_ratio = if unique_active > 0 {
        total_checks as f64 / unique_active as f64
    } else {
        1.0
    };
    let redundant_checks = total_checks.saturating_sub(unique_active);
    prebuild_emit_progress(format!(
        "■ 数量统计 | 编译检查 {total_checks} | 唯一编译结果 {unique_active} | 展开倍率 {expansion_ratio:.1}x | 冗余检查约 {redundant_checks}"
    ));
    prebuild_emit_progress(format!(
        "  RSS 相关 | canonical outcomes {} | session(before) scope/cache/identity = {}/{}/{} | session(after) = {}/{}/{} | warmup 直接复用 {}",
        canonical_identity_count,
        session_entries_before_clear.0,
        session_entries_before_clear.1,
        session_entries_before_clear.2,
        session_entries_after_clear.0,
        session_entries_after_clear.1,
        session_entries_after_clear.2,
        warmup_reuse_hits
    ));
    prebuild_emit_progress(format!(
        "  DAG 计划 | manifest scope {} (hot {} / deferred {}) | warmup req {} / scope {} | workset {} | response {} | dataframe {} | canonical nodes {} / budget {}",
        plan_nodes.manifest_compile_scope_nodes,
        plan_nodes.hot_compile_scope_nodes,
        plan_nodes.deferred_compile_scope_nodes,
        plan_nodes.planned_warmup_request_nodes,
        plan_nodes.planned_warmup_scope_nodes,
        plan_nodes.planned_metric_workset_nodes,
        plan_nodes.planned_response_artifact_nodes,
        plan_nodes.planned_dataframe_artifact_nodes,
        plan_nodes.canonical_prebuild_nodes,
        plan_nodes.budget.canonical_node_limit
    ));
    if plan_nodes.budget.over_canonical_node_limit {
        prebuild_emit_progress(format!(
            "  预算告警 | canonical prebuild nodes {} 超过预算 {}，请继续收缩 manifest/fanout/workset",
            plan_nodes.canonical_prebuild_nodes,
            plan_nodes.budget.canonical_node_limit
        ));
    }
    let preload_reuse_hits = diagnostics
        .compile_preload_reuse_hits
        .load(Ordering::Relaxed);
    let postload_identity_collapses = diagnostics
        .compile_postload_identity_collapses
        .load(Ordering::Relaxed);
    let compile_index_hits = diagnostics.compile_index_hits.load(Ordering::Relaxed);
    let compile_index_misses = diagnostics.compile_index_misses.load(Ordering::Relaxed);
    let compile_index_stale_entries = diagnostics
        .compile_index_stale_entries
        .load(Ordering::Relaxed);
    let compile_fallback_loads = diagnostics.compile_fallback_loads.load(Ordering::Relaxed);
    if preload_reuse_hits > 0 {
        prebuild_emit_progress(format!(
            "  预加载复用 {preload_reuse_hits} 次（命中已知 scope/cache key，跳过探测/加载）"
        ));
    }
    if compile_index_hits > 0 || compile_index_misses > 0 || compile_index_stale_entries > 0 {
        prebuild_emit_progress(format!(
            "  compile 索引 | hit {} | miss {} | stale {} | fallback_loads {}",
            compile_index_hits,
            compile_index_misses,
            compile_index_stale_entries,
            compile_fallback_loads
        ));
    }
    let manifest_probes = diagnostics.compile_manifest_probes.load(Ordering::Relaxed);
    let manifest_stale_skips = diagnostics
        .compile_manifest_stale_skips
        .load(Ordering::Relaxed);
    let artifact_loads_avoided = diagnostics
        .compile_artifact_loads_avoided
        .load(Ordering::Relaxed);
    let mrg_eval_skips = diagnostics.mrg_eval_skips.load(Ordering::Relaxed);
    let dataframe_eval_skips = diagnostics.dataframe_eval_skips.load(Ordering::Relaxed);
    if manifest_probes > 0 || artifact_loads_avoided > 0 {
        prebuild_emit_progress(format!(
            "  manifest 探测 {manifest_probes} | stale 跳过 {manifest_stale_skips} | 避免全量 load {artifact_loads_avoided}"
        ));
    }
    if mrg_eval_skips > 0 || dataframe_eval_skips > 0 {
        prebuild_emit_progress(format!(
            "  MRG eval 跳过 response {mrg_eval_skips} | dataframe {dataframe_eval_skips}"
        ));
    }
    if postload_identity_collapses > 0 {
        prebuild_emit_progress(format!(
            "  load 后 identity 折叠 {postload_identity_collapses} 次（不同请求 scope 收敛到同一编译结果）"
        ));
    }
    prebuild_emit_progress(format!(
        "  逻辑产物 | compile {}/{} | 数据集导入 {}/{} | metric response {}/{} | metric dataframe {}/{} | missing {}",
        coverage.compile_artifacts_ready,
        coverage.compile_artifacts_planned,
        coverage.dataset_import_artifacts_ready,
        coverage.dataset_import_artifacts_planned,
        coverage.metric_response_artifacts_ready,
        coverage.metric_response_artifacts_planned,
        coverage.metric_dataframe_artifacts_ready,
        coverage.metric_dataframe_artifacts_planned,
        coverage.total_missing_artifacts,
    ));

    if prebuild_disk_diagnostics_enabled() {
        let eval_root = mei_lang_kernel::resolve_app_var_root(app_root).join("eval-results");
        let response_dir = eval_root.join("results").join("metric-response");
        let dataframe_dir = eval_root.join("results").join("metric-dataframe");
        let response_disk = dir_size_summary(response_dir.as_path());
        let dataframe_disk = dir_size_summary(dataframe_dir.as_path());
        let eval_disk = dir_size_summary(eval_root.as_path());
        prebuild_emit_progress(format!(
            "■ 磁盘占用 | eval-results 合计 {} ({} 文件)",
            format_bytes(eval_disk.bytes),
            eval_disk.files,
        ));
        prebuild_emit_progress(format!(
            "  metric-response {} ({} 文件) | metric-dataframe {} ({} 文件)",
            format_bytes(response_disk.bytes),
            response_disk.files,
            format_bytes(dataframe_disk.bytes),
            dataframe_disk.files,
        ));
    } else {
        prebuild_emit_progress(
            "■ 磁盘占用 | 已跳过目录扫描（设置 MEI_PREBUILD_DISK_DIAGNOSTICS=1 可启用）",
        );
    }

    let current_rss = current_process_rss_bytes();
    let peak_rss = diagnostics.peak_rss_bytes.load(Ordering::Relaxed);
    match (current_rss, peak_rss) {
        (Some(current), peak) if peak > 0 => {
            prebuild_emit_progress(format!(
                "■ 内存 | 进程 RSS 当前 {} | 峰值 {}",
                format_bytes(current),
                format_bytes(peak as u64),
            ));
        }
        (Some(current), _) => {
            prebuild_emit_progress(format!("■ 内存 | 进程 RSS 当前 {}", format_bytes(current),));
        }
        (None, peak) if peak > 0 => {
            prebuild_emit_progress(format!(
                "■ 内存 | 进程 RSS 峰值 {}",
                format_bytes(peak as u64),
            ));
        }
        _ => {}
    }

    let mut duplicates: Vec<_> = by_active
        .into_iter()
        .filter(|(_, (count, _, _))| *count > 1)
        .collect();
    duplicates.sort_by_key(|(_, (count, _, _))| std::cmp::Reverse(*count));
    if duplicates.is_empty() {
        prebuild_emit_progress("■ 重复检查 | 无（每个编译结果仅检查 1 次）".to_string());
    } else {
        prebuild_emit_progress(format!(
            "■ 重复检查 Top {}（同 scene+file 被多次处理；优化方向：减少 discover 展开）",
            duplicates.len().min(10)
        ));
        for (identity, (count, miss_count, cost_ms)) in duplicates.into_iter().take(10) {
            let (scene, file) = identity
                .split_once('|')
                .map(|(scene, file)| (scene, file))
                .unwrap_or((identity.as_str(), ""));
            prebuild_emit_progress(format!(
                "  {count}x | scene={scene} | file={file} | 真实编译 {miss_count} | 累计 {:.1}s",
                cost_ms as f64 / 1000.0
            ));
        }
    }

    let mut miss_by_file: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for report in reports.iter().filter(|report| !report.cache_hit) {
        let file = report.active_target_file.as_str();
        let entry = miss_by_file.entry(file.to_string()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += report.compile_ms;
    }
    let mut repeat_miss: Vec<_> = miss_by_file
        .into_iter()
        .filter(|(_, (count, _))| *count > 1)
        .collect();
    repeat_miss.sort_by_key(|(_, (count, _))| std::cmp::Reverse(*count));
    if repeat_miss.is_empty() {
        prebuild_emit_progress("■ 重复真实编译 | 无（同一文件未重复编译）".to_string());
    } else {
        prebuild_emit_progress("■ 重复真实编译（应优先消除）".to_string());
        for (file, (count, ms)) in repeat_miss.into_iter().take(8) {
            prebuild_emit_progress(format!(
                "  {count}x | file={file} | 合计 {:.1}s",
                ms as f64 / 1000.0
            ));
        }
    }

    let mut slow_compiles: Vec<&PrebuildScopeReport> = reports
        .iter()
        .filter(|report| !report.cache_hit && report.compile_ms > 0)
        .collect();
    slow_compiles.sort_by_key(|report| std::cmp::Reverse(report.compile_ms));
    if slow_compiles.is_empty() {
        prebuild_emit_progress("■ 编译最慢 | 无真实编译（全部缓存命中）".to_string());
    } else {
        prebuild_emit_progress(format!(
            "■ 编译最慢 Top {}（优化 .mei / 减少 scope）",
            slow_compiles.len().min(8)
        ));
        emit_slow_compile_report(app_id, reports);
    }

    let metric_builds = diagnostics
        .metric_builds
        .lock()
        .expect("lock prebuild diagnostics")
        .clone();
    if metric_builds.is_empty() {
        prebuild_emit_progress("■ 指标求值最慢 | 无（本次未重新计算指标）".to_string());
    } else {
        let mut slow_metrics = metric_builds;
        slow_metrics.sort_by_key(|entry| std::cmp::Reverse(entry.ms));
        prebuild_emit_progress(format!(
            "■ 指标求值最慢 Top {}（优化 metric 口径 / 数据加载）",
            slow_metrics.len().min(8)
        ));
        for entry in slow_metrics.into_iter().take(8) {
            prebuild_emit_progress(format!(
                "  {:.1}s | {} | {} | metric={} | scene={}",
                entry.ms as f64 / 1000.0,
                entry.kind,
                entry.dataset,
                entry.metric,
                entry.scene
            ));
        }
    }

    let cpu_count = thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1);
    let parallelism_cap = prebuild_max_parallelism_cap();
    let home_compile_ms = reports
        .iter()
        .filter(|report| {
            !report.cache_hit
                && report
                    .active_target_file
                    .as_str()
                    .ends_with("scenes/home.mei")
        })
        .map(|report| report.compile_ms)
        .sum::<u64>();
    let home_compile_share = if compile_miss_ms > 0 {
        home_compile_ms as f64 * 100.0 / compile_miss_ms as f64
    } else {
        0.0
    };

    prebuild_emit_progress("■ 提速建议（按收益排序）".to_string());
    if expansion_ratio >= 2.0
        && redundant_checks > 0
        && compile_index_hits == 0
        && preload_reuse_hits == 0
    {
        prebuild_emit_progress(format!(
            "  1. [高] discover 展开 {expansion_ratio:.1}x：{total_checks} 次检查仅 {unique_active} 种结果，合并同源 scope 约可省 {:.0}s 缓存探测",
            cache_probe_ms as f64 / 1000.0 * redundant_checks as f64 / total_checks as f64
        ));
    } else if preload_reuse_hits > 0 || postload_identity_collapses > 0 || compile_index_hits > 0 {
        prebuild_emit_progress(format!(
            "  1. [已启用] 结果复用已消化重复检查（预加载复用 {preload_reuse_hits} / compile索引命中 {compile_index_hits} / load后折叠 {postload_identity_collapses}）；增量场景用 prebuild --verify 可进一步压到秒级"
        ));
    }
    if home_compile_ms > 0 {
        prebuild_emit_progress(format!(
            "  2. [高] scenes/home.mei 真实编译 {:.1}s（占真实编译 {home_compile_share:.0}%）→ 精简首页或拆分重模块",
            home_compile_ms as f64 / 1000.0
        ));
    }
    if max_parallelism < cpu_count && max_parallelism < parallelism_cap {
        prebuild_emit_progress(format!(
            "  3. [中] 当前 {max_parallelism} 路并行（本机 {cpu_count} 核）→ 可设 MEI_PREBUILD_MAX_PARALLELISM={} 再跑",
            cpu_count.min(16)
        ));
    } else if parallelism_cap == PREBUILD_MAX_PARALLELISM && cpu_count > PREBUILD_MAX_PARALLELISM {
        prebuild_emit_progress(format!(
            "  3. [中] 本机 {cpu_count} 核，可试 MEI_PREBUILD_MAX_PARALLELISM=16（当前上限 {PREBUILD_MAX_PARALLELISM}）"
        ));
    }
    prebuild_emit_progress(
        "  4. [中] 使用 release 构建：cargo build --release -p mei-lang-server（debug 编译通常慢 2-3x）"
            .to_string(),
    );
    prebuild_emit_progress(
        "  5. [中] 未改 .mei 时用 prebuild --verify（秒级校验，跳过全量重算）".to_string(),
    );
    if warning_count > 0 {
        prebuild_emit_progress(format!(
            "  6. [低] 修复 {warning_count} 条 warning（失败 scope 会拖慢产物阶段并可能重复重试）"
        ));
    }
}

fn is_script_target(path: &str) -> bool {
    path.ends_with(".mei")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrebuildMode {
    Build,
    Verify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrebuildScopeProfile {
    Full,
    HotOnly,
}

#[derive(Debug, Clone)]
pub struct PrebuildOptions {
    pub app_filter: Option<String>,
    pub mode: PrebuildMode,
    pub clean: bool,
    pub force_rebuild: bool,
    pub scope_profile: PrebuildScopeProfile,
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
    pub critical_warmup_requests_ms: u64,
    pub deferred_warmup_requests_ms: u64,
    pub critical_warmup_request_count: usize,
    pub deferred_warmup_request_count: usize,
    pub max_parallelism: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildCoverageReport {
    pub compile_artifacts_planned: usize,
    pub compile_artifacts_ready: usize,
    pub compile_artifacts_missing: usize,
    pub dataset_import_artifacts_planned: usize,
    pub dataset_import_artifacts_ready: usize,
    pub dataset_import_artifacts_missing: usize,
    pub metric_response_artifacts_planned: usize,
    pub metric_response_artifacts_ready: usize,
    pub metric_response_artifacts_built: usize,
    #[serde(default)]
    pub metric_response_artifacts_skipped_bundle_unchanged: usize,
    pub metric_response_artifacts_missing: usize,
    pub metric_dataframe_artifacts_planned: usize,
    pub metric_dataframe_artifacts_ready: usize,
    pub metric_dataframe_artifacts_built: usize,
    #[serde(default)]
    pub metric_dataframe_artifacts_skipped_bundle_unchanged: usize,
    pub metric_dataframe_artifacts_missing: usize,
    pub total_missing_artifacts: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildDiskUsageReport {
    pub files: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildEvalArtifactDiskReport {
    pub total: PrebuildDiskUsageReport,
    pub metric_response: PrebuildDiskUsageReport,
    pub metric_dataframe: PrebuildDiskUsageReport,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildCompileIndexStatsReport {
    pub preload_reuse_hits: usize,
    pub postload_identity_collapses: usize,
    pub hits: usize,
    pub misses: usize,
    pub stale_entries: usize,
    pub fallback_loads: usize,
    #[serde(default)]
    pub manifest_probes: usize,
    #[serde(default)]
    pub manifest_stale_skips: usize,
    #[serde(default)]
    pub artifact_loads_avoided: usize,
    #[serde(default)]
    pub mrg_eval_skips: usize,
    #[serde(default)]
    pub dataframe_eval_skips: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildSessionEntryStatsReport {
    pub scope_entries: usize,
    pub cache_entries: usize,
    pub identity_entries: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildNodeBudgetReport {
    pub canonical_node_limit: usize,
    pub startup_wall_ms_limit: u64,
    pub over_canonical_node_limit: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildPlanNodeStatsReport {
    pub manifest_compile_scope_nodes: usize,
    pub hot_compile_scope_nodes: usize,
    pub deferred_compile_scope_nodes: usize,
    pub planned_warmup_request_nodes: usize,
    pub planned_warmup_scope_nodes: usize,
    pub planned_metric_workset_nodes: usize,
    pub planned_response_artifact_nodes: usize,
    pub planned_dataframe_artifact_nodes: usize,
    pub planned_total_nodes: usize,
    pub canonical_prebuild_nodes: usize,
    pub budget: PrebuildNodeBudgetReport,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildSlowScopeDiagnostic {
    pub scene_id: Option<String>,
    pub target_file: String,
    pub compile_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildSlowMetricDiagnostic {
    pub kind: String,
    pub dataset: String,
    pub metric: String,
    pub scene: String,
    pub ms: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildWarmupDiagnosticReport {
    pub total_request_count: usize,
    pub executed_request_count: usize,
    pub cache_hit_count: usize,
    pub total_ms: u64,
    pub ok: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PrebuildDiagnosticsReport {
    pub total_scope_checks: usize,
    pub real_compile_count: usize,
    pub cache_hit_count: usize,
    pub unique_compile_result_count: usize,
    pub canonical_identity_count: usize,
    pub redundant_scope_checks: usize,
    pub expansion_ratio: f64,
    pub cache_probe_ms: u64,
    pub compile_miss_ms: u64,
    pub current_rss_bytes: Option<u64>,
    pub peak_rss_bytes: u64,
    pub eval_artifacts_disk: PrebuildEvalArtifactDiskReport,
    pub compile_index: PrebuildCompileIndexStatsReport,
    pub session_before_clear: PrebuildSessionEntryStatsReport,
    pub session_after_clear: PrebuildSessionEntryStatsReport,
    pub warmup_reuse_hits: usize,
    pub plan_nodes: PrebuildPlanNodeStatsReport,
    pub critical_warmup: PrebuildWarmupDiagnosticReport,
    pub deferred_warmup: PrebuildWarmupDiagnosticReport,
    pub slow_scopes: Vec<PrebuildSlowScopeDiagnostic>,
    pub slow_metrics: Vec<PrebuildSlowMetricDiagnostic>,
    #[serde(default)]
    pub fingerprint_skip: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildWarningReport {
    pub phase: String,
    pub category: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset_selector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
    pub error: String,
}

impl PrebuildWarningReport {
    pub fn display_message(&self) -> &str {
        self.message.as_str()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildAppReport {
    pub app_id: String,
    pub compile_scopes: Vec<PrebuildScopeReport>,
    pub coverage: PrebuildCoverageReport,
    pub timings: PrebuildTimingReport,
    pub data_snapshots: Option<PublishDataSnapshotsReport>,
    pub diagnostics: PrebuildDiagnosticsReport,
    pub warnings: Vec<PrebuildWarningReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildReport {
    pub schema_version: String,
    pub mode: PrebuildMode,
    pub scope_profile: PrebuildScopeProfile,
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
    pub diagnostics: PrebuildDiagnosticsReport,
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
    pub diagnostics: PrebuildDiagnosticsReport,
    pub warnings: Vec<PrebuildWarningReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrebuildReportSummary {
    pub schema_version: String,
    pub mode: PrebuildMode,
    pub scope_profile: PrebuildScopeProfile,
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
    pub diagnostics: PrebuildDiagnosticsReport,
    pub apps: Vec<PrebuildAppSummary>,
}

impl PrebuildReport {
    pub fn warning_categories(&self) -> Vec<String> {
        let mut categories = self
            .apps
            .iter()
            .flat_map(|app| app.warnings.iter().map(|warning| warning.category.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        categories.sort();
        categories
    }

    pub fn warning_category_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::<String, usize>::new();
        for warning in self.apps.iter().flat_map(|app| app.warnings.iter()) {
            *counts.entry(warning.category.clone()).or_insert(0) += 1;
        }
        counts
    }

    pub fn failing_datasets(&self) -> Vec<String> {
        let mut datasets = self
            .apps
            .iter()
            .flat_map(|app| {
                app.warnings
                    .iter()
                    .filter_map(|warning| warning.dataset_selector.clone())
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        datasets.sort();
        datasets
    }

    pub fn correctness_failed(&self) -> bool {
        !self.ok
            || self
                .apps
                .iter()
                .any(|app| !app.warnings.is_empty())
            || !self.failed_apps.is_empty()
    }

    pub fn summary(&self) -> PrebuildReportSummary {
        PrebuildReportSummary {
            schema_version: self.schema_version.clone(),
            mode: self.mode,
            scope_profile: self.scope_profile,
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
            diagnostics: self.diagnostics.clone(),
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
                    diagnostics: app.diagnostics.clone(),
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
    priority: WarmupRequestPriority,
    metric_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct PrebuildManifestPlan {
    initial_scope_count: usize,
    hot_scopes: Vec<CompileScope>,
    deferred_scopes: Vec<CompileScope>,
    warmup_requests: Vec<AggregatedWarmupRequest>,
}

#[derive(Debug, Clone)]
struct PlannedMetricWorkset {
    logical_node_id: String,
    scope_id: String,
    materialization_key: String,
    dataset_selector: String,
    owner_resource_id: String,
    requested_metric_ids: Vec<String>,
    request_all_metrics: bool,
    scene_id: String,
    scene_path: Option<String>,
    dependency_revision_key: String,
    response_cache_key: String,
    shared_cache_key: String,
    covered_metric_ids: BTreeSet<String>,
    defs_for_hydrate: Arc<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone)]
struct PlannedDataframeArtifact {
    logical_node_id: String,
    scope_id: String,
    materialization_key: String,
    artifact_key: String,
    shared_artifact_key: String,
    owner_resource_id: String,
    resource_selector_id: String,
    dataframe_metric_id: String,
    resolved_metric_id: String,
    page_size: usize,
    scene_id: String,
    scene_path: Option<String>,
    dependency_revision_key: String,
    scope_metric_token: String,
    defs_for_hydrate: Arc<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone)]
struct ScopeArtifactPlan {
    metric_worksets: Vec<PlannedMetricWorkset>,
    dataframe_artifacts: Vec<PlannedDataframeArtifact>,
}

#[derive(Debug, Clone)]
struct WarmupScopeBatch<'a> {
    scope: CompileScope,
    requests: Vec<&'a AggregatedWarmupRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum WarmupRequestPriority {
    Critical,
    Deferred,
}

fn warning_quoted_value(error: &str, marker: &str) -> Option<String> {
    let start = error.find(marker)? + marker.len();
    let rest = error.get(start..)?;
    let end = rest.find('`')?;
    let value = rest.get(..end)?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn warning_category_from_error(error: &str) -> (&'static str, Option<String>, Option<String>) {
    if error.contains("locate warmup dataset `") {
        return (
            "warmup_dataset_locate_failed",
            warning_quoted_value(error, "locate warmup dataset `"),
            None,
        );
    }
    if error.contains("build metric response artifact for dataset `") {
        return (
            "metric_response_eval_failed",
            warning_quoted_value(error, "build metric response artifact for dataset `"),
            None,
        );
    }
    if error.contains("build metric dataframe artifact for dataset `") {
        return (
            "metric_dataframe_eval_failed",
            warning_quoted_value(error, "build metric dataframe artifact for dataset `"),
            warning_quoted_value(error, "metric `"),
        );
    }
    if error.contains("does not cover all declared metrics") {
        return (
            "artifact_coverage_miss",
            warning_quoted_value(error, "dataset `"),
            None,
        );
    }
    if error.contains("missing metric response artifact")
        || error.contains("missing metric dataframe artifact")
    {
        return ("artifact_index_miss", warning_quoted_value(error, "dataset `"), None);
    }
    if error.contains("metric response index preload failed") {
        return ("metric_response_index_preload_failed", None, None);
    }
    ("prebuild_warning", None, None)
}

fn build_prebuild_warning(
    phase: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
    dataset_selector: Option<&str>,
    metric_id: Option<&str>,
    compile_revision: Option<&str>,
    cache_key: Option<&str>,
    error: impl Into<String>,
) -> PrebuildWarningReport {
    let error = error.into();
    let (category, inferred_dataset, inferred_metric) = warning_category_from_error(error.as_str());
    let scene_id = scene_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let target_file = target_file
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let dataset_selector = dataset_selector
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or(inferred_dataset);
    let metric_id = metric_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or(inferred_metric);
    let compile_revision = compile_revision
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let cache_key = cache_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let message = match (scene_id.as_deref(), target_file.as_deref(), dataset_selector.as_deref()) {
        (Some(scene), Some(target), Some(dataset)) => {
            format!("{phase} scene=`{scene}` target=`{target}` dataset=`{dataset}` failed: {error}")
        }
        (Some(scene), Some(target), None) => {
            format!("{phase} scene=`{scene}` target=`{target}` failed: {error}")
        }
        (Some(scene), None, Some(dataset)) => {
            format!("{phase} scene=`{scene}` dataset=`{dataset}` failed: {error}")
        }
        (None, None, Some(dataset)) => format!("{phase} dataset=`{dataset}` failed: {error}"),
        _ => format!("{phase} failed: {error}"),
    };
    PrebuildWarningReport {
        phase: phase.to_string(),
        category: category.to_string(),
        message,
        scene_id,
        target_file,
        dataset_selector,
        metric_id,
        compile_revision,
        cache_key,
        error,
    }
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

    fn to_world_scope(&self) -> WorldScope {
        let canonical = self.canonicalized();
        WorldScope {
            scene_id: canonical.requested_scene_id,
            target_file: canonical.requested_target_file,
        }
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

struct CoverageState {
    metric_response_jobs: ArtifactSingleflightState,
    metric_dataframe_jobs: ArtifactSingleflightState,
    metric_response_exact: Arc<Mutex<BTreeMap<String, LoadedMetricResponseArtifact>>>,
    metric_response_shared: Arc<Mutex<BTreeMap<String, LoadedMetricResponseArtifact>>>,
    metric_dataframe_exact: Arc<Mutex<BTreeMap<String, DatasetQueryResult>>>,
    metric_dataframe_shared: Arc<Mutex<BTreeMap<String, DatasetQueryResult>>>,
    diagnostics: Arc<PrebuildDiagnostics>,
    /// MetricDefBundle revisions captured before compile (MCG P1 skip).
    pre_mcg_bundle_revisions: BTreeMap<String, String>,
    source_root: Option<std::path::PathBuf>,
    app_id: Option<String>,
}

impl Default for CoverageState {
    fn default() -> Self {
        Self {
            metric_response_jobs: ArtifactSingleflightState::default(),
            metric_dataframe_jobs: ArtifactSingleflightState::default(),
            metric_response_exact: Arc::new(Mutex::new(BTreeMap::new())),
            metric_response_shared: Arc::new(Mutex::new(BTreeMap::new())),
            metric_dataframe_exact: Arc::new(Mutex::new(BTreeMap::new())),
            metric_dataframe_shared: Arc::new(Mutex::new(BTreeMap::new())),
            diagnostics: Arc::new(PrebuildDiagnostics::default()),
            pre_mcg_bundle_revisions: BTreeMap::new(),
            source_root: None,
            app_id: None,
        }
    }
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

    fn clear(&self) {
        let mut state = self.state.lock().expect("lock prebuild singleflight");
        state.inflight.clear();
        state.completed.clear();
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

    fn clear(&self) {
        self.metric_response_exact
            .lock()
            .expect("lock prebuild response exact cache")
            .clear();
        self.metric_response_shared
            .lock()
            .expect("lock prebuild response shared cache")
            .clear();
        self.metric_dataframe_exact
            .lock()
            .expect("lock prebuild dataframe exact cache")
            .clear();
        self.metric_dataframe_shared
            .lock()
            .expect("lock prebuild dataframe shared cache")
            .clear();
        self.metric_response_jobs.clear();
        self.metric_dataframe_jobs.clear();
    }
}

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

fn merge_coverage(target: &mut PrebuildCoverageReport, delta: &PrebuildCoverageReport) {
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

fn finalize_coverage_report(coverage: &mut PrebuildCoverageReport) {
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

fn run_prebuild_for_app(
    source_root: &Path,
    app: &RuntimeWarmupApp,
    mode: PrebuildMode,
    scope_profile: PrebuildScopeProfile,
) -> Result<PrebuildAppReport> {
    let app_started = Instant::now();
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
    let compile_session = Arc::new(Mutex::new(PrebuildCompileSession::default()));
    let manifest_plan = build_prebuild_manifest_plan(app, scope_profile);
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
        "[{}] ── 1/3 编译 .mei ── 约 {initial_scope_count} 个 manifest scope（request-scope 闭包 + 结果复用）",
        app.app_id
    ));
    let hot_scopes = manifest_plan.hot_scopes.clone();
    let deferred_scopes = manifest_plan.deferred_scopes.clone();
    let default_started = Instant::now();
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
        None => ensure_compile_scope_for_prebuild(
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
    let hot_total = hot_scopes.len();
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
            ensure_compile_scope_for_prebuild(
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
        let unique_key_total = representatives.len();
        let batch_results = run_limited_parallel_ordered_with_hook(
            representatives.clone(),
            max_parallelism,
            |scope| {
                ensure_compile_scope_for_prebuild(
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
                    0,
                    batch_new_hook.load(Ordering::Relaxed),
                    batch_cache_hook.load(Ordering::Relaxed),
                    false,
                    last_emit_hook.as_ref(),
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
                compile_session.as_ref(),
                &representative,
                outcome,
                &mut seen_scopes,
                &mut pending,
                &mut prepared_outcomes,
                &mut compile_reports,
            );
            for alias in aliases {
                let alias_outcome = scope_assembled_outcome(outcome, &alias);
                compile_session
                    .lock()
                    .expect("prebuild compile session lock")
                    .register(source_root, app.app_id.as_str(), &alias, alias_outcome.clone());
                record_prebuild_scope_compile(
                    compile_session.as_ref(),
                    &alias,
                    &alias_outcome,
                    &mut seen_scopes,
                    &mut pending,
                    &mut prepared_outcomes,
                    &mut compile_reports,
                );
            }
        }
        prebuild_emit_progress(&format!(
            "[{}] 编译 batch-{batch_idx} 完成 {:.1}s | 新编译 {batch_compiled} | 缓存 {batch_cache_hit} | 待发现队列 {}",
            app.app_id,
            batch_started.elapsed().as_secs_f64(),
            pending.len()
        ));
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
    let compile_scopes_ms = compile_started.elapsed().as_millis() as u64;
    diagnostics.sample_memory_peak();
    prebuild_emit_progress(&format!(
        "[{}] ── 1/3 编译完成 {:.1}s | 共 {} scope ──",
        app.app_id,
        compile_scopes_ms as f64 / 1000.0,
        compile_reports.len()
    ));
    let required_xlsx_sources = collect_required_xlsx_sources(
        app,
        unique_prepared_outcomes_for_artifacts(&prepared_outcomes)
            .iter()
            .map(|prepared| prepared.outcome.compiled.as_ref()),
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
    coverage.dataset_import_artifacts_planned = required_xlsx_sources.len();
    coverage.dataset_import_artifacts_ready = required_xlsx_sources.len();
    let _ = mei_lang_kernel::clear_runtime_eval_node_cache();
    let coverage_state = CoverageState {
        diagnostics: Arc::clone(&diagnostics),
        pre_mcg_bundle_revisions,
        source_root: Some(source_root.to_path_buf()),
        app_id: Some(app.app_id.clone()),
        ..CoverageState::default()
    };
    let artifact_outcomes = unique_prepared_outcomes_for_artifacts(&prepared_outcomes);
    let canonical_identity_count = artifact_outcomes.len();
    let session_entries_before_clear = if let Ok(session) = compile_session.lock() {
        (
            session.by_scope_key.len(),
            session.by_compile_cache_key.len(),
            session.by_identity.len(),
        )
    } else {
        (0, 0, 0)
    };
    let session_entries_after_clear = if let Ok(mut session) = compile_session.lock() {
        session.clear_runtime_maps();
        (
            session.by_scope_key.len(),
            session.by_compile_cache_key.len(),
            session.by_identity.len(),
        )
    } else {
        (0, 0, 0)
    };
    drop(compile_session);
    drop(prepared_outcomes);
    if std::env::var("MEI_PREBUILD_EVICTION")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .is_none_or(|value| !matches!(value.as_str(), "0" | "false" | "no" | "off"))
        && crate::graph::feature::graph_registry_dedup_enabled()
    {
        let _ = toolchain::clear_compile_cache_for_app(source_root, app.app_id.as_str());
    }
    let artifact_outcomes_for_warmup = artifact_outcomes.clone();
    let mut scope_artifact_plans = Vec::with_capacity(artifact_outcomes_for_warmup.len());
    for prepared in &artifact_outcomes_for_warmup {
        let matching_requests = matching_warmup_requests_for_outcome(&warmup_requests, &prepared.outcome);
        scope_artifact_plans.push(build_scope_artifact_plan(
            app.app_id.as_str(),
            app_root.as_path(),
            &prepared.scope,
            &prepared.outcome,
            matching_requests.as_slice(),
        )?);
    }
    let plan_nodes = build_plan_node_stats(
        &manifest_plan,
        canonical_identity_count,
        scope_artifact_plans.as_slice(),
    );
    coverage.compile_artifacts_planned = initial_scope_count;
    coverage.compile_artifacts_ready = compile_reports.len();
    coverage.metric_response_artifacts_planned = plan_nodes.planned_response_artifact_nodes;
    coverage.metric_dataframe_artifacts_planned = plan_nodes.planned_dataframe_artifact_nodes;
    let scope_artifacts_started = Instant::now();
    prebuild_emit_progress(&format!(
        "[{}] ── 2/3 生成 metric 产物 ── {} 个编译结果待处理（response + dataframe 落盘）",
        app.app_id,
        artifact_outcomes.len()
    ));
    let artifact_total = artifact_outcomes.len();
    let artifacts_started = Arc::new(Instant::now());
    let scope_results = run_limited_parallel_ordered_with_hook(
        artifact_outcomes
            .into_iter()
            .zip(scope_artifact_plans.clone())
            .collect(),
        max_parallelism,
        |(prepared, scope_plan)| {
            let mut local_coverage = PrebuildCoverageReport::default();
            let started = Instant::now();
            let result = ensure_scope_artifacts(
                app.app_id.as_str(),
                app_root.as_path(),
                &prepared.outcome,
                &scope_plan,
                mode,
                &mut local_coverage,
                &coverage_state,
            );
            (
                prepared.scope.clone(),
                result,
                local_coverage,
                started.elapsed(),
            )
        },
        {
            let app_id = app.app_id.clone();
            let done = Arc::new(AtomicUsize::new(0));
            let artifacts_started = Arc::clone(&artifacts_started);
            move |index,
                  (scope, result, local_coverage, wall_time): &(
                CompileScope,
                Result<()>,
                PrebuildCoverageReport,
                std::time::Duration,
            )| {
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                let scene = scope.requested_scene_id.clone().unwrap_or_default();
                let target = scope.requested_target_file.clone().unwrap_or_default();
                let file = format_scope_file(scene.as_str(), target.as_str(), None);
                let built_df = local_coverage.metric_dataframe_artifacts_built;
                let built_resp = local_coverage.metric_response_artifacts_built;
                if built_df > 0 || built_resp > 0 {
                    prebuild_emit_progress(format!(
                        "[{app_id}] 指标产物 {:.1}s | {n}/{artifact_total} | scene={scene} | file={file} | +{built_df} dataframe +{built_resp} response",
                        wall_time.as_secs_f64()
                    ));
                } else if result.is_err() {
                    prebuild_emit_progress(format!(
                        "[{app_id}] 指标产物失败 | {n}/{artifact_total} | scene={scene} | file={file}"
                    ));
                } else if n % 20 == 0 || n == artifact_total {
                    prebuild_emit_progress(format!(
                        "[{app_id}] 指标产物进度 {n}/{artifact_total} | 已用 {:.0}s（多数命中磁盘缓存）",
                        artifacts_started.elapsed().as_secs_f64()
                    ));
                }
                let _ = index;
            }
        },
    );
    for (scope, result, local_coverage, _wall_time) in scope_results {
        if let Err(error) = result {
            if mode == PrebuildMode::Verify {
                return Err(error);
            }
            warnings.push(build_prebuild_warning(
                "scope_artifacts",
                scope.requested_scene_id.as_deref(),
                scope.requested_target_file.as_deref(),
                None,
                None,
                None,
                None,
                error.to_string(),
            ));
        } else {
            merge_coverage(&mut coverage, &local_coverage);
        }
    }
    let scope_artifacts_ms = scope_artifacts_started.elapsed().as_millis() as u64;
    prebuild_emit_progress(&format!(
        "[{}] ── 2/3 产物完成 {:.1}s | response={} dataframe={} | 新建 dataframe {} 个 ──",
        app.app_id,
        scope_artifacts_ms as f64 / 1000.0,
        coverage.metric_response_artifacts_ready,
        coverage.metric_dataframe_artifacts_ready,
        coverage.metric_dataframe_artifacts_built
    ));
    coverage_state.clear();
    let _ = mei_lang_datasets::clear_all_metric_caches();
    let _ = mei_lang_kernel::clear_runtime_eval_node_cache();
    let warmup_reuse_hits = warmup_requests
        .iter()
        .filter(|request| {
            artifact_outcomes_for_warmup
                .iter()
                .any(|prepared| warmup_request_matches_outcome(request, &prepared.outcome))
        })
        .count();
    let warmup_requests_to_run = warmup_requests
        .iter()
        .filter(|request| {
            !artifact_outcomes_for_warmup
                .iter()
                .any(|prepared| warmup_request_matches_outcome(request, &prepared.outcome))
        })
        .collect::<Vec<_>>();
    let mut critical_warmup_requests = Vec::new();
    let mut deferred_warmup_requests = Vec::new();
    let critical_warmup_cache_hit_count = warmup_requests
        .iter()
        .filter(|request| {
            request.priority == WarmupRequestPriority::Critical
                && artifact_outcomes_for_warmup
                    .iter()
                    .any(|prepared| warmup_request_matches_outcome(request, &prepared.outcome))
        })
        .count();
    let deferred_warmup_cache_hit_count = warmup_requests
        .iter()
        .filter(|request| {
            request.priority == WarmupRequestPriority::Deferred
                && artifact_outcomes_for_warmup
                    .iter()
                    .any(|prepared| warmup_request_matches_outcome(request, &prepared.outcome))
        })
        .count();
    for request in warmup_requests_to_run {
        match request.priority {
            WarmupRequestPriority::Critical => critical_warmup_requests.push(request),
            WarmupRequestPriority::Deferred => deferred_warmup_requests.push(request),
        }
    }
    let critical_warmup_request_count = critical_warmup_requests.len();
    let deferred_warmup_request_count = deferred_warmup_requests.len();
    let critical_warmup_total_count =
        critical_warmup_request_count + critical_warmup_cache_hit_count;
    let deferred_warmup_total_count =
        deferred_warmup_request_count + deferred_warmup_cache_hit_count;
    let mut critical_warmup_ok = true;
    let mut deferred_warmup_ok = true;
    let run_and_merge_warmup = |label: &str,
                                requests: &[&AggregatedWarmupRequest],
                                ok_flag: &mut bool,
                                warnings: &mut Vec<PrebuildWarningReport>,
                                coverage: &mut PrebuildCoverageReport|
     -> Result<u64> {
        if requests.is_empty() {
            return Ok(0);
        }
        prebuild_emit_progress(&format!(
            "[{}] ── 3/3 warmup {label} ── {} requests ──",
            app.app_id,
            requests.len()
        ));
        let started = Instant::now();
        let results = run_warmup_request_batch(
            source_root,
            app.app_id.as_str(),
            app_root.as_path(),
            mode,
            components_root.as_path(),
            &coverage_state,
            requests,
            max_parallelism,
        );
        for (scope, dataset_results, local_coverage) in results {
            let scope = CompileScope {
                requested_scene_id: scope.requested_scene_id.clone(),
                requested_target_file: scope.requested_target_file.clone(),
            };
            for (dataset_id, result) in dataset_results {
                if let Err(error) = result {
                    *ok_flag = false;
                    if mode == PrebuildMode::Verify {
                        return Err(error);
                    }
                    warnings.push(build_prebuild_warning(
                        &format!("warmup_{label}"),
                        scope.requested_scene_id.as_deref(),
                        scope.requested_target_file.as_deref(),
                        Some(dataset_id.as_str()),
                        None,
                        None,
                        None,
                        error.to_string(),
                    ));
                }
            }
            merge_coverage(coverage, &local_coverage);
        }
        Ok(started.elapsed().as_millis() as u64)
    };
    let critical_warmup_requests_ms = run_and_merge_warmup(
        "critical",
        critical_warmup_requests.as_slice(),
        &mut critical_warmup_ok,
        &mut warnings,
        &mut coverage,
    )?;
    let deferred_warmup_requests_ms = run_and_merge_warmup(
        "deferred",
        deferred_warmup_requests.as_slice(),
        &mut deferred_warmup_ok,
        &mut warnings,
        &mut coverage,
    )?;
    coverage_state.clear();
    let _ = mei_lang_datasets::clear_all_metric_caches();
    let _ = mei_lang_kernel::clear_runtime_eval_node_cache();
    finalize_coverage_report(&mut coverage);
    if mode == PrebuildMode::Verify && coverage.total_missing_artifacts > 0 {
        anyhow::bail!(
            "prebuild coverage verify failed: missing artifacts total={} compile={} dataset_import={} metric_response={} metric_dataframe={}",
            coverage.total_missing_artifacts,
            coverage.compile_artifacts_missing,
            coverage.dataset_import_artifacts_missing,
            coverage.metric_response_artifacts_missing,
            coverage.metric_dataframe_artifacts_missing
        );
    }
    let warmup_requests_ms = critical_warmup_requests_ms + deferred_warmup_requests_ms;
    if let Err(error) =
        mei_lang_datasets::preload_prebuild_metric_response_index(app_root.as_path())
    {
        warnings.push(build_prebuild_warning(
            "post_prebuild",
            None,
            None,
            None,
            None,
            None,
            None,
            format!("metric response index preload failed: {error}"),
        ));
    }
    let diagnostics_report = build_prebuild_diagnostics_report(
        app_root.as_path(),
        compile_reports.as_slice(),
        diagnostics.as_ref(),
        plan_nodes.clone(),
        canonical_identity_count,
        session_entries_before_clear,
        session_entries_after_clear,
        warmup_reuse_hits,
        critical_warmup_total_count,
        critical_warmup_request_count,
        critical_warmup_cache_hit_count,
        critical_warmup_requests_ms,
        critical_warmup_ok,
        deferred_warmup_total_count,
        deferred_warmup_request_count,
        deferred_warmup_cache_hit_count,
        deferred_warmup_requests_ms,
        deferred_warmup_ok,
    );
    emit_prebuild_optimization_report(
        app.app_id.as_str(),
        app_root.as_path(),
        compile_reports.as_slice(),
        &coverage,
        diagnostics.as_ref(),
        &plan_nodes,
        compile_scopes_ms,
        scope_artifacts_ms,
        max_parallelism,
        warnings.len(),
        canonical_identity_count,
        session_entries_before_clear,
        session_entries_after_clear,
        warmup_reuse_hits,
    );
    let summary = crate::diagnostics::LastBuildSummary::from_prebuild_diagnostics(
        app.app_id.as_str(),
        &diagnostics_report,
    );
    if let Err(error) =
        crate::diagnostics::persist_last_build_summary(app_root.as_path(), &summary)
    {
        tracing::warn!(
            %error,
            app_id = %app.app_id,
            "failed to persist last build summary"
        );
    }
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
            critical_warmup_requests_ms,
            deferred_warmup_requests_ms,
            critical_warmup_request_count,
            deferred_warmup_request_count,
            max_parallelism,
        },
        data_snapshots,
        diagnostics: diagnostics_report,
        warnings,
    })
}

fn compile_scopes_for_app(
    app: &RuntimeWarmupApp,
    scope_profile: PrebuildScopeProfile,
) -> Vec<CompileScope> {
    let mut scopes = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push_scope = |scope: CompileScope| {
        let scope = scope.canonicalized();
        if seen.insert(scope.key()) {
            scopes.push(scope);
        }
    };
    push_scope(CompileScope::default_scope());
    let scene_ids = scene_ids_for_profile(app, scope_profile);
    let focus_targets = focus_targets_for_profile(app, scope_profile);
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
    for scene_id in hot_scene_ids(app) {
        for focus in &focus_targets {
            push_scope(CompileScope {
                requested_scene_id: Some(scene_id.clone()),
                requested_target_file: Some(focus.clone()),
            });
        }
    }
    for request in app
        .datasets
        .iter()
        .filter(|request| warmup_dataset_request_in_profile(app, request, scope_profile))
    {
        push_scope(warmup_request_scope(request));
    }
    scopes
}

fn build_prebuild_manifest_plan(
    app: &RuntimeWarmupApp,
    scope_profile: PrebuildScopeProfile,
) -> PrebuildManifestPlan {
    let warmup_requests = aggregate_warmup_requests(app, scope_profile);
    let default_scope = CompileScope::default_scope();
    let all_scopes = compile_scopes_for_app(app, scope_profile);
    let initial_scope_count = all_scopes.len();
    let hot_scope_keys = hot_scene_ids(app)
        .into_iter()
        .map(|scene| format!("{}|", scene.trim()))
        .filter(|key| key != "|")
        .collect::<BTreeSet<_>>();
    let (hot_scopes, deferred_scopes): (Vec<_>, Vec<_>) = all_scopes
        .into_iter()
        .filter(|scope| scope.key() != default_scope.key())
        .partition(|scope| hot_scope_keys.contains(&scope.key()));
    PrebuildManifestPlan {
        initial_scope_count,
        hot_scopes,
        deferred_scopes,
        warmup_requests,
    }
}

fn hot_scene_ids(app: &RuntimeWarmupApp) -> Vec<String> {
    let mut scene_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for scene_id in app.default_scene.iter().chain(app.hot_scenes.iter()) {
        let scene_id = scene_id.trim();
        if scene_id.is_empty() || !seen.insert(scene_id.to_string()) {
            continue;
        }
        scene_ids.push(scene_id.to_string());
    }
    scene_ids
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

fn scene_ids_for_profile(
    app: &RuntimeWarmupApp,
    scope_profile: PrebuildScopeProfile,
) -> Vec<String> {
    match scope_profile {
        PrebuildScopeProfile::Full => explicit_scene_ids(app),
        PrebuildScopeProfile::HotOnly => hot_scene_ids(app),
    }
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
        if let Some(target) = warmup_request_target_file(request) {
            push(target.as_str());
        }
    }
    targets
}

fn warmup_dataset_selector_target_file(dataset_selector: &str) -> Option<String> {
    dataset_selector
        .split("::")
        .map(str::trim)
        .find(|segment| segment.starts_with("scenes/") && segment.ends_with(".mei"))
        .map(str::to_string)
}

fn warmup_request_target_file(request: &RuntimeWarmupDatasetRequest) -> Option<String> {
    request
        .focus
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| warmup_dataset_selector_target_file(request.dataset_id.as_str()))
}

fn warmup_request_scope(request: &RuntimeWarmupDatasetRequest) -> CompileScope {
    CompileScope {
        requested_scene_id: request.scene_id.clone(),
        requested_target_file: warmup_request_target_file(request),
    }
    .canonicalized()
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

fn focus_targets_for_profile(
    app: &RuntimeWarmupApp,
    scope_profile: PrebuildScopeProfile,
) -> Vec<String> {
    match scope_profile {
        PrebuildScopeProfile::Full => all_focus_targets(app),
        // Hot path should keep the explicit entry/main focus, but skip dataset-derived expansions.
        PrebuildScopeProfile::HotOnly => explicit_focus_targets(app),
    }
}

fn aggregate_warmup_requests(
    app: &RuntimeWarmupApp,
    scope_profile: PrebuildScopeProfile,
) -> Vec<AggregatedWarmupRequest> {
    let mut aggregated = BTreeMap::<String, AggregatedWarmupRequest>::new();
    for request in app
        .datasets
        .iter()
        .filter(|request| warmup_dataset_request_in_profile(app, request, scope_profile))
    {
        let scope = warmup_request_scope(request);
        let priority = warmup_request_priority(app, request);
        let metric_ids = requested_metric_ids(request);
        let request_all_metrics = metric_ids.is_empty();
        let key = format!("{}|{}", scope.key(), request.dataset_id.trim());
        if let Some(entry) = aggregated.get_mut(&key) {
            entry.priority = entry.priority.min(priority);
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
                priority,
                metric_ids,
            },
        );
    }
    aggregated.into_values().collect()
}

fn explicit_warmup_request_priority(
    request: &RuntimeWarmupDatasetRequest,
) -> Option<WarmupRequestPriority> {
    match request.priority.as_deref().map(str::trim) {
        Some("critical" | "hot") => Some(WarmupRequestPriority::Critical),
        Some("deferred" | "heavy" | "full") => Some(WarmupRequestPriority::Deferred),
        _ => None,
    }
}

fn warmup_request_priority(
    app: &RuntimeWarmupApp,
    request: &RuntimeWarmupDatasetRequest,
) -> WarmupRequestPriority {
    if let Some(priority) = explicit_warmup_request_priority(request) {
        return priority;
    }
    let hot_scenes = hot_scene_ids(app);
    let explicit_focuses = explicit_focus_targets(app);
    let request_scene = request
        .scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(scene_id) = request_scene {
        return if hot_scenes.iter().any(|value| value == scene_id) {
            WarmupRequestPriority::Critical
        } else {
            WarmupRequestPriority::Deferred
        };
    }
    if let Some(focus) = warmup_request_target_file(request) {
        return if explicit_focuses.iter().any(|value| value == &focus) {
            WarmupRequestPriority::Critical
        } else {
            WarmupRequestPriority::Deferred
        };
    }
    WarmupRequestPriority::Critical
}

fn warmup_dataset_request_in_profile(
    app: &RuntimeWarmupApp,
    request: &RuntimeWarmupDatasetRequest,
    scope_profile: PrebuildScopeProfile,
) -> bool {
    if scope_profile == PrebuildScopeProfile::Full {
        return true;
    }
    warmup_request_priority(app, request) == WarmupRequestPriority::Critical
}

pub(crate) fn app_has_deferred_warmup_work(app: &RuntimeWarmupApp) -> bool {
    let full = build_prebuild_manifest_plan(app, PrebuildScopeProfile::Full);
    let hot = build_prebuild_manifest_plan(app, PrebuildScopeProfile::HotOnly);
    (full.hot_scopes.len() + full.deferred_scopes.len())
        > (hot.hot_scopes.len() + hot.deferred_scopes.len())
        || full.warmup_requests.len() > hot.warmup_requests.len()
}

fn warmup_request_matches_outcome(
    request: &AggregatedWarmupRequest,
    outcome: &SharedCompileOutcome,
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
    if !mei_lang_kernel::locate_dataset_resource(&outcome.compiled, request.dataset_id.as_str()).is_ok()
        || !dataset_can_materialize_metric_artifacts(
            &outcome.compiled,
            request.dataset_id.as_str(),
        )
    {
        return false;
    }
    if request.metric_ids.is_empty() {
        return true;
    }
    request.metric_ids.iter().all(|metric_id| {
        locate_runtime_metric_resource(
            &outcome.compiled,
            request.dataset_id.as_str(),
            metric_id.as_str(),
        )
        .is_ok()
    })
}

fn matching_warmup_requests_for_outcome<'a>(
    requests: &'a [AggregatedWarmupRequest],
    outcome: &SharedCompileOutcome,
) -> Vec<&'a AggregatedWarmupRequest> {
    requests
        .iter()
        .filter(|request| warmup_request_matches_outcome(request, outcome))
        .collect()
}

fn group_warmup_requests_by_scope<'a>(
    requests: &[&'a AggregatedWarmupRequest],
) -> Vec<WarmupScopeBatch<'a>> {
    let mut grouped = BTreeMap::<String, WarmupScopeBatch<'a>>::new();
    for request in requests {
        grouped
            .entry(request.scope.key())
            .and_modify(|batch| batch.requests.push(*request))
            .or_insert_with(|| WarmupScopeBatch {
                scope: request.scope.clone(),
                requests: vec![*request],
            });
    }
    grouped.into_values().collect()
}

fn run_warmup_request_batch(
    source_root: &Path,
    app_id: &str,
    app_root: &Path,
    mode: PrebuildMode,
    components_root: &Path,
    coverage_state: &CoverageState,
    requests: &[&AggregatedWarmupRequest],
    max_parallelism: usize,
) -> Vec<(CompileScope, Vec<(String, Result<()>)>, PrebuildCoverageReport)> {
    let grouped_requests = group_warmup_requests_by_scope(requests);
    run_limited_parallel_ordered(grouped_requests, max_parallelism, |batch| {
        let scope = batch.scope.clone();
        let mut local_coverage = PrebuildCoverageReport::default();
        let mut results = Vec::with_capacity(batch.requests.len());
        let compiled = ensure_compile_scope(source_root, app_id, &scope, mode, components_root);
        match compiled {
            Ok(outcome) => {
                for request in batch.requests {
                    let result = ensure_request_artifacts_for_compiled(
                        app_id,
                        app_root,
                        &outcome,
                        request.dataset_id.as_str(),
                        request.metric_ids.as_slice(),
                        mode,
                        &mut local_coverage,
                        coverage_state,
                    );
                    results.push((request.dataset_id.clone(), result));
                }
            }
            Err(error) => {
                let error_text = error.to_string();
                for request in batch.requests {
                    results.push((
                        request.dataset_id.clone(),
                        Err(anyhow::anyhow!(error_text.clone())),
                    ));
                }
            }
        }
        (scope, results, local_coverage)
    })
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
            if target.ends_with(".board.mei") {
                for export_scene_id in compiled
                    .scene_projection_assembly_by_id
                    .keys()
                    .chain(compiled.scene_bindings_by_id.keys())
                {
                    let export_scene_id = export_scene_id.trim();
                    if export_scene_id.is_empty() || export_scene_id == active_scene {
                        continue;
                    }
                    push_scope(CompileScope {
                        requested_scene_id: Some(export_scene_id.to_string()),
                        requested_target_file: Some(target.to_string()),
                    });
                }
            }
        }
    } else if let Some(board_file) = scope
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|target| !target.is_empty() && target.ends_with(".board.mei"))
        .or_else(|| {
            active_target
                .ends_with(".board.mei")
                .then_some(active_target)
        })
    {
        for entry in compiled.build_board_index.boards.values() {
            if entry.board_file.trim() != board_file {
                continue;
            }
            push_scope(CompileScope {
                requested_scene_id: Some(entry.scene_id.clone()),
                requested_target_file: Some(board_file.to_string()),
            });
        }
    }
    scopes
}

#[derive(Clone)]
struct SharedCompileOutcome {
    compiled: Arc<CompiledApp>,
    cache_hit: bool,
    artifact_cache_hit: bool,
    compile_revision: String,
    cache_lookup_ms: u64,
    artifact_load_ms: u64,
    compile_ms: u64,
}

impl SharedCompileOutcome {
    fn from_shared(outcome: toolchain::CompileWithCacheOutcomeShared) -> Self {
        Self {
            compiled: outcome.compiled,
            cache_hit: outcome.cache_hit,
            artifact_cache_hit: outcome.artifact_cache_hit,
            compile_revision: outcome.compile_revision,
            cache_lookup_ms: outcome.cache_lookup_ms,
            artifact_load_ms: outcome.artifact_load_ms,
            compile_ms: outcome.compile_ms,
        }
    }
}

#[derive(Clone)]
struct PreparedCompileOutcome {
    scope: CompileScope,
    outcome: SharedCompileOutcome,
}

fn compiled_scope_identity(outcome: &SharedCompileOutcome) -> String {
    format!(
        "{}|{}|{}",
        outcome.compiled.active_scene.as_deref().unwrap_or_default(),
        outcome.compiled.active_target_file,
        outcome.compile_revision
    )
}

fn compiled_default_target_file(compiled: &CompiledApp) -> Option<&str> {
    compiled
        .scene_routes
        .iter()
        .find(|route| route.is_default)
        .or_else(|| compiled.scene_routes.iter().find(|route| route.scene_id.trim() == "home"))
        .map(|route| route.target_file.trim())
        .filter(|target| !target.is_empty())
}

fn compile_outcome_matches_scope(scope: &CompileScope, compiled: &CompiledApp) -> bool {
    let requested = scope.canonicalized();
    let active_scene = compiled
        .active_scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let active_target = compiled.active_target_file.trim();
    if let Some(scene_id) = requested
        .requested_scene_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if active_scene != Some(scene_id) {
            return false;
        }
    }
    if let Some(target_file) = requested
        .requested_target_file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if active_target != target_file {
            return false;
        }
    }
    if requested.requested_scene_id.is_none() && requested.requested_target_file.is_none() {
        if let Some(default_target) = compiled_default_target_file(compiled) {
            return active_target == default_target;
        }
    }
    true
}

#[derive(Default)]
struct PrebuildCompileSession {
    by_scope_key: BTreeMap<String, SharedCompileOutcome>,
    by_compile_cache_key: BTreeMap<String, SharedCompileOutcome>,
    by_identity: BTreeMap<String, SharedCompileOutcome>,
    discovered_scope_keys: BTreeSet<String>,
    /// Each `.board.mei` target is expanded at most once per prebuild compile phase.
    expanded_board_targets: BTreeSet<String>,
}

impl PrebuildCompileSession {
    fn register(
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
        self.by_identity.entry(identity).or_insert(outcome);
    }

    fn try_reuse(
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

    fn should_discover(&mut self, scope: &CompileScope) -> bool {
        self.discovered_scope_keys.insert(scope.key())
    }

    fn note_scope_alias(&mut self, scope: &CompileScope, outcome: &SharedCompileOutcome) {
        self.by_scope_key
            .entry(scope.key())
            .or_insert_with(|| outcome.clone());
    }

    fn clear_runtime_maps(&mut self) {
        self.by_scope_key.clear();
        self.by_compile_cache_key.clear();
        self.by_identity.clear();
    }

    fn filter_board_discovered_scopes(
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
        self.expanded_board_targets
            .insert(board_target.clone());
        discovered.to_vec()
    }
}

fn is_board_export_scope(scope: &CompileScope, board_file: &str) -> bool {
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

fn group_scopes_by_compile_cache_key(
    source_root: &Path,
    app_id: &str,
    scopes: Vec<CompileScope>,
) -> Vec<(CompileScope, Vec<CompileScope>)> {
    let mut groups: BTreeMap<String, (CompileScope, Vec<CompileScope>)> = BTreeMap::new();
    for scope in scopes {
        let cache_key = toolchain::compile_cache_key(source_root, app_id, &scope.to_options());
        match groups.get_mut(&cache_key) {
            Some((representative, aliases)) if representative.key() != scope.key() => {
                aliases.push(scope);
            }
            None => {
                groups.insert(cache_key, (scope, Vec::new()));
            }
            _ => {}
        }
    }
    groups.into_values().collect()
}

fn session_try_reuse(
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

struct PersistedCompileIndexReuse {
    outcome: SharedCompileOutcome,
    discovered_scopes: Vec<CompileScope>,
    observed_count: usize,
}

fn try_reuse_persisted_compile_index(
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

fn try_reuse_compile_scope_before_load(
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

fn mark_prebuild_session_reuse(outcome: &SharedCompileOutcome) -> SharedCompileOutcome {
    SharedCompileOutcome {
        compiled: Arc::clone(&outcome.compiled),
        cache_hit: true,
        artifact_cache_hit: true,
        compile_revision: outcome.compile_revision.clone(),
        cache_lookup_ms: 0,
        artifact_load_ms: 0,
        compile_ms: 0,
    }
}

fn ensure_compile_scope_for_prebuild(
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

fn record_prebuild_scope_compile_with_discovered(
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

fn record_prebuild_scope_compile(
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

fn unique_prepared_outcomes_for_artifacts(
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

fn ensure_compile_scope(
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
    outcome: &SharedCompileOutcome,
    plan: &ScopeArtifactPlan,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let mrg_registry = if crate::graph::feature::graph_registry_dedup_enabled() {
        state
            .source_root
            .as_deref()
            .zip(state.app_id.as_deref())
            .map(|(source_root, registry_app)| {
                crate::graph::load_mrg_registry(source_root, registry_app)
            })
    } else {
        None
    };
    let _dirty_frontier = mrg_registry
        .as_ref()
        .map(|registry| registry.dirty_slots().len())
        .unwrap_or(0);
    for workset in &plan.metric_worksets {
        if let Some(registry) = mrg_registry.as_ref() {
            if let Some(current_rev) = current_bundle_revision_for_plan(workset) {
                let scope_key = crate::graph::mrg_eval_scope_key(
                    workset.scene_id.as_str(),
                    workset.scene_path.as_deref(),
                );
                let mrg_covers = crate::graph::mrg_slot_covers_eval(
                    registry,
                    workset.owner_resource_id.as_str(),
                    current_rev.as_str(),
                    workset.dependency_revision_key.as_str(),
                    scope_key.as_str(),
                    workset.shared_cache_key.as_str(),
                ) && prebuild_metric_response_index_covers_key(
                    app_root,
                    &workset.shared_cache_key,
                    &workset.covered_metric_ids,
                    workset.request_all_metrics,
                )?;
                if mrg_covers {
                    state
                        .diagnostics
                        .mrg_eval_skips
                        .fetch_add(1, Ordering::Relaxed);
                    coverage.metric_response_artifacts_skipped_bundle_unchanged += 1;
                    coverage.metric_response_artifacts_ready += 1;
                    continue;
                }
            }
        }
        ensure_metric_response_artifact_for_plan(
            app_id,
            app_root,
            outcome,
            workset,
            mode,
            coverage,
            state,
        )?;
    }
    for dataframe in &plan.dataframe_artifacts {
        if let Some(registry) = mrg_registry.as_ref() {
            if let Some(current_rev) = current_dataframe_bundle_revision(dataframe) {
                let scope_key = crate::graph::mrg_eval_scope_key(
                    dataframe.scene_id.as_str(),
                    dataframe.scene_path.as_deref(),
                );
                let mrg_covers = crate::graph::mrg_slot_covers_dataframe_eval(
                    registry,
                    dataframe.owner_resource_id.as_str(),
                    current_rev.as_str(),
                    dataframe.dependency_revision_key.as_str(),
                    scope_key.as_str(),
                    dataframe.shared_artifact_key.as_str(),
                ) && (metric_dataframe_result_artifact_exists(app_root, &dataframe.shared_artifact_key)
                    || metric_dataframe_result_artifact_exists(app_root, &dataframe.artifact_key));
                if mrg_covers {
                    state
                        .diagnostics
                        .dataframe_eval_skips
                        .fetch_add(1, Ordering::Relaxed);
                    coverage.metric_dataframe_artifacts_skipped_bundle_unchanged += 1;
                    coverage.metric_dataframe_artifacts_ready += 1;
                    continue;
                }
            }
        }
        ensure_metric_dataframe_artifact_for_plan(
            app_root,
            outcome,
            dataframe,
            mode,
            coverage,
            state,
        )?;
    }
    Ok(())
}

fn ensure_request_artifacts_for_compiled(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
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
            let metric_groups = group_metric_ids_by_owner(
                &outcome.compiled,
                resource.id.as_str(),
                &response_metric_ids,
            )?;
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
        let mut dataframe_metrics = requested_dataframe_metric_ids(dataset, metric_ids);
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

fn compiled_has_world_metrics_runtime_defs(compiled: &CompiledApp) -> bool {
    compiled.resources.iter().any(|resource| {
        resource.dataset.as_ref().is_some_and(|dataset| {
            dataset.has_runtime_metric_defs() && is_world_metrics_resource(resource.id.as_str())
        })
    })
}

fn dataset_can_materialize_metric_artifacts(
    compiled: &CompiledApp,
    dataset_selector: &str,
) -> bool {
    let Ok(resource) = mei_lang_kernel::locate_dataset_resource(compiled, dataset_selector) else {
        return false;
    };
    let Some(dataset) = resource.dataset.as_ref() else {
        return false;
    };
    if dataset.has_runtime_metric_defs() {
        return true;
    }
    compiled_has_world_metrics_runtime_defs(compiled)
}

fn response_metric_ids(
    compiled: &mei_lang_kernel::CompiledApp,
    dataset: &DatasetView,
) -> Vec<String> {
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

fn requested_dataframe_metric_ids(dataset: &DatasetView, metric_ids: &[String]) -> Vec<String> {
    let mut ids = if metric_ids.is_empty() {
        dataframe_metric_ids(dataset)
            .into_iter()
            .collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };
    for metric_id in metric_ids {
        let metric_id = metric_id.trim();
        if metric_id.is_empty() {
            continue;
        }
        if metric_id.ends_with("::__scalar_rowset__") || metric_def_is_dataframe(dataset, metric_id)
        {
            ids.insert(metric_id.to_string());
        }
    }
    ids.into_iter().collect()
}

fn metric_def_is_dataframe(dataset: &DatasetView, metric_id: &str) -> bool {
    dataset
        .runtime_metric_defs
        .get(metric_id)
        .and_then(Value::as_object)
        .and_then(|map| map.get("shape"))
        .and_then(Value::as_str)
        .is_some_and(|shape| shape == "dataframe")
}

fn collect_contract_metric_ids(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let is_metric_key = matches!(
                    key.as_str(),
                    "metric_id"
                        | "table_metric_id"
                        | "detail_table_metric_id"
                        | "drilldown_table_metric_id"
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

fn artifact_scene_context_for_resource(
    compiled: &CompiledApp,
    resource_id: &str,
) -> (String, Option<String>) {
    let Some(target_file) =
        mei_lang_kernel::imported_capsule_path_from_world_metrics_resource_id(resource_id)
    else {
        return artifact_scene_context(compiled);
    };
    let scene_id = compiled
        .scene_routes
        .iter()
        .find(|route| route.target_file == target_file)
        .map(|route| route.scene_id.clone())
        .or_else(|| compiled.active_scene.clone())
        .unwrap_or_else(|| "default".to_string());
    (scene_id, Some(target_file))
}

fn scope_identity_key(scene_id: &str, scene_path: Option<&str>) -> String {
    compile_scope_key_from_parts(
        Some(scene_id),
        scene_path.map(str::trim).filter(|value| !value.is_empty()),
    )
}

fn request_metric_scope_token(metric_ids: &[String]) -> String {
    if metric_ids.is_empty() {
        "*".to_string()
    } else {
        metric_scope_cache_key(metric_ids)
    }
}

fn logical_metric_workset_id(app_id: &str, owner_resource_id: &str, metric_ids: &[String]) -> String {
    format!(
        "workset|app={app_id}|owner={owner_resource_id}|metrics={}",
        request_metric_scope_token(metric_ids)
    )
}

fn summarize_metric_ids(metric_ids: &[String]) -> String {
    if metric_ids.is_empty() {
        return "*".to_string();
    }
    let mut preview = metric_ids
        .iter()
        .take(6)
        .map(|metric_id| short_metric_id(metric_id).to_string())
        .collect::<Vec<_>>();
    if metric_ids.len() > 6 {
        preview.push(format!("+{} more", metric_ids.len() - 6));
    }
    preview.join(", ")
}

fn materialization_identity(
    logical_node_id: &str,
    scope_id: &str,
    dependency_revision_key: &str,
    compile_revision: &str,
) -> String {
    format!(
        "{logical_node_id}|scope={scope_id}|dependency={dependency_revision_key}|compile={compile_revision}"
    )
}

fn logical_dataframe_artifact_id(owner_resource_id: &str, metric_id: &str, page_size: usize) -> String {
    format!("dataframe|owner={owner_resource_id}|metric={metric_id}|page_size={page_size}")
}

fn plan_metric_workset(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    dataset_selector: &str,
    metric_ids: &[String],
) -> Result<PlannedMetricWorkset> {
    let request_all_metrics = metric_ids.is_empty();
    let access_plan = plan_access_metric_eval_for_ids(&outcome.compiled, dataset_selector, metric_ids)
        .with_context(|| {
            format!(
                "plan metric response artifact for dataset `{dataset_selector}` metrics [{}]",
                summarize_metric_ids(metric_ids)
            )
        })?;
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
    let defs_for_hydrate = Arc::new(runtime_workset.defs_for_hydrate);
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root,
        &outcome.compiled,
        &access_plan.owner.id,
        defs_for_hydrate.as_ref(),
    );
    let query_state = empty_query_state();
    let query = collect_all_query_options(&query_state);
    let (scene_id, scene_path) =
        artifact_scene_context_for_resource(&outcome.compiled, access_plan.owner.id.as_str());
    let scope_id = scope_identity_key(scene_id.as_str(), scene_path.as_deref());
    let logical_node_id =
        logical_metric_workset_id(app_id, access_plan.owner.id.as_str(), &access_plan.request_metric_ids);
    let materialization_key = materialization_identity(
        logical_node_id.as_str(),
        scope_id.as_str(),
        dependency_revision_key.as_str(),
        outcome.compile_revision.as_str(),
    );
    let response_cache_key = metric_response_cache_scope_key(
        app_id,
        scene_id.as_str(),
        scene_path.as_deref(),
        &access_plan.owner.id,
        &query,
        &outcome.compile_revision,
        &dependency_revision_key,
        &[],
        None,
    );
    let shared_cache_key = metric_response_prebuild_shared_key(
        app_id,
        &access_plan.owner.id,
        &query,
        &dependency_revision_key,
    );
    Ok(PlannedMetricWorkset {
        logical_node_id,
        scope_id,
        materialization_key,
        dataset_selector: dataset_selector.to_string(),
        owner_resource_id: access_plan.owner.id.clone(),
        requested_metric_ids: access_plan.request_metric_ids,
        request_all_metrics,
        scene_id,
        scene_path,
        dependency_revision_key,
        response_cache_key,
        shared_cache_key,
        covered_metric_ids,
        defs_for_hydrate,
    })
}

fn plan_dataframe_artifact(
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    resource: &LoadedResource,
    metric_id: &str,
    page_size: usize,
) -> Result<Option<PlannedDataframeArtifact>> {
    let Ok((owner_resource, resolved_metric_id)) =
        locate_runtime_metric_resource(&outcome.compiled, resource.id.as_str(), metric_id)
    else {
        return Ok(None);
    };
    let owner_dataset = owner_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
    let dataframe_metric_id =
        prebuild_dataframe_metric_selector(&owner_dataset.runtime_metric_defs, &resolved_metric_id);
    let runtime_workset = runtime_metric_workset(
        &owner_resource.id,
        std::slice::from_ref(&dataframe_metric_id),
        owner_dataset,
    );
    let defs_for_hydrate = Arc::new(runtime_workset.defs_for_hydrate);
    let dependency_revision_key = metric_request_revision_fingerprint_for_compiled(
        app_root,
        &outcome.compiled,
        &owner_resource.id,
        if owner_dataset.runtime_metric_defs.is_empty() {
            defs_for_hydrate.as_ref()
        } else {
            &owner_dataset.runtime_metric_defs
        },
    );
    let query_options = widget_dataframe_query_options(page_size);
    let (scene_id, scene_path) =
        artifact_scene_context_for_resource(&outcome.compiled, owner_resource.id.as_str());
    let scope_metric_token = dataframe_scope_metric_token(
        &outcome.compiled,
        resource.id.as_str(),
        dataframe_metric_id.as_str(),
    )
    .unwrap_or_else(|| metric_scope_cache_key(std::slice::from_ref(&dataframe_metric_id)));
    let artifact_key = metric_dataframe_result_cache_key(
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
    let shared_artifact_key = prebuild_metric_dataframe_shared_key(
        owner_resource.id.as_str(),
        dataframe_metric_id.as_str(),
        &query_options,
        &dependency_revision_key,
    );
    let scope_id = scope_identity_key(scene_id.as_str(), scene_path.as_deref());
    let logical_node_id = logical_dataframe_artifact_id(
        owner_resource.id.as_str(),
        dataframe_metric_id.as_str(),
        page_size,
    );
    let materialization_key = materialization_identity(
        logical_node_id.as_str(),
        scope_id.as_str(),
        dependency_revision_key.as_str(),
        outcome.compile_revision.as_str(),
    );
    Ok(Some(PlannedDataframeArtifact {
        logical_node_id,
        scope_id,
        materialization_key,
        artifact_key,
        shared_artifact_key,
        owner_resource_id: owner_resource.id.clone(),
        resource_selector_id: resource.id.clone(),
        dataframe_metric_id,
        resolved_metric_id,
        page_size,
        scene_id,
        scene_path,
        dependency_revision_key,
        scope_metric_token,
        defs_for_hydrate,
    }))
}

fn widget_dataframe_page_sizes() -> &'static [usize] {
    static PAGE_SIZES: OnceLock<Vec<usize>> = OnceLock::new();
    PAGE_SIZES.get_or_init(|| {
        let mut sizes = std::env::var("MEI_PREBUILD_DATAFRAME_PAGE_SIZES")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .filter_map(|item| item.trim().parse::<usize>().ok())
                    .collect::<Vec<_>>()
            })
            .filter(|sizes| !sizes.is_empty())
            .unwrap_or_else(|| vec![16, 20, 64]);
        sizes.sort_unstable();
        sizes.dedup();
        sizes
    })
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
    format!("{}|{}", source_path, metric_keys.join(","))
}

fn collect_request_artifact_plans(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    dataset_selector: &str,
    metric_ids: &[String],
    metric_worksets: &mut BTreeMap<String, PlannedMetricWorkset>,
    dataframe_tasks: &mut BTreeMap<String, PlannedDataframeArtifact>,
) -> Result<()> {
    let resource = mei_lang_kernel::locate_dataset_resource(&outcome.compiled, dataset_selector)
        .with_context(|| format!("locate warmup dataset `{dataset_selector}`"))?;
    let dataset = resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", resource.id))?;
    if metric_ids.is_empty() {
        let response_ids = response_metric_ids(&outcome.compiled, dataset);
        if !response_ids.is_empty() {
            let metric_groups =
                group_metric_ids_by_owner(&outcome.compiled, resource.id.as_str(), &response_ids)?;
            for metric_ids in metric_groups.into_values() {
                let plan = plan_metric_workset(
                    app_id,
                    app_root,
                    outcome,
                    resource.id.as_str(),
                    metric_ids.as_slice(),
                )?;
                metric_worksets
                    .entry(plan.materialization_key.clone())
                    .or_insert(plan);
            }
        } else {
            let plan = plan_metric_workset(app_id, app_root, outcome, resource.id.as_str(), &[])?;
            metric_worksets
                .entry(plan.materialization_key.clone())
                .or_insert(plan);
        }
    } else {
        let metric_groups =
            group_metric_ids_by_owner(&outcome.compiled, resource.id.as_str(), metric_ids)?;
        for metric_ids in metric_groups.into_values() {
            let plan = plan_metric_workset(
                app_id,
                app_root,
                outcome,
                resource.id.as_str(),
                metric_ids.as_slice(),
            )?;
            metric_worksets
                .entry(plan.materialization_key.clone())
                .or_insert(plan);
        }
    }

    if is_world_metrics_resource(resource.id.as_str()) {
        let mut requested = requested_dataframe_metric_ids(dataset, metric_ids);
        requested.sort();
        requested.dedup();
        for metric_id in requested {
            for page_size in widget_dataframe_page_sizes() {
                if let Some(plan) = plan_dataframe_artifact(
                    app_root,
                    outcome,
                    &resource,
                    metric_id.as_str(),
                    *page_size,
                )? {
                    dataframe_tasks
                        .entry(plan.materialization_key.clone())
                        .or_insert(plan);
                }
            }
        }
        return Ok(());
    }

    for metric_id in dataframe_metric_ids(dataset) {
        for page_size in widget_dataframe_page_sizes() {
            if let Some(plan) = plan_dataframe_artifact(
                app_root,
                outcome,
                &resource,
                metric_id.as_str(),
                *page_size,
            )? {
                dataframe_tasks
                    .entry(plan.materialization_key.clone())
                    .or_insert(plan);
            }
        }
    }
    Ok(())
}

fn build_scope_artifact_plan(
    app_id: &str,
    app_root: &Path,
    scope: &CompileScope,
    outcome: &SharedCompileOutcome,
    requests: &[&AggregatedWarmupRequest],
) -> Result<ScopeArtifactPlan> {
    let mut metric_worksets = BTreeMap::<String, PlannedMetricWorkset>::new();
    let mut dataframe_tasks = BTreeMap::<String, PlannedDataframeArtifact>::new();
    for request in requests {
        collect_request_artifact_plans(
            app_id,
            app_root,
            outcome,
            request.dataset_id.as_str(),
            request.metric_ids.as_slice(),
            &mut metric_worksets,
            &mut dataframe_tasks,
        )?;
    }
    if scope.key() == CompileScope::default_scope().key()
        && compiled_has_world_metrics_runtime_defs(&outcome.compiled)
    {
        collect_request_artifact_plans(
            app_id,
            app_root,
            outcome,
            "__world_metrics__",
            &[],
            &mut metric_worksets,
            &mut dataframe_tasks,
        )?;
    }
    Ok(ScopeArtifactPlan {
        metric_worksets: metric_worksets.into_values().collect(),
        dataframe_artifacts: dataframe_tasks.into_values().collect(),
    })
}

fn build_plan_node_stats(
    manifest_plan: &PrebuildManifestPlan,
    canonical_identity_count: usize,
    scope_plans: &[ScopeArtifactPlan],
) -> PrebuildPlanNodeStatsReport {
    let mut metric_workset_nodes = BTreeSet::new();
    let mut response_nodes = BTreeSet::new();
    let mut dataframe_nodes = BTreeSet::new();
    let mut warmup_scope_nodes = BTreeSet::new();
    let mut logical_workset_nodes = BTreeSet::new();
    let mut logical_dataframe_nodes = BTreeSet::new();
    let mut scope_ids = BTreeSet::new();
    let mut dependency_keys = BTreeSet::new();
    for request in &manifest_plan.warmup_requests {
        warmup_scope_nodes.insert(request.scope.key());
    }
    for plan in scope_plans {
        for workset in &plan.metric_worksets {
            logical_workset_nodes.insert(workset.logical_node_id.clone());
            scope_ids.insert(workset.scope_id.clone());
            dependency_keys.insert(workset.dependency_revision_key.clone());
            metric_workset_nodes.insert(workset.materialization_key.clone());
            response_nodes.insert(workset.response_cache_key.clone());
        }
        for dataframe in &plan.dataframe_artifacts {
            logical_dataframe_nodes.insert(dataframe.logical_node_id.clone());
            scope_ids.insert(dataframe.scope_id.clone());
            dependency_keys.insert(dataframe.dependency_revision_key.clone());
            let _ = dataframe.scope_metric_token.as_str();
            dataframe_nodes.insert(dataframe.artifact_key.clone());
        }
    }
    let _ = (logical_workset_nodes.len(), logical_dataframe_nodes.len(), scope_ids.len(), dependency_keys.len());
    let canonical_prebuild_nodes = canonical_identity_count + metric_workset_nodes.len();
    let budget = PrebuildNodeBudgetReport {
        canonical_node_limit: CANONICAL_PREBUILD_NODE_BUDGET,
        startup_wall_ms_limit: STARTUP_PREBUILD_WALL_MS_BUDGET_MS,
        over_canonical_node_limit: canonical_prebuild_nodes > CANONICAL_PREBUILD_NODE_BUDGET,
    };
    let planned_total_nodes = canonical_prebuild_nodes
        + manifest_plan.warmup_requests.len()
        + dataframe_nodes.len();
    PrebuildPlanNodeStatsReport {
        manifest_compile_scope_nodes: 1
            + manifest_plan.hot_scopes.len()
            + manifest_plan.deferred_scopes.len(),
        hot_compile_scope_nodes: manifest_plan.hot_scopes.len(),
        deferred_compile_scope_nodes: manifest_plan.deferred_scopes.len(),
        planned_warmup_request_nodes: manifest_plan.warmup_requests.len(),
        planned_warmup_scope_nodes: warmup_scope_nodes.len(),
        planned_metric_workset_nodes: metric_workset_nodes.len(),
        planned_response_artifact_nodes: response_nodes.len(),
        planned_dataframe_artifact_nodes: dataframe_nodes.len(),
        planned_total_nodes,
        canonical_prebuild_nodes,
        budget,
    }
}

fn current_bundle_revision_for_plan(plan: &PlannedMetricWorkset) -> Option<String> {
    let defs = plan.defs_for_hydrate.as_ref();
    if defs.is_empty() {
        return None;
    }
    let serialized = serde_json::to_string(defs).ok()?;
    Some(format!(
        "mdb:{}",
        crate::graph::types::stable_hash(&serialized)
    ))
}

fn current_dataframe_bundle_revision(plan: &PlannedDataframeArtifact) -> Option<String> {
    let defs = plan.defs_for_hydrate.as_ref();
    if defs.is_empty() {
        return None;
    }
    let serialized = serde_json::to_string(defs).ok()?;
    Some(format!(
        "mdb:{}",
        crate::graph::types::stable_hash(&serialized)
    ))
}

fn promote_prebuild_metric_response_slot(
    source_root: Option<&Path>,
    app_id: Option<&str>,
    plan: &PlannedMetricWorkset,
    bundle_revision: &str,
) {
    let (Some(source_root), Some(app_id)) = (source_root, app_id) else {
        return;
    };
    let scope_key =
        crate::graph::mrg_eval_scope_key(plan.scene_id.as_str(), plan.scene_path.as_deref());
    let workset_id = format!(
        "workset|app={app_id}|owner={}|metrics={}",
        plan.owner_resource_id,
        if plan.request_all_metrics {
            "*".to_string()
        } else {
            plan.requested_metric_ids.join(",")
        }
    );
    if let Err(error) = crate::graph::mrg::slots::record_mrg_slot_after_eval(
        source_root,
        app_id,
        workset_id.as_str(),
        scope_key.as_str(),
        plan.owner_resource_id.as_str(),
        bundle_revision,
        plan.dependency_revision_key.as_str(),
        plan.response_cache_key.as_str(),
        "eval-results/results/metric-response/",
        0,
        true,
    ) {
        tracing::warn!(
            app_id = %app_id,
            owner = %plan.owner_resource_id,
            error = %error,
            "failed to promote MRG slot from existing metric response artifact"
        );
    }
}

fn ensure_metric_response_artifact_for_plan(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    plan: &PlannedMetricWorkset,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    if let Some(current_rev) = current_bundle_revision_for_plan(plan) {
        if crate::graph::metric_bundle_revision_unchanged(
            &state.pre_mcg_bundle_revisions,
            plan.owner_resource_id.as_str(),
            current_rev.as_str(),
        ) && prebuild_metric_response_index_covers_key(
            app_root,
            &plan.shared_cache_key,
            &plan.covered_metric_ids,
            plan.request_all_metrics,
        )? {
            promote_prebuild_metric_response_slot(
                state.source_root.as_deref(),
                state.app_id.as_deref(),
                plan,
                current_rev.as_str(),
            );
            coverage.metric_response_artifacts_skipped_bundle_unchanged += 1;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
        if let (Some(source_root), Some(stored_app_id)) = (
            state.source_root.as_deref(),
            state.app_id.as_deref(),
        ) {
            let registry = crate::graph::load_mrg_registry(source_root, stored_app_id);
            let scope_key = crate::graph::mrg_eval_scope_key(
                plan.scene_id.as_str(),
                plan.scene_path.as_deref(),
            );
            let mrg_covers = crate::graph::mrg_slot_covers_eval(
                &registry,
                plan.owner_resource_id.as_str(),
                current_rev.as_str(),
                plan.dependency_revision_key.as_str(),
                scope_key.as_str(),
                plan.shared_cache_key.as_str(),
            ) || crate::graph::mrg_slot_covers_eval(
                &registry,
                plan.owner_resource_id.as_str(),
                current_rev.as_str(),
                plan.dependency_revision_key.as_str(),
                scope_key.as_str(),
                plan.response_cache_key.as_str(),
            );
            if mrg_covers
                && prebuild_metric_response_index_covers_key(
                    app_root,
                    &plan.shared_cache_key,
                    &plan.covered_metric_ids,
                    plan.request_all_metrics,
                )?
            {
                promote_prebuild_metric_response_slot(
                    state.source_root.as_deref(),
                    state.app_id.as_deref(),
                    plan,
                    current_rev.as_str(),
                );
                state
                    .diagnostics
                    .mrg_eval_skips
                    .fetch_add(1, Ordering::Relaxed);
                coverage.metric_response_artifacts_skipped_bundle_unchanged += 1;
                coverage.metric_response_artifacts_ready += 1;
                return Ok(());
            }
        }
    }
    let owner_resource = mei_lang_kernel::locate_dataset_resource(
        &outcome.compiled,
        plan.owner_resource_id.as_str(),
    )
    .with_context(|| format!("locate warmup dataset `{}`", plan.dataset_selector))?;
    let owner_dataset = owner_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", owner_resource.id))?;
    let query_state = empty_query_state();
    let query = collect_all_query_options(&query_state);
    if let Some(artifact) = state.metric_response_exact(&plan.response_cache_key) {
        let artifact_covers_request = metric_response_artifact_covers_request(
            &artifact,
            &plan.covered_metric_ids,
            plan.request_all_metrics,
        );
        if artifact_covers_request {
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                &owner_resource,
                &artifact,
                &query,
                plan.defs_for_hydrate.as_ref(),
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
    }
    if let Some(artifact) = state.metric_response_shared(&plan.shared_cache_key) {
        let artifact_covers_request = metric_response_artifact_covers_request(
            &artifact,
            &plan.covered_metric_ids,
            plan.request_all_metrics,
        );
        if artifact_covers_request {
            materialize_metric_response_alias(app_root, &plan.response_cache_key, &artifact)?;
            state.store_metric_response_exact(&plan.response_cache_key, &artifact);
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                &owner_resource,
                &artifact,
                &query,
                plan.defs_for_hydrate.as_ref(),
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
    }
    if prebuild_metric_response_index_covers_key(
        app_root,
        &plan.shared_cache_key,
        &plan.covered_metric_ids,
        plan.request_all_metrics,
    )? {
        coverage.metric_response_artifacts_ready += 1;
        return Ok(());
    }
    if let Some((artifact, _)) =
        load_metric_response_result_artifact(app_root, &plan.response_cache_key)?
    {
        let artifact_covers_request = metric_response_artifact_covers_request(
            &artifact,
            &plan.covered_metric_ids,
            plan.request_all_metrics,
        );
        if artifact_covers_request {
            state.store_metric_response_exact(&plan.response_cache_key, &artifact);
            state.store_metric_response_shared(&plan.shared_cache_key, &artifact);
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                &owner_resource,
                &artifact,
                &query,
                plan.defs_for_hydrate.as_ref(),
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
        if mode == PrebuildMode::Verify {
            anyhow::bail!(
                "metric response artifact for dataset `{}` scope scene=`{}` target=`{}` does not cover all declared metrics",
                plan.dataset_selector,
                plan.scene_id,
                plan.scene_path.as_deref().unwrap_or("")
            );
        }
    } else if mode == PrebuildMode::Verify {
        anyhow::bail!(
            "missing metric response artifact for dataset `{}` scope scene=`{}` target=`{}`",
            plan.dataset_selector,
            plan.scene_id,
            plan.scene_path.as_deref().unwrap_or("")
        );
    }
    if let Some((artifact, _)) = load_metric_response_result_artifact(app_root, &plan.shared_cache_key)?
    {
        let artifact_covers_request = metric_response_artifact_covers_request(
            &artifact,
            &plan.covered_metric_ids,
            plan.request_all_metrics,
        );
        if artifact_covers_request {
            materialize_metric_response_alias(app_root, &plan.response_cache_key, &artifact)?;
            state.store_metric_response_shared(&plan.shared_cache_key, &artifact);
            state.store_metric_response_exact(&plan.response_cache_key, &artifact);
            materialize_metric_response_sibling_aliases(
                app_id,
                app_root,
                outcome,
                &owner_resource,
                &artifact,
                &query,
                plan.defs_for_hydrate.as_ref(),
                state,
            )?;
            coverage.metric_response_artifacts_ready += 1;
            return Ok(());
        }
    }
    let reservation = state
        .metric_response_jobs
        .wait_or_reserve(&plan.shared_cache_key);
    if let ArtifactReservation::Completed = reservation {
        if let Some(artifact) = state.metric_response_shared(&plan.shared_cache_key) {
            let artifact_covers_request = metric_response_artifact_covers_request(
                &artifact,
                &plan.covered_metric_ids,
                plan.request_all_metrics,
            );
            if artifact_covers_request {
                materialize_metric_response_alias(app_root, &plan.response_cache_key, &artifact)?;
                state.store_metric_response_exact(&plan.response_cache_key, &artifact);
                materialize_metric_response_sibling_aliases(
                    app_id,
                    app_root,
                    outcome,
                    &owner_resource,
                    &artifact,
                    &query,
                    plan.defs_for_hydrate.as_ref(),
                    state,
                )?;
                coverage.metric_response_artifacts_ready += 1;
                return Ok(());
            }
        }
    }
    prebuild_emit_progress(format!(
        "[{app_id}] 指标求值开始 | response | dataset={} | scene={}",
        short_dataset_id(plan.dataset_selector.as_str()),
        plan.scene_id
    ));
    let metric_started = Instant::now();
    let primary_resource = mei_lang_kernel::locate_dataset_resource(
        &outcome.compiled,
        plan.dataset_selector.as_str(),
    )
    .with_context(|| format!("locate warmup dataset `{}`", plan.dataset_selector))?;
    let primary_dataset = primary_resource
        .dataset
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("resource `{}` is not a dataset", primary_resource.id))?;
    let access_plan = AccessMetricEvalPlan {
        primary: primary_resource,
        primary_dataset,
        owner: owner_resource,
        owner_dataset,
        request_metric_ids: plan.requested_metric_ids.clone(),
    };
    let eval_outcome = evaluate_runtime_metrics_from_plan(
        &outcome.compiled,
        app_root,
        &access_plan,
        plan.scene_id.as_str(),
        plan.scene_path.as_deref(),
        &query_state,
        &[],
        RuntimeMetricEvalMode::WithDag,
        plan.request_all_metrics,
    )
    .with_context(|| format!("build metric response artifact for dataset `{}`", plan.dataset_selector));
    let eval_outcome = match eval_outcome {
        Ok(eval_outcome) => eval_outcome,
        Err(error) => {
            state.metric_response_jobs.finish(&plan.shared_cache_key, false);
            if let Some(source_root) = state.source_root.as_deref() {
                let bundle_revision = current_bundle_revision_for_plan(plan).unwrap_or_default();
                let scope_key = crate::graph::mrg_eval_scope_key(
                    plan.scene_id.as_str(),
                    plan.scene_path.as_deref(),
                );
                crate::graph::record_prebuild_slot_failed(
                    source_root,
                    app_id,
                    plan.logical_node_id.as_str(),
                    scope_key.as_str(),
                    plan.owner_resource_id.as_str(),
                    bundle_revision.as_str(),
                    plan.dependency_revision_key.as_str(),
                    error.to_string().as_str(),
                );
            }
            return Err(error);
        }
    };
    prebuild_emit_progress(format!(
        "[{app_id}] 指标求值 {:.1}s | response | dataset={} | scene={} | rows={}",
        metric_started.elapsed().as_secs_f64(),
        short_dataset_id(plan.dataset_selector.as_str()),
        plan.scene_id,
        eval_outcome.total_rows
    ));
    state.diagnostics.record_metric_build(
        "response",
        plan.dataset_selector.as_str(),
        "(bundle)",
        plan.scene_id.as_str(),
        metric_started.elapsed().as_millis() as u64,
    );
    let declared_metric_ids = owner_dataset
        .runtime_metric_defs
        .keys()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let complete = plan.request_all_metrics
        && !declared_metric_ids.is_empty()
        && declared_metric_ids
            .iter()
            .all(|metric_id| plan.covered_metric_ids.contains(metric_id));
    let built_artifact = LoadedMetricResponseArtifact {
        total_rows: eval_outcome.total_rows,
        metrics_map: eval_outcome.metrics_map.clone(),
        covered_metric_ids: plan.covered_metric_ids.clone(),
        complete,
    };
    let store_result = (|| -> Result<()> {
        store_cached_metric_response(
            plan.shared_cache_key.clone(),
            eval_outcome.total_rows,
            &eval_outcome.metrics_map,
            &plan.covered_metric_ids,
            complete,
        );
        store_metric_response_result_artifact(
            app_root,
            &plan.shared_cache_key,
            eval_outcome.total_rows,
            &eval_outcome.metrics_map,
            &plan.covered_metric_ids,
            complete,
        )?;
        materialize_metric_response_alias_parts(
            app_root,
            &plan.response_cache_key,
            eval_outcome.total_rows,
            &eval_outcome.metrics_map,
            &plan.covered_metric_ids,
            complete,
        )?;
        Ok(())
    })();
    state
        .metric_response_jobs
        .finish(&plan.shared_cache_key, store_result.is_ok());
    if store_result.is_ok() {
        state.store_metric_response_shared(&plan.shared_cache_key, &built_artifact);
        state.store_metric_response_exact(&plan.response_cache_key, &built_artifact);
    }
    store_result?;
    materialize_metric_response_sibling_aliases(
        app_id,
        app_root,
        outcome,
        &owner_resource,
        &built_artifact,
        &query,
        plan.defs_for_hydrate.as_ref(),
        state,
    )?;
    coverage.metric_response_artifacts_built += 1;
    if let Some(source_root) = state.source_root.as_deref() {
        let bundle_revision = current_bundle_revision_for_plan(plan).unwrap_or_default();
        let scope_key = crate::graph::mrg_eval_scope_key(
            plan.scene_id.as_str(),
            plan.scene_path.as_deref(),
        );
        crate::graph::record_prebuild_slot(
            source_root,
            app_id,
            plan.logical_node_id.as_str(),
            scope_key.as_str(),
            plan.owner_resource_id.as_str(),
            bundle_revision.as_str(),
            plan.dependency_revision_key.as_str(),
            plan.shared_cache_key.as_str(),
            "eval-results/results/metric-response/",
            metric_started.elapsed().as_millis() as u64,
        );
    }
    Ok(())
}

fn ensure_metric_response_artifact_for_request(
    app_id: &str,
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    dataset_selector: &str,
    metric_ids: &[String],
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let plan = plan_metric_workset(app_id, app_root, outcome, dataset_selector, metric_ids)?;
    ensure_metric_response_artifact_for_plan(app_id, app_root, outcome, &plan, mode, coverage, state)
}

fn dataframe_scope_metric_token(
    compiled: &mei_lang_kernel::CompiledApp,
    resource_id: &str,
    metric_selector: &str,
) -> Option<String> {
    let (_, resolved_metric_id) =
        locate_runtime_metric_resource(compiled, resource_id, metric_selector).ok()?;
    Some(metric_scope_cache_key(std::slice::from_ref(
        &resolved_metric_id,
    )))
}

fn prebuild_dataframe_metric_selector(
    metric_defs: &BTreeMap<String, Value>,
    resolved_metric_id: &str,
) -> String {
    let resolved_metric_id = resolved_metric_id.trim();
    if resolved_metric_id.is_empty() || resolved_metric_id.ends_with("::__scalar_rowset__") {
        return resolved_metric_id.to_string();
    }
    let scalar_rowset_id = format!("{resolved_metric_id}::__scalar_rowset__");
    if metric_defs.contains_key(&scalar_rowset_id) {
        return scalar_rowset_id;
    }
    let shape = metric_defs
        .get(resolved_metric_id)
        .and_then(Value::as_object)
        .and_then(|map| map.get("shape"))
        .and_then(Value::as_str);
    if matches!(shape, Some("scalar") | Some("scalar_map")) {
        return scalar_rowset_id;
    }
    resolved_metric_id.to_string()
}

fn ensure_metric_dataframe_artifact_for_plan(
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    plan: &PlannedDataframeArtifact,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    if let Some(current_rev) = current_dataframe_bundle_revision(plan) {
        if crate::graph::metric_bundle_revision_unchanged(
            &state.pre_mcg_bundle_revisions,
            plan.owner_resource_id.as_str(),
            current_rev.as_str(),
        ) && (metric_dataframe_result_artifact_exists(app_root, &plan.shared_artifact_key)
            || metric_dataframe_result_artifact_exists(app_root, &plan.artifact_key))
        {
            coverage.metric_dataframe_artifacts_skipped_bundle_unchanged += 1;
            coverage.metric_dataframe_artifacts_ready += 1;
            return Ok(());
        }
        if let (Some(source_root), Some(stored_app_id)) = (
            state.source_root.as_deref(),
            state.app_id.as_deref(),
        ) {
            let registry = crate::graph::load_mrg_registry(source_root, stored_app_id);
            let scope_key = crate::graph::mrg_eval_scope_key(
                plan.scene_id.as_str(),
                plan.scene_path.as_deref(),
            );
            if crate::graph::mrg_slot_covers_dataframe_eval(
                &registry,
                plan.owner_resource_id.as_str(),
                current_rev.as_str(),
                plan.dependency_revision_key.as_str(),
                scope_key.as_str(),
                plan.shared_artifact_key.as_str(),
            ) && (metric_dataframe_result_artifact_exists(app_root, &plan.shared_artifact_key)
                || metric_dataframe_result_artifact_exists(app_root, &plan.artifact_key))
            {
                state
                    .diagnostics
                    .dataframe_eval_skips
                    .fetch_add(1, Ordering::Relaxed);
                coverage.metric_dataframe_artifacts_skipped_bundle_unchanged += 1;
                coverage.metric_dataframe_artifacts_ready += 1;
                return Ok(());
            }
        }
    }
    let owner_resource = mei_lang_kernel::locate_dataset_resource(
        &outcome.compiled,
        plan.owner_resource_id.as_str(),
    )
    .with_context(|| {
        format!(
            "locate warmup dataset `{}` for dataframe metric `{}`",
            plan.resource_selector_id, plan.dataframe_metric_id
        )
    })?;
    let query_options = widget_dataframe_query_options(plan.page_size);
    if let Some(result) = state.metric_dataframe_shared(&plan.shared_artifact_key) {
        store_metric_dataframe_result_artifact(app_root, &plan.artifact_key, &result)?;
        state.store_metric_dataframe_exact(&plan.artifact_key, &result);
        materialize_metric_dataframe_sibling_aliases(
            app_root,
            outcome,
            &owner_resource,
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        materialize_metric_dataframe_metric_aliases(
            app_root,
            outcome,
            plan.resource_selector_id.as_str(),
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if state.metric_dataframe_exact(&plan.artifact_key).is_some() {
        if let Some(result) = state.metric_dataframe_exact(&plan.artifact_key) {
            materialize_metric_dataframe_sibling_aliases(
                app_root,
                outcome,
                &owner_resource,
                plan.resolved_metric_id.as_str(),
                &query_options,
                plan.defs_for_hydrate.as_ref(),
                &result,
                state,
            )?;
            materialize_metric_dataframe_metric_aliases(
                app_root,
                outcome,
                plan.resource_selector_id.as_str(),
                plan.resolved_metric_id.as_str(),
                &query_options,
                plan.defs_for_hydrate.as_ref(),
                &result,
                state,
            )?;
        }
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if let Some((result, _)) =
        load_metric_dataframe_result_artifact(app_root, &plan.shared_artifact_key)?
    {
        store_metric_dataframe_result_artifact(app_root, &plan.artifact_key, &result)?;
        state.store_metric_dataframe_shared(&plan.shared_artifact_key, &result);
        state.store_metric_dataframe_exact(&plan.artifact_key, &result);
        materialize_metric_dataframe_sibling_aliases(
            app_root,
            outcome,
            &owner_resource,
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        materialize_metric_dataframe_metric_aliases(
            app_root,
            outcome,
            plan.resource_selector_id.as_str(),
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if metric_dataframe_result_artifact_exists(app_root, &plan.shared_artifact_key) {
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if metric_dataframe_result_artifact_exists(app_root, &plan.artifact_key) {
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if let Some((result, _)) = load_metric_dataframe_result_artifact(app_root, &plan.artifact_key)? {
        state.store_metric_dataframe_exact(&plan.artifact_key, &result);
        state.store_metric_dataframe_shared(&plan.shared_artifact_key, &result);
        materialize_metric_dataframe_sibling_aliases(
            app_root,
            outcome,
            &owner_resource,
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        materialize_metric_dataframe_metric_aliases(
            app_root,
            outcome,
            plan.resource_selector_id.as_str(),
            plan.resolved_metric_id.as_str(),
            &query_options,
            plan.defs_for_hydrate.as_ref(),
            &result,
            state,
        )?;
        coverage.metric_dataframe_artifacts_ready += 1;
        return Ok(());
    }
    if mode == PrebuildMode::Verify {
        anyhow::bail!(
            "missing metric dataframe artifact for dataset `{}` metric `{}` scope scene=`{}` target=`{}`",
            plan.resource_selector_id,
            plan.dataframe_metric_id,
            plan.scene_id,
            plan.scene_path.as_deref().unwrap_or("")
        );
    }
    let reservation = state
        .metric_dataframe_jobs
        .wait_or_reserve(&plan.shared_artifact_key);
    if let ArtifactReservation::Completed = reservation {
        if let Some(result) = state.metric_dataframe_shared(&plan.shared_artifact_key) {
            store_metric_dataframe_result_artifact(app_root, &plan.artifact_key, &result)?;
            state.store_metric_dataframe_exact(&plan.artifact_key, &result);
            materialize_metric_dataframe_sibling_aliases(
                app_root,
                outcome,
                &owner_resource,
                plan.resolved_metric_id.as_str(),
                &query_options,
                plan.defs_for_hydrate.as_ref(),
                &result,
                state,
            )?;
            materialize_metric_dataframe_metric_aliases(
                app_root,
                outcome,
                plan.resource_selector_id.as_str(),
                plan.resolved_metric_id.as_str(),
                &query_options,
                plan.defs_for_hydrate.as_ref(),
                &result,
                state,
            )?;
            coverage.metric_dataframe_artifacts_ready += 1;
            return Ok(());
        }
    }
    prebuild_emit_progress(format!(
        "[{}] 指标求值开始 | dataframe | {} | metric={} | scene={}",
        app_root.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        short_dataset_id(plan.resource_selector_id.as_str()),
        short_metric_id(plan.dataframe_metric_id.as_str()),
        plan.scene_id
    ));
    let metric_started = Instant::now();
    let result = query_metric_dataframe(
        &outcome.compiled,
        app_root,
        owner_resource.id.as_str(),
        plan.dataframe_metric_id.as_str(),
        Some(plan.scene_id.as_str()),
        plan.scene_path.as_deref(),
        &outcome.compile_revision,
        query_options.clone(),
        None,
        Vec::new(),
    )
    .with_context(|| {
        format!(
            "build metric dataframe artifact for dataset `{}` metric `{}`",
            plan.resource_selector_id, plan.dataframe_metric_id
        )
    });
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            state.metric_dataframe_jobs.finish(&plan.shared_artifact_key, false);
            if let (Some(source_root), Some(app_id)) =
                (state.source_root.as_deref(), state.app_id.as_deref())
            {
                let bundle_revision =
                    current_dataframe_bundle_revision(plan).unwrap_or_default();
                let scope_key = crate::graph::mrg_eval_scope_key(
                    plan.scene_id.as_str(),
                    plan.scene_path.as_deref(),
                );
                crate::graph::record_prebuild_slot_failed(
                    source_root,
                    app_id,
                    plan.logical_node_id.as_str(),
                    scope_key.as_str(),
                    plan.owner_resource_id.as_str(),
                    bundle_revision.as_str(),
                    plan.dependency_revision_key.as_str(),
                    error.to_string().as_str(),
                );
            }
            return Err(error);
        }
    };
    prebuild_emit_progress(format!(
        "[{}] 指标求值 {:.1}s | dataframe | {} | metric={} | scene={} | rows={}",
        app_root.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        metric_started.elapsed().as_secs_f64(),
        short_dataset_id(plan.resource_selector_id.as_str()),
        short_metric_id(plan.dataframe_metric_id.as_str()),
        plan.scene_id,
        result.total
    ));
    state.diagnostics.record_metric_build(
        "dataframe",
        plan.resource_selector_id.as_str(),
        plan.dataframe_metric_id.as_str(),
        plan.scene_id.as_str(),
        metric_started.elapsed().as_millis() as u64,
    );
    let store_result = (|| -> Result<()> {
        store_metric_dataframe_result_artifact(app_root, &plan.shared_artifact_key, &result)?;
        if plan.shared_artifact_key != plan.artifact_key {
            store_metric_dataframe_result_artifact(app_root, &plan.artifact_key, &result)?;
        }
        Ok(())
    })();
    state
        .metric_dataframe_jobs
        .finish(&plan.shared_artifact_key, store_result.is_ok());
    if store_result.is_ok() {
        state.store_metric_dataframe_shared(&plan.shared_artifact_key, &result);
        state.store_metric_dataframe_exact(&plan.artifact_key, &result);
    }
    store_result?;
    materialize_metric_dataframe_sibling_aliases(
        app_root,
        outcome,
        &owner_resource,
        plan.resolved_metric_id.as_str(),
        &query_options,
        plan.defs_for_hydrate.as_ref(),
        &result,
        state,
    )?;
    materialize_metric_dataframe_metric_aliases(
        app_root,
        outcome,
        plan.resource_selector_id.as_str(),
        plan.resolved_metric_id.as_str(),
        &query_options,
        plan.defs_for_hydrate.as_ref(),
        &result,
        state,
    )?;
    coverage.metric_dataframe_artifacts_built += 1;
    if let (Some(source_root), Some(stored_app_id)) = (
        state.source_root.as_deref(),
        state.app_id.as_deref(),
    ) {
        let bundle_revision = current_dataframe_bundle_revision(plan).unwrap_or_default();
        let scope_key = crate::graph::mrg_eval_scope_key(
            plan.scene_id.as_str(),
            plan.scene_path.as_deref(),
        );
        crate::graph::record_prebuild_dataframe_slot(
            source_root,
            stored_app_id,
            plan.logical_node_id.as_str(),
            scope_key.as_str(),
            plan.owner_resource_id.as_str(),
            bundle_revision.as_str(),
            plan.dependency_revision_key.as_str(),
            plan.shared_artifact_key.as_str(),
            "eval-results/results/metric-dataframe/",
            metric_started.elapsed().as_millis() as u64,
        );
    }
    Ok(())
}

fn ensure_metric_dataframe_artifact(
    app_root: &Path,
    outcome: &SharedCompileOutcome,
    resource: &LoadedResource,
    metric_id: &str,
    page_size: usize,
    mode: PrebuildMode,
    coverage: &mut PrebuildCoverageReport,
    state: &CoverageState,
) -> Result<()> {
    let Some(plan) = plan_dataframe_artifact(app_root, outcome, resource, metric_id, page_size)? else {
        return Ok(());
    };
    ensure_metric_dataframe_artifact_for_plan(app_root, outcome, &plan, mode, coverage, state)
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
    outcome: &SharedCompileOutcome,
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
    let (scene_id, scene_path) =
        artifact_scene_context_for_resource(&outcome.compiled, owner_resource.id.as_str());
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
            None,
        );
        if state.metric_response_exact(&response_cache_key).is_some() {
            continue;
        }
        if metric_response_result_artifact_exists(app_root, &response_cache_key) {
            continue;
        }
        materialize_metric_response_alias(app_root, &response_cache_key, artifact)?;
    }
    Ok(())
}

fn materialize_metric_dataframe_metric_aliases(
    app_root: &Path,
    outcome: &SharedCompileOutcome,
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
        let Ok((owner_resource, canonical_metric_id)) = locate_runtime_metric_resource(
            &outcome.compiled,
            resource_id,
            metric_selector.as_str(),
        ) else {
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
        .unwrap_or_else(|| metric_scope_cache_key(std::slice::from_ref(&canonical_metric_id)));
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
        if metric_dataframe_result_artifact_exists(app_root, &response_cache_key) {
            continue;
        }
        store_metric_dataframe_result_artifact(app_root, &response_cache_key, result)?;
    }
    let _ = metric_defs;
    Ok(())
}

fn materialize_metric_dataframe_sibling_aliases(
    app_root: &Path,
    outcome: &SharedCompileOutcome,
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
        .min(prebuild_max_parallelism_cap())
        .min(job_count)
        .max(1)
}

fn run_limited_parallel_ordered<T, R, F>(items: Vec<T>, max_parallelism: usize, job: F) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> R + Sync,
{
    run_limited_parallel_ordered_with_hook(items, max_parallelism, job, |_, _| {})
}

fn run_limited_parallel_ordered_with_hook<T, R, F, H>(
    items: Vec<T>,
    max_parallelism: usize,
    job: F,
    hook: H,
) -> Vec<R>
where
    T: Send,
    R: Send,
    F: Fn(T) -> R + Sync,
    H: Fn(usize, &R) + Sync,
{
    if items.len() <= 1 || max_parallelism <= 1 {
        return items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let result = job(item);
                hook(index, &result);
                result
            })
            .collect();
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
        let hook_ref = &hook;
        let mut handles = Vec::new();
        for bucket in buckets.into_iter().filter(|bucket| !bucket.is_empty()) {
            handles.push(scope.spawn(move || {
                let mut output = Vec::with_capacity(bucket.len());
                for (index, item) in bucket {
                    let result = job_ref(item);
                    hook_ref(index, &result);
                    output.push((index, result));
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

#[derive(Debug, Clone, Default, Serialize)]
pub struct ScopedMaterializeReport {
    #[serde(rename = "scopeArtifactsMs")]
    pub scope_artifacts_ms: u64,
    #[serde(rename = "mrgSlotsReady")]
    pub mrg_slots_ready: usize,
    #[serde(rename = "evalArtifactsWarmed")]
    pub eval_artifacts_warmed: usize,
}

/// Write-path: warm metric/dataframe artifacts for a scoped compile outcome (Build scoped rebuild).
pub fn materialize_scope_after_compile(
    source_root: &Path,
    app_id: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
    outcome: &toolchain::CompileWithCacheOutcome,
    mode: PrebuildMode,
) -> Result<ScopedMaterializeReport> {
    use crate::graph::types::MaterialState;

    let started = Instant::now();
    let app_root = resolve_app_root(source_root, app_id);
    let shared = SharedCompileOutcome {
        compiled: Arc::new(outcome.compiled.clone()),
        cache_hit: outcome.cache_hit,
        artifact_cache_hit: outcome.artifact_cache_hit,
        compile_revision: outcome.compile_revision.clone(),
        cache_lookup_ms: outcome.cache_lookup_ms,
        artifact_load_ms: outcome.artifact_load_ms,
        compile_ms: outcome.compile_ms,
    };
    let scope = CompileScope {
        requested_scene_id: scene_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        requested_target_file: target_file
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    };
    let mut coverage = PrebuildCoverageReport::default();
    let mut state = CoverageState::default();
    state.source_root = Some(source_root.to_path_buf());
    state.app_id = Some(app_id.to_string());
    state.pre_mcg_bundle_revisions =
        crate::graph::dedup::load_mcg_bundle_revisions(source_root, app_id);

    let mut warmed_via_plan = false;
    if let Ok(Some(manifest)) = resolve_runtime_warmup_manifest(source_root) {
        if let Some(app) = manifest.apps.iter().find(|entry| entry.app_id == app_id) {
            let plan = build_prebuild_manifest_plan(app, PrebuildScopeProfile::Full);
            let matching =
                matching_warmup_requests_for_outcome(&plan.warmup_requests, &shared);
            if !matching.is_empty() {
                let scope_plan = build_scope_artifact_plan(
                    app_id,
                    app_root.as_path(),
                    &scope,
                    &shared,
                    matching.as_slice(),
                )?;
                ensure_scope_artifacts(
                    app_id,
                    app_root.as_path(),
                    &shared,
                    &scope_plan,
                    mode,
                    &mut coverage,
                    &state,
                )?;
                warmed_via_plan = true;
            }
        }
    }

    if !warmed_via_plan {
        for resource in &shared.compiled.resources {
            let Some(dataset) = resource.dataset.as_ref() else {
                continue;
            };
            if !dataset.has_runtime_metric_defs() {
                continue;
            }
            let _ = ensure_request_artifacts_for_compiled(
                app_id,
                app_root.as_path(),
                &shared,
                resource.id.as_str(),
                &[],
                mode,
                &mut coverage,
                &state,
            );
        }
        if compiled_has_world_metrics_runtime_defs(&shared.compiled) {
            let _ = ensure_request_artifacts_for_compiled(
                app_id,
                app_root.as_path(),
                &shared,
                "__world_metrics__",
                &[],
                mode,
                &mut coverage,
                &state,
            );
        }
    }

    let eval_artifacts_warmed = coverage
        .metric_response_artifacts_built
        .saturating_add(coverage.metric_dataframe_artifacts_built)
        .saturating_add(coverage.metric_response_artifacts_skipped_bundle_unchanged)
        .saturating_add(coverage.metric_dataframe_artifacts_skipped_bundle_unchanged);
    let mrg_slots_ready = if crate::graph::feature::graph_registry_dedup_enabled() {
        crate::graph::load_mrg_registry(source_root, app_id)
            .slots
            .iter()
            .filter(|slot| slot.state == MaterialState::Ready)
            .count()
    } else {
        0
    };

    Ok(ScopedMaterializeReport {
        scope_artifacts_ms: started.elapsed().as_millis() as u64,
        mrg_slots_ready,
        eval_artifacts_warmed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{BoardFileEntry, CompiledApp, CompiledSceneRoute, DatasetView, LoadedResource, SourceDecl};
    use serde_json::json;

    fn test_outcome(active_scene: &str, active_target_file: &str) -> SharedCompileOutcome {
        SharedCompileOutcome {
            compiled: Arc::new(CompiledApp {
                app_id: "demo".to_string(),
                title: "demo".to_string(),
                app_root: "/tmp/demo".to_string(),
                active_scene: Some(active_scene.to_string()),
                active_target_file: active_target_file.to_string(),
                scene_routes: vec![CompiledSceneRoute {
                    scene_id: "home".to_string(),
                    frame_id: None,
                    target_file: "scenes/home.mei".to_string(),
                    kind: "page".to_string(),
                    title: None,
                    is_default: true,
                    access_export: true,
                }],
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
            }),
            cache_hit: true,
            artifact_cache_hit: true,
            compile_revision: "rev-a".to_string(),
            cache_lookup_ms: 0,
            artifact_load_ms: 0,
            compile_ms: 0,
        }
    }

    #[test]
    fn default_scope_rejects_non_default_active_target() {
        let outcome = test_outcome("home", "scenes/07-问题办理.board.mei");
        assert!(!compile_outcome_matches_scope(
            &CompileScope::default_scope(),
            &outcome.compiled
        ));
    }

    #[test]
    fn finalize_coverage_report_computes_missing_artifacts() {
        let mut coverage = PrebuildCoverageReport {
            compile_artifacts_planned: 3,
            compile_artifacts_ready: 2,
            dataset_import_artifacts_planned: 2,
            dataset_import_artifacts_ready: 2,
            metric_response_artifacts_planned: 5,
            metric_response_artifacts_ready: 3,
            metric_dataframe_artifacts_planned: 4,
            metric_dataframe_artifacts_ready: 1,
            ..PrebuildCoverageReport::default()
        };

        finalize_coverage_report(&mut coverage);

        assert_eq!(coverage.compile_artifacts_missing, 1);
        assert_eq!(coverage.dataset_import_artifacts_missing, 0);
        assert_eq!(coverage.metric_response_artifacts_missing, 2);
        assert_eq!(coverage.metric_dataframe_artifacts_missing, 3);
        assert_eq!(coverage.total_missing_artifacts, 6);
    }

    fn test_dataset_resource(id: &str) -> LoadedResource {
        LoadedResource {
            id: id.to_string(),
            kind: "dataset".to_string(),
            title: None,
            document: None,
            dataset: Some(DatasetView {
                id: id.to_string(),
                title: None,
                purpose: None,
                schema: Vec::new(),
                stage_schema: Vec::new(),
                columns: vec!["value".to_string()],
                rows: vec![json!({"value": 1})],
                source: SourceDecl {
                    kind: "csv".to_string(),
                    path: "data/demo.csv".to_string(),
                    sheet: None,
                    header_row: None,
                    preview_rows: None,
                    page_size: None,
                    max_page_size: None,
                    table: None,
                    query: None,
                    connection: None,
                    content: None,
                },
                sources: Vec::new(),
                metrics: Default::default(),
                runtime_metric_defs: Default::default(),
                runtime_analysis_graph: Default::default(),
                runtime_analysis_contracts: Default::default(),
            }),
        }
    }

    #[test]
    fn prebuild_report_summary_omits_compile_revision() {
        let report = PrebuildReport {
            schema_version: "mei-prebuild-report-v1".to_string(),
            mode: PrebuildMode::Verify,
            scope_profile: PrebuildScopeProfile::Full,
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
            diagnostics: PrebuildDiagnosticsReport::default(),
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
                diagnostics: PrebuildDiagnosticsReport::default(),
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
                priority: None,
                metric_id: None,
                metric_ids: Vec::new(),
            }],
            xlsx_sources: Vec::new(),
        };
        let scope_keys = compile_scopes_for_app(&app, PrebuildScopeProfile::Full)
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
        assert!(!scope_keys.contains("details|scenes/02-inspection.mei"));
    }

    #[test]
    fn hot_only_compile_scopes_skip_deferred_dataset_closure() {
        let app = RuntimeWarmupApp {
            app_id: "demo".to_string(),
            default_scene: Some("home".to_string()),
            hot_scenes: vec!["dashboard".to_string()],
            scenes: vec!["details".to_string()],
            focuses: vec!["main.mei".to_string()],
            datasets: vec![
                RuntimeWarmupDatasetRequest {
                    scene_id: Some("dashboard".to_string()),
                    focus: Some("main.mei".to_string()),
                    dataset_id: "hot_ds".to_string(),
                    priority: None,
                    metric_id: None,
                    metric_ids: Vec::new(),
                },
                RuntimeWarmupDatasetRequest {
                    scene_id: Some("details".to_string()),
                    focus: Some("scenes/details.mei".to_string()),
                    dataset_id: "deferred_ds".to_string(),
                    priority: None,
                    metric_id: None,
                    metric_ids: Vec::new(),
                },
            ],
            xlsx_sources: Vec::new(),
        };
        let scope_keys = compile_scopes_for_app(&app, PrebuildScopeProfile::HotOnly)
            .into_iter()
            .map(|scope| scope.key())
            .collect::<BTreeSet<_>>();

        assert!(scope_keys.contains("|"));
        assert!(scope_keys.contains("home|"));
        assert!(scope_keys.contains("dashboard|"));
        assert!(scope_keys.contains("|main.mei"));
        assert!(scope_keys.contains("dashboard|main.mei"));
        assert!(!scope_keys.contains("details|"));
        assert!(!scope_keys.contains("details|scenes/details.mei"));
    }

    #[test]
    fn hot_only_warmup_requests_keep_hot_scoped_datasets() {
        let app = RuntimeWarmupApp {
            app_id: "demo".to_string(),
            default_scene: Some("home".to_string()),
            hot_scenes: vec!["dashboard".to_string()],
            scenes: vec!["details".to_string()],
            focuses: vec!["main.mei".to_string()],
            datasets: vec![
                RuntimeWarmupDatasetRequest {
                    scene_id: Some("dashboard".to_string()),
                    focus: Some("main.mei".to_string()),
                    dataset_id: "hot_ds".to_string(),
                    priority: None,
                    metric_id: Some("metric_a".to_string()),
                    metric_ids: Vec::new(),
                },
                RuntimeWarmupDatasetRequest {
                    scene_id: Some("details".to_string()),
                    focus: Some("scenes/details.mei".to_string()),
                    dataset_id: "deferred_ds".to_string(),
                    priority: None,
                    metric_id: Some("metric_b".to_string()),
                    metric_ids: Vec::new(),
                },
            ],
            xlsx_sources: Vec::new(),
        };
        let requests = aggregate_warmup_requests(&app, PrebuildScopeProfile::HotOnly);

        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].dataset_id, "hot_ds");
        assert_eq!(requests[0].scope.key(), "dashboard|main.mei");
    }

    #[test]
    fn warmup_scope_batches_group_multiple_requests_by_scope() {
        let request_a = AggregatedWarmupRequest {
            scope: CompileScope {
                requested_scene_id: Some("home".to_string()),
                requested_target_file: Some("scenes/home.mei".to_string()),
            },
            dataset_id: "ds_a".to_string(),
            priority: WarmupRequestPriority::Critical,
            metric_ids: vec!["metric_a".to_string()],
        };
        let request_b = AggregatedWarmupRequest {
            scope: CompileScope {
                requested_scene_id: Some("home".to_string()),
                requested_target_file: Some("scenes/home.mei".to_string()),
            },
            dataset_id: "ds_b".to_string(),
            priority: WarmupRequestPriority::Critical,
            metric_ids: vec!["metric_b".to_string()],
        };
        let request_c = AggregatedWarmupRequest {
            scope: CompileScope {
                requested_scene_id: Some("details".to_string()),
                requested_target_file: Some("scenes/details.mei".to_string()),
            },
            dataset_id: "ds_c".to_string(),
            priority: WarmupRequestPriority::Deferred,
            metric_ids: vec!["metric_c".to_string()],
        };

        let grouped = group_warmup_requests_by_scope(&[&request_a, &request_b, &request_c])
            .into_iter()
            .map(|batch| (batch.scope.key(), batch.requests.len()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped.get("home|scenes/home.mei"), Some(&2));
        assert_eq!(grouped.get("details|scenes/details.mei"), Some(&1));
    }

    #[test]
    fn discovered_compile_scopes_do_not_cross_bind_target_specific_scope_with_overlays() {
        let scope = CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: Some("scenes/home.mei".to_string()),
        };
        let mut outcome = test_outcome("home", "scenes/home.mei");
        let compiled = Arc::make_mut(&mut outcome.compiled);
        compiled.scene_local_nav_by_target.insert(
            "scenes/popup.board.mei".to_string(),
            json!({"scene_file":"scenes/popup.board.mei"}),
        );
        compiled.scene_routes.push(CompiledSceneRoute {
            scene_id: "popup_scene".to_string(),
            frame_id: None,
            target_file: "scenes/popup.board.mei".to_string(),
            kind: "board".to_string(),
            title: None,
            is_default: false,
            access_export: true,
        });

        let discovered = discovered_compile_scopes(&scope, &outcome.compiled)
            .into_iter()
            .map(|scope| scope.key())
            .collect::<BTreeSet<_>>();
        assert!(discovered.contains("home|"));
        assert!(discovered.contains("home|scenes/home.mei"));
        assert!(!discovered.contains("home|scenes/popup.board.mei"));
        assert!(!discovered.contains("popup_scene|scenes/popup.board.mei"));
    }

    #[test]
    fn discovered_compile_scopes_keep_default_scope_explicit_only() {
        let scope = CompileScope::default_scope();
        let mut outcome = test_outcome("home", "scenes/home.mei");
        let compiled = Arc::make_mut(&mut outcome.compiled);
        compiled.scene_local_nav_by_target.insert(
            "scenes/popup.board.mei".to_string(),
            json!({"scene_file":"scenes/popup.board.mei"}),
        );
        compiled.scene_routes.push(CompiledSceneRoute {
            scene_id: "popup_scene".to_string(),
            frame_id: None,
            target_file: "scenes/popup.board.mei".to_string(),
            kind: "board".to_string(),
            title: None,
            is_default: false,
            access_export: true,
        });

        let discovered = discovered_compile_scopes(&scope, &outcome.compiled)
            .into_iter()
            .map(|scope| scope.key())
            .collect::<BTreeSet<_>>();
        assert!(discovered.contains("home|"));
        assert!(discovered.contains("home|scenes/home.mei"));
        assert_eq!(discovered.len(), 2);
        assert!(!discovered.contains("home|scenes/popup.board.mei"));
        assert!(!discovered.contains("popup_scene|scenes/popup.board.mei"));
    }

    #[test]
    fn discovered_compile_scopes_do_not_expand_target_only_scope_into_route_aliases() {
        let scope = CompileScope {
            requested_scene_id: None,
            requested_target_file: Some("scenes/popup.board.mei".to_string()),
        };
        let mut outcome = test_outcome("popup_scene", "scenes/popup.board.mei");
        let compiled = Arc::make_mut(&mut outcome.compiled);
        compiled.scene_routes.push(CompiledSceneRoute {
            scene_id: "popup_scene".to_string(),
            frame_id: None,
            target_file: "scenes/popup.board.mei".to_string(),
            kind: "board".to_string(),
            title: None,
            is_default: false,
            access_export: true,
        });
        compiled.scene_routes.push(CompiledSceneRoute {
            scene_id: "popup_scene_duplicate".to_string(),
            frame_id: None,
            target_file: "scenes/popup.board.mei".to_string(),
            kind: "board".to_string(),
            title: None,
            is_default: false,
            access_export: true,
        });

        let discovered = discovered_compile_scopes(&scope, &outcome.compiled)
            .into_iter()
            .map(|scope| scope.key())
            .collect::<BTreeSet<_>>();
        assert!(discovered.contains("popup_scene|"));
        assert!(discovered.contains("popup_scene|scenes/popup.board.mei"));
        assert!(!discovered.contains("popup_scene_duplicate|scenes/popup.board.mei"));
    }

    #[test]
    fn discovered_compile_scopes_expand_board_target_without_active_scene() {
        let scope = CompileScope {
            requested_scene_id: None,
            requested_target_file: Some("scenes/01-elements.board.mei".to_string()),
        };
        let mut outcome = test_outcome("", "scenes/01-elements.board.mei");
        {
            let compiled = Arc::make_mut(&mut outcome.compiled);
            compiled.active_scene = None;
            compiled.build_board_index.boards.insert(
                "scenes/01-elements.board.mei#key_enterprises_analytics_board".to_string(),
                BoardFileEntry {
                    board_file: "scenes/01-elements.board.mei".to_string(),
                    scene_id: "key_enterprises_analytics_board".to_string(),
                    label: "Key enterprises".to_string(),
                    ..Default::default()
                },
            );
            compiled.build_board_index.boards.insert(
                "scenes/01-elements.board.mei#enforcement_units_analytics_board".to_string(),
                BoardFileEntry {
                    board_file: "scenes/01-elements.board.mei".to_string(),
                    scene_id: "enforcement_units_analytics_board".to_string(),
                    label: "Enforcement units".to_string(),
                    ..Default::default()
                },
            );
        }

        let discovered = discovered_compile_scopes(&scope, &outcome.compiled)
            .into_iter()
            .map(|scope| scope.key())
            .collect::<BTreeSet<_>>();
        assert!(discovered.contains(
            "key_enterprises_analytics_board|scenes/01-elements.board.mei"
        ));
        assert!(discovered.contains(
            "enforcement_units_analytics_board|scenes/01-elements.board.mei"
        ));
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
                priority: None,
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
    fn requested_dataframe_metric_ids_respects_explicit_metric_list() {
        let mut resource = test_dataset_resource("demo_ds");
        let dataset = resource.dataset.as_mut().expect("dataset");
        dataset
            .runtime_metric_defs
            .insert("table_a".to_string(), json!({"shape":"dataframe"}));
        dataset
            .runtime_metric_defs
            .insert("table_b".to_string(), json!({"shape":"dataframe"}));
        dataset.runtime_analysis_contracts.insert(
            "demo".to_string(),
            json!({
                "table_metric_id": "table_a",
                "detail_table_metric_id": "table_b"
            }),
        );

        let requested =
            requested_dataframe_metric_ids(dataset, &[String::from("table_a::__scalar_rowset__")]);
        assert_eq!(requested, vec!["table_a::__scalar_rowset__".to_string()]);

        let all_requested = requested_dataframe_metric_ids(dataset, &[]);
        assert!(all_requested.contains(&"table_a".to_string()));
        assert!(all_requested.contains(&"table_b".to_string()));
    }

    #[test]
    fn warmup_request_scope_uses_dataset_selector_target_when_focus_missing() {
        let request = RuntimeWarmupDatasetRequest {
            scene_id: Some("home".to_string()),
            focus: None,
            dataset_id: "__world_metrics__::scenes/10-地图.mei::metrics".to_string(),
            priority: None,
            metric_id: None,
            metric_ids: Vec::new(),
        };
        let scope = warmup_request_scope(&request);
        assert_eq!(scope.requested_scene_id.as_deref(), Some("home"));
        assert_eq!(
            scope.requested_target_file.as_deref(),
            Some("scenes/10-地图.mei")
        );
    }

    #[test]
    fn aggregate_warmup_requests_derive_target_file_from_dataset_selector() {
        let app = RuntimeWarmupApp {
            app_id: "demo".to_string(),
            default_scene: Some("home".to_string()),
            hot_scenes: vec!["home".to_string()],
            scenes: Vec::new(),
            focuses: vec!["main.mei".to_string()],
            datasets: vec![RuntimeWarmupDatasetRequest {
                scene_id: Some("home".to_string()),
                focus: None,
                dataset_id: "__world_metrics__::scenes/10-地图.mei::metrics".to_string(),
                priority: None,
                metric_id: None,
                metric_ids: Vec::new(),
            }],
            xlsx_sources: Vec::new(),
        };
        let requests = aggregate_warmup_requests(&app, PrebuildScopeProfile::Full);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].scope.key(), "home|scenes/10-地图.mei");
    }

    #[test]
    fn prebuild_warning_classifies_dataset_locate_failure() {
        let warning = build_prebuild_warning(
            "warmup_critical",
            Some("home"),
            Some("scenes/10-地图.mei"),
            None,
            None,
            None,
            None,
            "locate warmup dataset `__world_metrics__::scenes/10-地图.mei::metrics`".to_string(),
        );
        assert_eq!(warning.category, "warmup_dataset_locate_failed");
        assert_eq!(
            warning.dataset_selector.as_deref(),
            Some("__world_metrics__::scenes/10-地图.mei::metrics")
        );
        assert_eq!(warning.scene_id.as_deref(), Some("home"));
        assert_eq!(warning.target_file.as_deref(), Some("scenes/10-地图.mei"));
    }

    #[test]
    fn requested_metric_ids_merge_scalar_and_list_fields() {
        let request = RuntimeWarmupDatasetRequest {
            scene_id: Some("home".to_string()),
            focus: None,
            dataset_id: "demo_ds".to_string(),
            priority: None,
            metric_id: Some("total".to_string()),
            metric_ids: vec!["delta".to_string(), "total".to_string()],
        };

        assert_eq!(
            requested_metric_ids(&request),
            vec!["delta".to_string(), "total".to_string()]
        );
    }

    #[test]
    fn hot_only_warmup_requests_respect_explicit_deferred_priority() {
        let app = RuntimeWarmupApp {
            app_id: "demo".to_string(),
            default_scene: Some("home".to_string()),
            hot_scenes: vec!["home".to_string()],
            scenes: vec!["home".to_string()],
            focuses: vec!["main.mei".to_string()],
            datasets: vec![
                RuntimeWarmupDatasetRequest {
                    scene_id: Some("home".to_string()),
                    focus: Some("main.mei".to_string()),
                    dataset_id: "critical_ds".to_string(),
                    priority: Some("critical".to_string()),
                    metric_id: Some("metric_a".to_string()),
                    metric_ids: Vec::new(),
                },
                RuntimeWarmupDatasetRequest {
                    scene_id: Some("home".to_string()),
                    focus: Some("main.mei".to_string()),
                    dataset_id: "heavy_ds".to_string(),
                    priority: Some("deferred".to_string()),
                    metric_id: Some("metric_b".to_string()),
                    metric_ids: Vec::new(),
                },
            ],
            xlsx_sources: Vec::new(),
        };
        let requests = aggregate_warmup_requests(&app, PrebuildScopeProfile::HotOnly);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].dataset_id, "critical_ds");
        assert_eq!(requests[0].priority, WarmupRequestPriority::Critical);
    }

    #[test]
    fn prebuild_dataframe_metric_selector_rewrites_scalar_metric_to_rowset() {
        let metric_defs = BTreeMap::from([(
            "scenes/01-执法要素.mei::enforcement_items_count".to_string(),
            serde_json::json!({
                "id": "scenes/01-执法要素.mei::enforcement_items_count",
                "shape": "scalar_map"
            }),
        )]);
        assert_eq!(
            prebuild_dataframe_metric_selector(
                &metric_defs,
                "scenes/01-执法要素.mei::enforcement_items_count"
            ),
            "scenes/01-执法要素.mei::enforcement_items_count::__scalar_rowset__"
        );
    }

    #[test]
    fn prebuild_dataframe_metric_selector_keeps_dataframe_metric() {
        let metric_defs = BTreeMap::from([(
            "warnings_realtime_cockpit_table".to_string(),
            serde_json::json!({
                "id": "warnings_realtime_cockpit_table",
                "shape": "dataframe"
            }),
        )]);
        assert_eq!(
            prebuild_dataframe_metric_selector(&metric_defs, "warnings_realtime_cockpit_table"),
            "warnings_realtime_cockpit_table"
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
            PreparedCompileOutcome {
                scope: default_scope,
                outcome: test_outcome("home", "scenes/home.mei"),
            },
            PreparedCompileOutcome {
                scope: home_scope,
                outcome: test_outcome("home", "scenes/home.mei"),
            },
        ];
        let unique = unique_prepared_outcomes_for_artifacts(&prepared);
        assert_eq!(unique.len(), 1);
        assert_eq!(unique[0].scope.requested_scene_id.as_deref(), Some("home"));
    }

    #[test]
    fn observed_count_replays_reports_without_dup_prepared_outcomes() {
        let scope = CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: Some("scenes/home.mei".to_string()),
        };
        let outcome = test_outcome("home", "scenes/home.mei");
        let session = Mutex::new(PrebuildCompileSession::default());
        let mut seen_scopes = BTreeSet::new();
        let mut pending = std::collections::VecDeque::new();
        let mut prepared_outcomes = Vec::new();
        let mut compile_reports = Vec::new();

        record_prebuild_scope_compile_with_discovered(
            &session,
            &scope,
            &outcome,
            Some(&[]),
            3,
            &mut seen_scopes,
            &mut pending,
            &mut prepared_outcomes,
            &mut compile_reports,
        );

        assert_eq!(compile_reports.len(), 3);
        assert_eq!(prepared_outcomes.len(), 1);
        assert!(pending.is_empty());
    }

    #[test]
    fn compile_index_observed_count_comes_from_reports_not_prepared_duplicates() {
        let scope = CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: Some("scenes/home.mei".to_string()),
        };
        let prepared_outcomes = vec![PreparedCompileOutcome {
            scope: scope.clone(),
            outcome: test_outcome("home", "scenes/home.mei"),
        }];
        let compile_reports = vec![
            scope_report_from_outcome(&scope, &test_outcome("home", "scenes/home.mei")),
            scope_report_from_outcome(&scope, &test_outcome("home", "scenes/home.mei")),
            scope_report_from_outcome(&scope, &test_outcome("home", "scenes/home.mei")),
        ];

        let index = build_prebuild_compile_index(
            Path::new("/tmp/ws"),
            "demo",
            &prepared_outcomes,
            &compile_reports,
        );
        let entry = index
            .entries_by_scope_key
            .get(&scope.key())
            .expect("compile index entry");

        assert_eq!(entry.observed_count, 3);
    }

    #[test]
    fn filter_board_discovered_scopes_expands_once_per_board_file() {
        let mut session = PrebuildCompileSession::default();
        let parent = CompileScope {
            requested_scene_id: None,
            requested_target_file: Some("scenes/a.board.mei".to_string()),
        };
        let discovered = vec![
            CompileScope {
                requested_scene_id: Some("s1".to_string()),
                requested_target_file: Some("scenes/a.board.mei".to_string()),
            },
            CompileScope {
                requested_scene_id: Some("s2".to_string()),
                requested_target_file: Some("scenes/a.board.mei".to_string()),
            },
        ];
        let first = session.filter_board_discovered_scopes(&parent, &discovered);
        assert_eq!(first.len(), 2);
        let second = session.filter_board_discovered_scopes(&parent, &discovered);
        assert!(second.is_empty());
    }

    #[test]
    fn clear_runtime_maps_drops_compile_session_indexes() {
        let mut session = PrebuildCompileSession::default();
        let default_scope = CompileScope::default_scope();
        let home_scope = CompileScope {
            requested_scene_id: Some("home".to_string()),
            requested_target_file: Some("scenes/home.mei".to_string()),
        };
        let outcome = test_outcome("home", "scenes/home.mei");

        session.register(
            Path::new("/tmp/ws"),
            "demo",
            &default_scope,
            outcome.clone(),
        );
        session.note_scope_alias(&home_scope, &outcome);

        assert!(!session.by_scope_key.is_empty());
        assert!(!session.by_compile_cache_key.is_empty());
        assert!(!session.by_identity.is_empty());

        session.clear_runtime_maps();

        assert!(session.by_scope_key.is_empty());
        assert!(session.by_compile_cache_key.is_empty());
        assert!(session.by_identity.is_empty());
    }

    #[test]
    fn warmup_request_matches_active_scene_without_exact_scope_key() {
        let request = AggregatedWarmupRequest {
            scope: CompileScope {
                requested_scene_id: Some("home".to_string()),
                requested_target_file: None,
            },
            dataset_id: "penalty_result_dashboard_ds".to_string(),
            priority: WarmupRequestPriority::Critical,
            metric_ids: vec!["penalties_total_count::__scalar_rowset__".to_string()],
        };
        let mut outcome = test_outcome("home", "scenes/home.mei");
        let mut resource = test_dataset_resource("penalty_result_dashboard_ds");
        resource.dataset.as_mut().expect("dataset").runtime_metric_defs.insert(
            "penalties_total_count::__scalar_rowset__".to_string(),
            json!({"shape": "scalar_map"}),
        );
        Arc::make_mut(&mut outcome.compiled).resources.push(resource);
        assert!(warmup_request_matches_outcome(&request, &outcome));
        assert_eq!(
            matching_warmup_requests_for_outcome(&[request], &outcome).len(),
            1
        );
    }

    #[test]
    fn warmup_request_does_not_match_outcome_without_dataset_resource() {
        let request = AggregatedWarmupRequest {
            scope: CompileScope {
                requested_scene_id: Some("home".to_string()),
                requested_target_file: Some("scenes/10-地图.mei".to_string()),
            },
            dataset_id: "__world_metrics__::scenes/10-地图.mei::metrics".to_string(),
            priority: WarmupRequestPriority::Critical,
            metric_ids: Vec::new(),
        };
        let outcome = test_outcome("home", "scenes/10-地图.mei");
        assert!(!warmup_request_matches_outcome(&request, &outcome));
        assert!(
            matching_warmup_requests_for_outcome(&[request], &outcome).is_empty()
        );
    }

    #[test]
    fn parallel_runner_preserves_input_order() {
        let values = run_limited_parallel_ordered(vec![1, 2, 3, 4], 4, |value| value * 10);
        assert_eq!(values, vec![10, 20, 30, 40]);
    }
}
