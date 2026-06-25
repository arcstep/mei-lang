use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};

use crate::prebuild::PrebuildReport;

const STARTUP_RUN_SCHEMA_VERSION: &str = "mei-startup-run-v1";

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StartupRunSnapshot {
    pub schema_version: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "sourceRoot")]
    pub source_root: String,
    #[serde(rename = "artifactDir")]
    pub artifact_dir: String,
    #[serde(rename = "startupPolicy")]
    pub startup_policy: String,
    #[serde(rename = "buildDescriptor")]
    pub build_descriptor: Value,
    #[serde(rename = "hostPid")]
    pub host_pid: u32,
    pub phase: String,
    #[serde(rename = "startedAtMs")]
    pub started_at_ms: u64,
    #[serde(rename = "updatedAtMs")]
    pub updated_at_ms: u64,
    #[serde(rename = "finishedAtMs")]
    pub finished_at_ms: Option<u64>,
    #[serde(rename = "accessReady")]
    pub access_ready: bool,
    #[serde(rename = "fullWarmupReady")]
    pub full_warmup_ready: bool,
    #[serde(rename = "deferredWarmupPending")]
    pub deferred_warmup_pending: bool,
    #[serde(rename = "accessArtifactsReady", skip_serializing_if = "Option::is_none")]
    pub access_artifacts_ready: Option<bool>,
    #[serde(rename = "startupOutcome", skip_serializing_if = "Option::is_none")]
    pub startup_outcome: Option<String>,
    #[serde(rename = "startupWarmupKind", skip_serializing_if = "Option::is_none")]
    pub startup_warmup_kind: Option<String>,
    #[serde(rename = "lastWarningCount", skip_serializing_if = "Option::is_none")]
    pub last_warning_count: Option<usize>,
    #[serde(rename = "lastFailedAppCount", skip_serializing_if = "Option::is_none")]
    pub last_failed_app_count: Option<usize>,
    #[serde(rename = "correctnessFailed", skip_serializing_if = "Option::is_none")]
    pub correctness_failed: Option<bool>,
    #[serde(rename = "warningCategories", skip_serializing_if = "Vec::is_empty", default)]
    pub warning_categories: Vec<String>,
    #[serde(
        rename = "warningCategoryCounts",
        skip_serializing_if = "serde_json::Map::is_empty",
        default
    )]
    pub warning_category_counts: serde_json::Map<String, Value>,
    #[serde(rename = "failingDatasets", skip_serializing_if = "Vec::is_empty", default)]
    pub failing_datasets: Vec<String>,
    pub finished: bool,
}

#[derive(Debug, Serialize)]
struct StartupRunTimelineEvent {
    pub schema_version: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    pub event: String,
    #[serde(rename = "atMs")]
    pub at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
}

#[derive(Debug, Clone)]
struct StartupRunState {
    run_dir: PathBuf,
    timeline_path: PathBuf,
    summary: StartupRunSnapshot,
}

fn startup_run_state() -> &'static Mutex<Option<StartupRunState>> {
    static STATE: OnceLock<Mutex<Option<StartupRunState>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn now_ms_for_host_message() -> u64 {
    now_ms()
}

fn next_run_id() -> String {
    format!("run-{}-{}", now_ms(), std::process::id())
}

fn startup_run_root(source_root: &Path) -> PathBuf {
    mei_lang_kernel::resolve_workspace_runtime_root(source_root).join("startup-runs")
}

fn with_state<T>(f: impl FnOnce(&mut StartupRunState) -> T) -> Option<T> {
    startup_run_state()
        .lock()
        .ok()
        .and_then(|mut guard| guard.as_mut().map(f))
}

fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("tmp");
    let body = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    fs::write(&tmp_path, body)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn append_jsonl(path: &Path, value: &impl Serialize) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, value).map_err(io::Error::other)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn persist_run_json(state: &StartupRunState) {
    let run_json_path = state.run_dir.join("run.json");
    if let Err(error) = write_json(run_json_path.as_path(), &state.summary) {
        tracing::warn!(%error, path = %run_json_path.display(), "failed to persist startup run summary");
    }
}

