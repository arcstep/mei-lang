use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::{json, Value};

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

fn next_run_id() -> String {
    format!("run-{}-{}", now_ms(), std::process::id())
}

fn startup_run_root(source_root: &Path) -> PathBuf {
    source_root
        .join(".mei")
        .join("runtime")
        .join("startup-runs")
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

pub(crate) fn initialize(source_root: &Path, startup_policy: &str) {
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
