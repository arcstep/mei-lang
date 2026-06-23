use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use mei_lang_datasets::{
    collect_all_query_options, evaluate_runtime_metrics_from_plan,
    load_metric_dataframe_result_artifact, load_metric_response_result_artifact,
    locate_runtime_metric_resource, metric_dataframe_result_cache_key,
    metric_request_revision_fingerprint_for_compiled, metric_response_cache_scope_key,
    metric_response_prebuild_shared_key,
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
use mei_lang_toolchain::{self as toolchain, PublishDataSnapshotsReport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

const PREBUILD_REPORT_SCHEMA_VERSION: &str = "mei-prebuild-report-v1";
const PREBUILD_MAX_PARALLELISM: usize = 16;

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
        let kb: u64 = String::from_utf8_lossy(&output.stdout).trim().parse().ok()?;
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
        return DirSizeSummary {
            files: 0,
            bytes: 0,
        };
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

fn prebuild_emit_progress(message: impl AsRef<str>) {
    let _guard = prebuild_progress_lock()
        .lock()
        .expect("prebuild progress lock");
    eprintln!("{}", message.as_ref());
    let _ = std::io::stderr().flush();
}

fn format_scope_file(scene: &str, requested_target: &str, active_target: Option<&str>) -> String {
    if !requested_target.is_empty() {
        return requested_target.to_string();
    }
    if let Some(target) = active_target.map(str::trim).filter(|value| !value.is_empty()) {
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

const PREBUILD_COMPILE_INDEX_SCHEMA_VERSION: &str = "mei-prebuild-compile-index-v4";

fn default_observed_count() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedCompileScopeRef {
    requested_scene_id: Option<String>,
    requested_target_file: Option<String>,
}

impl PersistedCompileScopeRef {
    fn to_scope(&self) -> CompileScope {
        compile_scope_from_parts(
            self.requested_scene_id.clone(),
            self.requested_target_file.clone(),
        )
    }
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
    app_root
        .join(".mei")
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
        schema_version: PREBUILD_COMPILE_INDEX_SCHEMA_VERSION.to_string(),
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
    if persisted.schema_version != PREBUILD_COMPILE_INDEX_SCHEMA_VERSION {
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
    let mut best_scope_by_identity =
        BTreeMap::<String, &PreparedCompileOutcome>::new();
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
    let mut entries_by_scope_key = BTreeMap::new();
    for prepared in prepared_outcomes {
        let scope = &prepared.scope;
        let outcome = &prepared.outcome;
        let identity = compiled_scope_identity(outcome);
        let Some(canonical) = best_scope_by_identity.get(&identity) else {
            continue;
        };
        let entry = PersistedPrebuildCompileIndexEntry {
            scope_key: scope.key(),
            requested_scene_id: scope.canonicalized().requested_scene_id,
            requested_target_file: scope.canonicalized().requested_target_file,
            compile_cache_key: toolchain::compile_cache_key(source_root, app_id, &scope.to_options()),
            canonical_scope_key: canonical.scope.key(),
            canonical_requested_scene_id: canonical.scope.canonicalized().requested_scene_id,
            canonical_requested_target_file: canonical.scope.canonicalized().requested_target_file,
            canonical_compile_cache_key: toolchain::compile_cache_key(
                source_root,
                app_id,
                &canonical.scope.to_options(),
            ),
            identity,
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
    PrebuildCompileIndex { entries_by_scope_key }
}

fn compile_active_identity(report: &PrebuildScopeReport) -> String {
    format!(
        "{}|{}",
        report.active_scene_id.as_deref().unwrap_or(""),
        report.active_target_file
    )
}

fn emit_prebuild_optimization_report(
    app_id: &str,
    app_root: &Path,
    reports: &[PrebuildScopeReport],
    coverage: &PrebuildCoverageReport,
    diagnostics: &PrebuildDiagnostics,
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
        .map(|report| report.cache_lookup_ms.saturating_add(report.artifact_load_ms))
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
    if postload_identity_collapses > 0 {
        prebuild_emit_progress(format!(
            "  load 后 identity 折叠 {postload_identity_collapses} 次（不同请求 scope 收敛到同一编译结果）"
        ));
    }
    prebuild_emit_progress(format!(
        "  逻辑产物 | 数据集导入 {} | metric response {} | metric dataframe {}",
        coverage.dataset_import_artifacts_ready,
        coverage.metric_response_artifacts_ready,
        coverage.metric_dataframe_artifacts_ready,
    ));

    let eval_root = app_root.join(".mei").join("eval-artifacts");
    let response_dir = eval_root.join("results").join("metric-response");
    let dataframe_dir = eval_root.join("results").join("metric-dataframe");
    let response_disk = dir_size_summary(response_dir.as_path());
    let dataframe_disk = dir_size_summary(dataframe_dir.as_path());
    let eval_disk = dir_size_summary(eval_root.as_path());
    prebuild_emit_progress(format!(
        "■ 磁盘占用 | eval-artifacts 合计 {} ({} 文件)",
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
            prebuild_emit_progress(format!(
                "■ 内存 | 进程 RSS 当前 {}",
                format_bytes(current),
            ));
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
    if expansion_ratio >= 2.0 && redundant_checks > 0 && compile_index_hits == 0 && preload_reuse_hits == 0 {
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

struct CoverageState {
    metric_response_jobs: ArtifactSingleflightState,
    metric_dataframe_jobs: ArtifactSingleflightState,
    metric_response_exact: Arc<Mutex<BTreeMap<String, LoadedMetricResponseArtifact>>>,
    metric_response_shared: Arc<Mutex<BTreeMap<String, LoadedMetricResponseArtifact>>>,
    metric_dataframe_exact: Arc<Mutex<BTreeMap<String, DatasetQueryResult>>>,
    metric_dataframe_shared: Arc<Mutex<BTreeMap<String, DatasetQueryResult>>>,
    diagnostics: Arc<PrebuildDiagnostics>,
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
    prebuild_emit_progress(&format!(
        "prebuild {} | workspace={} | apps={}",
        match options.mode {
            PrebuildMode::Build => "构建",
            PrebuildMode::Verify => "校验",
        },
        source_root.display(),
        manifest
            .apps
            .iter()
            .map(|app| app.app_id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
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
    let warmup_requests = aggregate_warmup_requests(app);
    let max_parallelism = prebuild_parallelism(
        compile_scopes_for_app(app)
            .len()
            .max(warmup_requests.len())
            .max(1),
    );
    let default_scope = CompileScope::default_scope();
    let compile_started = Instant::now();
    let initial_scope_count = compile_scopes_for_app(app).len();
    prebuild_emit_progress(&format!(
        "[{}] ── 1/3 编译 .mei ── 约 {initial_scope_count} 个 manifest scope（request-scope 闭包 + 结果复用）",
        app.app_id
    ));
    let mut scopes = compile_scopes_for_app(app);
    scopes.retain(|scope| scope.key() != default_scope.key());
    let hot_scope_keys = app
        .hot_scenes
        .iter()
        .map(|scene| format!("{}|", scene.trim()))
        .filter(|key| key != "|")
        .collect::<BTreeSet<_>>();
    let (hot_scopes, deferred_scopes): (Vec<_>, Vec<_>) = scopes
        .into_iter()
        .partition(|scope| hot_scope_keys.contains(&scope.key()));
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
        if default_outcome.cache_hit { "命中" } else { "未命中" },
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
                    (!discovered_scopes.is_empty()).then_some(discovered_scopes.as_slice()),
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
                warnings.push(format!(
                    "compile scope scene=`{}` target=`{}` failed: {error}",
                    scope.requested_scene_id.as_deref().unwrap_or(""),
                    scope.requested_target_file.as_deref().unwrap_or(""),
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
    while !pending.is_empty() {
        batch_idx += 1;
        let batch = pending.drain(..).collect::<Vec<_>>();
        let batch_size = batch.len();
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
        let compile_groups =
            group_scopes_by_compile_cache_key(source_root, app.app_id.as_str(), to_compile_after_index);
        let unique_keys = compile_groups.len();
        prebuild_emit_progress(&format!(
            "[{}] 编译 batch-{batch_idx} | {batch_size} scope | session 复用 {session_hit_count} | index 复用 {} | 唯一 cache key {unique_keys}",
            app.app_id,
            index_hit_count
        ));
        let batch_started = Instant::now();
        let representatives = compile_groups
            .iter()
            .map(|(scope, _)| scope.clone())
            .collect::<Vec<_>>();
        let batch_results = run_limited_parallel_ordered(
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
                    warnings.push(format!(
                        "compile scope scene=`{}` target=`{}` failed: {error}",
                        scope.requested_scene_id.as_deref().unwrap_or(""),
                        scope.requested_target_file.as_deref().unwrap_or(""),
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
                compile_session
                    .lock()
                    .expect("prebuild compile session lock")
                    .register(source_root, app.app_id.as_str(), &alias, outcome.clone());
                record_prebuild_scope_compile(
                    compile_session.as_ref(),
                    &alias,
                    outcome,
                    &mut seen_scopes,
                    &mut pending,
                    &mut prepared_outcomes,
                    &mut compile_reports,
                );
            }
        }
        prebuild_emit_progress(&format!(
            "[{}] 编译 batch-{batch_idx} 完成 {:.1}s | 新编译 {batch_compiled} | 缓存 {batch_cache_hit}",
            app.app_id,
            batch_started.elapsed().as_secs_f64()
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
    coverage.dataset_import_artifacts_ready = required_xlsx_sources.len();
    let _ = mei_lang_kernel::clear_runtime_eval_node_cache();
    let coverage_state = CoverageState {
        diagnostics: Arc::clone(&diagnostics),
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
    let artifact_outcomes_for_warmup = artifact_outcomes.clone();
    let scope_artifacts_started = Instant::now();
    prebuild_emit_progress(&format!(
        "[{}] ── 2/3 生成 metric 产物 ── {} 个编译结果待处理（response + dataframe 落盘）",
        app.app_id,
        artifact_outcomes.len()
    ));
    let artifact_total = artifact_outcomes.len();
    let artifacts_started = Arc::new(Instant::now());
    let scope_results = run_limited_parallel_ordered_with_hook(
        artifact_outcomes,
        max_parallelism,
        |prepared| {
            let mut local_coverage = PrebuildCoverageReport::default();
            let matching_requests =
                matching_warmup_requests_for_outcome(&warmup_requests, &prepared.outcome);
            let started = Instant::now();
            let result = ensure_scope_artifacts(
                app.app_id.as_str(),
                app_root.as_path(),
                &prepared.scope,
                &prepared.outcome,
                matching_requests.as_slice(),
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
            move |index, (scope, result, local_coverage, wall_time): &(
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
    let warmup_started = Instant::now();
    let warmup_results = run_limited_parallel_ordered(
        warmup_requests_to_run,
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
    coverage_state.clear();
    let _ = mei_lang_datasets::clear_all_metric_caches();
    let _ = mei_lang_kernel::clear_runtime_eval_node_cache();
    let warmup_requests_ms = warmup_started.elapsed().as_millis() as u64;
    if let Err(error) =
        mei_lang_datasets::rebuild_and_install_prebuild_metric_response_index(app_root.as_path())
    {
        warnings.push(format!("metric response index rebuild failed: {error}"));
    }
    emit_prebuild_optimization_report(
        app.app_id.as_str(),
        app_root.as_path(),
        compile_reports.as_slice(),
        &coverage,
        diagnostics.as_ref(),
        compile_scopes_ms,
        scope_artifacts_ms,
        max_parallelism,
        warnings.len(),
        canonical_identity_count,
        session_entries_before_clear,
        session_entries_after_clear,
        warmup_reuse_hits,
    );
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
    true
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

#[derive(Default)]
struct PrebuildCompileSession {
    by_scope_key: BTreeMap<String, SharedCompileOutcome>,
    by_compile_cache_key: BTreeMap<String, SharedCompileOutcome>,
    by_identity: BTreeMap<String, SharedCompileOutcome>,
    discovered_scope_keys: BTreeSet<String>,
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
        let cache_key =
            toolchain::compile_cache_key(source_root, app_id, &scope.to_options());
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
        let cache_key =
            toolchain::compile_cache_key(source_root, app_id, &scope.to_options());
        if let Some(outcome) = self.by_compile_cache_key.get(&cache_key) {
            return Some(mark_prebuild_session_reuse(outcome));
        }
        if let Some(outcome) = self.by_scope_key.get(&scope.key()) {
            return Some(mark_prebuild_session_reuse(outcome));
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
    if let Some(outcome) = session_try_reuse(compile_session, source_root, app_id, &canonical_scope) {
        diagnostics.compile_index_hits.fetch_add(1, Ordering::Relaxed);
        compile_session
            .lock()
            .expect("prebuild compile session lock")
            .register(source_root, app_id, scope, outcome.clone());
        return Some(PersistedCompileIndexReuse {
            outcome: mark_prebuild_session_reuse(&outcome),
            discovered_scopes: entry
                .discovered_scopes
                .iter()
                .map(PersistedCompileScopeRef::to_scope)
                .collect(),
            observed_count: entry.observed_count.max(1),
        });
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
    if compiled_scope_identity(&outcome) != entry.identity {
        diagnostics
            .compile_index_stale_entries
            .fetch_add(1, Ordering::Relaxed);
        return None;
    }
    diagnostics.compile_index_hits.fetch_add(1, Ordering::Relaxed);
    let mut locked = compile_session
        .lock()
        .expect("prebuild compile session lock");
    locked.register(source_root, app_id, &canonical_scope, outcome.clone());
    locked.register(source_root, app_id, scope, outcome.clone());
    Some(PersistedCompileIndexReuse {
        outcome: mark_prebuild_session_reuse(&outcome),
        discovered_scopes: entry
            .discovered_scopes
            .iter()
            .map(PersistedCompileScopeRef::to_scope)
            .collect(),
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
        let discovered_scopes = compile_index
            .and_then(|index| index.entries_by_scope_key.get(&scope.key()))
            .map(|entry| {
                entry
                    .discovered_scopes
                    .iter()
                    .map(PersistedCompileScopeRef::to_scope)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        diagnostics
            .compile_preload_reuse_hits
            .fetch_add(1, Ordering::Relaxed);
        session
            .lock()
            .expect("prebuild compile session lock")
            .note_scope_alias(scope, &reused);
        return Some(PersistedCompileIndexReuse {
            outcome: reused,
            discovered_scopes,
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

    let outcome = match mode {
        PrebuildMode::Build | PrebuildMode::Verify => toolchain::load_compile_artifact_only_shared(
            source_root,
            app_id,
            &scope.to_options(),
            components_root,
        ),
    };
    let outcome = match outcome {
        Some(outcome) => SharedCompileOutcome::from_shared(outcome),
        None => ensure_compile_scope(source_root, app_id, scope, mode, components_root)?,
    };
    let identity = compiled_scope_identity(&outcome);
    let mut locked = session
        .lock()
        .expect("prebuild compile session lock");
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
    if compile_session
        .lock()
        .expect("prebuild compile session lock")
        .should_discover(scope)
    {
        let discovered_iter = discovered_scopes
            .map(|scopes| scopes.to_vec())
            .unwrap_or_else(|| discovered_compile_scopes(scope, &outcome.compiled));
        for discovered in discovered_iter {
            if seen_scopes.insert(discovered.key()) {
                pending.push_back(discovered);
            }
        }
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
        PrebuildMode::Build => toolchain::compile_app_with_cache_shared(
            source_root,
            app_id,
            options,
            components_root,
        )
        .map(SharedCompileOutcome::from_shared)
        .map_err(|failure| failure.error)
        .with_context(|| {
            format!(
                "compile scope scene=`{}` target=`{}` for app `{app_id}`",
                scope.requested_scene_id.as_deref().unwrap_or(""),
                scope.requested_target_file.as_deref().unwrap_or("")
            )
        }),
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
    scope: &CompileScope,
    outcome: &SharedCompileOutcome,
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
    outcome: &SharedCompileOutcome,
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
    outcome: &SharedCompileOutcome,
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

fn should_auto_discover_scope_artifacts(scope: &CompileScope, outcome: &SharedCompileOutcome) -> bool {
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
    outcome: &SharedCompileOutcome,
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
    outcome: &SharedCompileOutcome,
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
    let (scene_id, scene_path) =
        artifact_scene_context_for_resource(&outcome.compiled, access_plan.owner.id.as_str());
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
    let shared_cache_key = metric_response_prebuild_shared_key(
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
    prebuild_emit_progress(format!(
        "[{app_id}] 指标求值开始 | response | dataset={} | scene={scene_id}",
        short_dataset_id(dataset_selector)
    ));
    let metric_started = Instant::now();
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
    prebuild_emit_progress(format!(
        "[{app_id}] 指标求值 {:.1}s | response | dataset={} | scene={scene_id} | rows={}",
        metric_started.elapsed().as_secs_f64(),
        short_dataset_id(dataset_selector),
        eval_outcome.total_rows
    ));
    state.diagnostics.record_metric_build(
        "response",
        dataset_selector,
        "(bundle)",
        &scene_id,
        metric_started.elapsed().as_millis() as u64,
    );
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
    outcome: &SharedCompileOutcome,
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
    let (scene_id, scene_path) =
        artifact_scene_context_for_resource(&outcome.compiled, owner_resource.id.as_str());
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
    prebuild_emit_progress(format!(
        "[{}] 指标求值开始 | dataframe | {} | metric={} | scene={scene_id}",
        app_root.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        short_dataset_id(resource.id.as_str()),
        short_metric_id(resolved_metric_id.as_str())
    ));
    let metric_started = Instant::now();
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
    prebuild_emit_progress(format!(
        "[{}] 指标求值 {:.1}s | dataframe | {} | metric={} | scene={scene_id} | rows={}",
        app_root.file_name().and_then(|s| s.to_str()).unwrap_or(""),
        metric_started.elapsed().as_secs_f64(),
        short_dataset_id(resource.id.as_str()),
        short_metric_id(resolved_metric_id.as_str()),
        result.total
    ));
    state.diagnostics.record_metric_build(
        "dataframe",
        resource.id.as_str(),
        resolved_metric_id.as_str(),
        scene_id.as_str(),
        metric_started.elapsed().as_millis() as u64,
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::CompiledApp;

    fn test_outcome(active_scene: &str, active_target_file: &str) -> SharedCompileOutcome {
        SharedCompileOutcome {
            compiled: Arc::new(CompiledApp {
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
        assert_eq!(
            unique[0].scope.requested_scene_id.as_deref(),
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