pub(crate) fn current_started_at_ms() -> Option<u64> {
    startup_run_state()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|state| state.summary.started_at_ms))
}

pub(crate) fn initialize(source_root: &Path, startup_policy: &str) {
    crate::http::host_log::initialize(source_root);
    crate::http::host_log::prune_startup_runs(source_root);
    let source_root_text = source_root.display().to_string();
    let run_id = next_run_id();
    let run_dir = startup_run_root(source_root).join(run_id.as_str());
    let timeline_path = run_dir.join("timeline.jsonl");
    let started_at_ms = now_ms();
    let summary = StartupRunSnapshot {
        schema_version: STARTUP_RUN_SCHEMA_VERSION.to_string(),
        run_id: run_id.clone(),
        source_root: source_root_text,
        artifact_dir: run_dir.display().to_string(),
        startup_policy: startup_policy.to_string(),
        build_descriptor: crate::build_info::descriptor(),
        host_pid: std::process::id(),
        phase: "serve_started".to_string(),
        started_at_ms,
        updated_at_ms: started_at_ms,
        finished_at_ms: None,
        access_ready: false,
        full_warmup_ready: false,
        deferred_warmup_pending: false,
        access_artifacts_ready: None,
        startup_outcome: None,
        startup_warmup_kind: None,
        last_warning_count: None,
        last_failed_app_count: None,
        correctness_failed: None,
        warning_categories: Vec::new(),
        warning_category_counts: serde_json::Map::new(),
        failing_datasets: Vec::new(),
        finished: false,
    };
    let mut guard = match startup_run_state().lock() {
        Ok(guard) => guard,
        Err(error) => {
            tracing::warn!(%error, "failed to lock startup run state");
            return;
        }
    };
    *guard = Some(StartupRunState {
        run_dir,
        timeline_path,
        summary,
    });
    if let Some(state) = guard.as_ref() {
        persist_run_json(state);
    }
    drop(guard);
    record_phase("serve_started", None);
}

pub(crate) fn current_snapshot() -> Option<StartupRunSnapshot> {
    startup_run_state()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|state| state.summary.clone()))
}

pub(crate) fn current_run_id() -> Option<String> {
    current_snapshot().map(|snapshot| snapshot.run_id)
}

pub(crate) fn current_startup_policy() -> Option<String> {
    current_snapshot().map(|snapshot| snapshot.startup_policy)
}

pub(crate) fn current_artifact_dir() -> Option<String> {
    current_snapshot().map(|snapshot| snapshot.artifact_dir)
}

pub(crate) fn record_phase(event: &str, detail: Option<Value>) {
    let event_text = event.trim();
    if event_text.is_empty() {
        return;
    }
    let _ = with_state(|state| {
        let at_ms = now_ms();
        state.summary.phase = event_text.to_string();
        state.summary.updated_at_ms = at_ms;
        if event_text == "startup_finished" {
            state.summary.finished = true;
            state.summary.finished_at_ms = Some(at_ms);
            if let Some(detail) = detail.as_ref() {
                if let Some(outcome) = detail.get("startupOutcome").and_then(Value::as_str) {
                    state.summary.startup_outcome = Some(outcome.to_string());
                }
                if let Some(access) = detail.get("accessArtifactsReady").and_then(Value::as_bool) {
                    state.summary.access_artifacts_ready = Some(access);
                }
                if let Some(kind) = detail.get("warmupKind").and_then(Value::as_str) {
                    state.summary.startup_warmup_kind = Some(kind.to_string());
                }
                if let Some(count) = detail.get("warningCount").and_then(Value::as_u64) {
                    state.summary.last_warning_count = Some(count as usize);
                }
                if let Some(count) = detail.get("failedAppCount").and_then(Value::as_u64) {
                    state.summary.last_failed_app_count = Some(count as usize);
                }
                if let Some(failed) = detail.get("correctnessFailed").and_then(Value::as_bool) {
                    state.summary.correctness_failed = Some(failed);
                }
                if let Some(categories) = detail.get("warningCategories").and_then(Value::as_array) {
                    state.summary.warning_categories = categories
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect();
                }
                if let Some(counts) = detail.get("warningCategoryCounts").and_then(Value::as_object)
                {
                    state.summary.warning_category_counts = counts.clone();
                }
                if let Some(datasets) = detail.get("failingDatasets").and_then(Value::as_array) {
                    state.summary.failing_datasets = datasets
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect();
                }
            }
        }
        persist_run_json(state);
        let record = StartupRunTimelineEvent {
            schema_version: STARTUP_RUN_SCHEMA_VERSION.to_string(),
            run_id: state.summary.run_id.clone(),
            event: event_text.to_string(),
            at_ms,
            detail,
        };
        if let Err(error) = append_jsonl(state.timeline_path.as_path(), &record) {
            tracing::warn!(%error, path = %state.timeline_path.display(), "failed to append startup timeline event");
        }
    });
}

pub(crate) fn update_readiness_snapshot(
    phase: &str,
    access_ready: bool,
    full_warmup_ready: bool,
    deferred_warmup_pending: bool,
    snapshot: &impl Serialize,
) {
    let _ = with_state(|state| {
        let at_ms = now_ms();
        state.summary.phase = phase.to_string();
        state.summary.updated_at_ms = at_ms;
        state.summary.access_ready = access_ready;
        state.summary.full_warmup_ready = full_warmup_ready;
        state.summary.deferred_warmup_pending = deferred_warmup_pending;
        persist_run_json(state);
        let readiness_path = state.run_dir.join("readiness-final.json");
        if let Err(error) = write_json(readiness_path.as_path(), snapshot) {
            tracing::warn!(%error, path = %readiness_path.display(), "failed to persist readiness snapshot");
        }
    });
}

fn infer_startup_warmup_kind(report: &PrebuildReport) -> &'static str {
    let diagnostics = &report.diagnostics;
    if diagnostics.real_compile_count == 0 {
        if diagnostics.cache_hit_count > 0
            || diagnostics.compile_index.hits > 0
            || diagnostics.warmup_reuse_hits > 0
        {
            "incremental_cache"
        } else {
            "verify_only"
        }
    } else {
        "cold_or_rebuild"
    }
}

pub(crate) fn record_startup_prebuild_outcome(
    prebuild_slot: &str,
    report: &PrebuildReport,
    access_artifacts_ready: bool,
    warning_count: usize,
    failed_app_count: usize,
    compile_ms: u64,
    warmup_ms: u64,
    startup_sequence_complete: bool,
) {
    let warmup_kind = infer_startup_warmup_kind(report);
    let warning_categories = report.warning_categories();
    let warning_category_counts = serde_json::to_value(report.warning_category_counts())
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let failing_datasets = report.failing_datasets();
    let correctness_failed = report.correctness_failed();
    record_phase(
        "startup_prebuild_finished",
        Some(json!({
            "slot": prebuild_slot,
            "scopeProfile": format!("{:?}", report.scope_profile),
            "accessArtifactsReady": access_artifacts_ready,
            "ok": report.ok,
            "warmupKind": warmup_kind,
            "failedAppCount": failed_app_count,
            "warningCount": warning_count,
            "totalWallMs": report.total_wall_ms,
            "compileMs": compile_ms,
            "warmupMs": warmup_ms,
            "realCompileCount": report.diagnostics.real_compile_count,
            "cacheHitCount": report.diagnostics.cache_hit_count,
            "compileIndexHits": report.diagnostics.compile_index.hits,
            "compileIndexMisses": report.diagnostics.compile_index.misses,
            "correctnessFailed": correctness_failed,
            "warningCategories": warning_categories,
            "warningCategoryCounts": warning_category_counts,
            "failingDatasets": failing_datasets,
        })),
    );
    if !access_artifacts_ready {
        record_phase(
            "access_not_ready",
            Some(json!({
                "slot": prebuild_slot,
                "failedAppCount": failed_app_count,
                "warningCount": warning_count,
                "ok": report.ok,
                "correctnessFailed": correctness_failed,
            })),
        );
    } else if startup_sequence_complete {
        record_phase(
            "access_ready",
            Some(json!({
                "slot": prebuild_slot,
                "totalWallMs": report.total_wall_ms,
            })),
        );
    }
    if !startup_sequence_complete {
        return;
    }
    let startup_ok = report.ok && access_artifacts_ready;
    let startup_outcome = if startup_ok {
        "ready"
    } else {
        "not_ready"
    };
    let _ = with_state(|state| {
        state.summary.startup_warmup_kind = Some(warmup_kind.to_string());
        state.summary.startup_outcome = Some(startup_outcome.to_string());
        state.summary.access_artifacts_ready = Some(access_artifacts_ready);
        state.summary.last_warning_count = Some(warning_count);
        state.summary.last_failed_app_count = Some(failed_app_count);
        state.summary.correctness_failed = Some(correctness_failed);
        state.summary.warning_categories = report.warning_categories();
        state.summary.warning_category_counts = serde_json::to_value(report.warning_category_counts())
            .ok()
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        state.summary.failing_datasets = report.failing_datasets();
    });
    if report.ok && access_artifacts_ready {
        record_phase(
            "full_warmup_ready",
            Some(json!({
                "slot": prebuild_slot,
                "totalWallMs": report.total_wall_ms,
            })),
        );
    }
    record_phase(
        "startup_finished",
        Some(json!({
            "ok": startup_ok,
            "startupOutcome": startup_outcome,
            "warmupKind": warmup_kind,
            "accessArtifactsReady": access_artifacts_ready,
            "failedAppCount": failed_app_count,
            "warningCount": warning_count,
            "totalWallMs": report.total_wall_ms,
            "correctnessFailed": correctness_failed,
            "warningCategories": report.warning_categories(),
            "warningCategoryCounts": report.warning_category_counts(),
            "failingDatasets": report.failing_datasets(),
        })),
    );
}

pub(crate) fn write_prebuild_report(slot: &str, report: &impl Serialize) {
    let file_name = format!("prebuild-{}.json", slot.trim());
    write_named_json(file_name.as_str(), report);
}

pub(crate) fn write_prebuild_error(slot: &str, error: &str, detail: Option<Value>) {
    let file_name = format!("prebuild-{}.json", slot.trim());
    let value = json!({
        "schema_version": STARTUP_RUN_SCHEMA_VERSION,
        "run_id": current_run_id(),
        "slot": slot,
        "ok": false,
        "error": error,
        "detail": detail,
    });
    write_named_json(file_name.as_str(), &value);
}

pub(crate) fn write_request_trace_record(record: &impl Serialize) {
    let _ = with_state(|state| {
        let path = state.run_dir.join("request-trace.jsonl");
        if let Err(error) = append_jsonl(path.as_path(), record) {
            tracing::warn!(%error, path = %path.display(), "failed to append request trace record");
        }
    });
}

pub(crate) fn write_request_trace_summary(summary: &impl Serialize) {
    write_named_json("request-trace-summary.json", summary);
}

pub(crate) fn write_named_json(name: &str, value: &impl Serialize) {
    let _ = with_state(|state| {
        let path = state.run_dir.join(name);
        if let Err(error) = write_json(path.as_path(), value) {
            tracing::warn!(%error, path = %path.display(), "failed to persist startup run artifact");
        }
    });
}
