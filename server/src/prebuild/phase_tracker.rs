use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::prebuild_emit_progress;
use super::prebuild_progress_heartbeat_secs;

pub(crate) const PREBUILD_PROGRESS_JSON_REL: &str = "runtime/prebuild-progress.json";
pub(crate) const PREBUILD_PROGRESS_SCHEMA_VERSION: &str = "mei-prebuild-progress-v1";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrebuildProgressSnapshot {
    pub schema_version: String,
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub elapsed_ms: u64,
    pub updated_at_ms: u64,
}

struct PrebuildPhaseTrackerInner {
    started: Instant,
    phase: String,
    app_id: Option<String>,
    detail: Option<String>,
    heartbeat: Option<JoinHandle<()>>,
}

pub(crate) struct PrebuildPhaseTracker {
    inner: Mutex<PrebuildPhaseTrackerInner>,
}

impl PrebuildPhaseTracker {
    fn new() -> Self {
        Self {
            inner: Mutex::new(PrebuildPhaseTrackerInner {
                started: Instant::now(),
                phase: "init".to_string(),
                app_id: None,
                detail: None,
                heartbeat: None,
            }),
        }
    }

    pub(crate) fn global() -> &'static PrebuildPhaseTracker {
        static TRACKER: OnceLock<PrebuildPhaseTracker> = OnceLock::new();
        TRACKER.get_or_init(PrebuildPhaseTracker::new)
    }

    pub(crate) fn begin_session(source_root: &Path) {
        let tracker = Self::global();
        let root = source_root.to_path_buf();
        let started = Instant::now();
        if let Ok(mut guard) = tracker.inner.lock() {
            guard.started = started;
            guard.phase = "cli_prepare".to_string();
            guard.app_id = None;
            guard.detail = Some("初始化 prebuild 会话".to_string());
            if let Some(handle) = guard.heartbeat.take() {
                drop(guard);
                let _ = handle.join();
            } else {
                drop(guard);
            }
            if let Ok(mut guard) = tracker.inner.lock() {
                guard.heartbeat = Some(spawn_heartbeat(root.clone()));
            }
        }
        tracker.write_json(source_root);
    }

    pub(crate) fn end_session(source_root: &Path) {
        let tracker = Self::global();
        let heartbeat = if let Ok(mut guard) = tracker.inner.lock() {
            guard.phase = "finished".to_string();
            guard.heartbeat.take()
        } else {
            None
        };
        if let Some(handle) = heartbeat {
            let _ = handle.join();
        }
        tracker.write_json(source_root);
    }

    pub(crate) fn set_phase(
        &self,
        source_root: &Path,
        phase: &str,
        app_id: Option<&str>,
        detail: Option<&str>,
    ) {
        let message = match (app_id, detail) {
            (Some(app), Some(detail)) if !detail.trim().is_empty() => {
                format!("[{app}] {phase} — {detail}")
            }
            (Some(app), _) => format!("[{app}] {phase}"),
            (None, Some(detail)) if !detail.trim().is_empty() => format!("{phase} — {detail}"),
            _ => phase.to_string(),
        };
        prebuild_emit_progress(message);
        if let Ok(mut guard) = self.inner.lock() {
            guard.phase = phase.to_string();
            guard.app_id = app_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            guard.detail = detail
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
        }
        self.write_json(source_root);
    }

    pub(crate) fn snapshot(&self) -> PrebuildProgressSnapshot {
        let guard = self
            .inner
            .lock()
            .expect("prebuild phase tracker lock");
        PrebuildProgressSnapshot {
            schema_version: PREBUILD_PROGRESS_SCHEMA_VERSION.to_string(),
            phase: guard.phase.clone(),
            app_id: guard.app_id.clone(),
            detail: guard.detail.clone(),
            elapsed_ms: guard.started.elapsed().as_millis() as u64,
            updated_at_ms: now_ms(),
        }
    }

    pub(crate) fn write_json(&self, source_root: &Path) {
        let path = source_root.join(PREBUILD_PROGRESS_JSON_REL);
        let Some(parent) = path.parent() else {
            return;
        };
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
        let snapshot = self.snapshot();
        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            let _ = std::fs::write(path, json);
        }
    }
}

fn spawn_heartbeat(source_root: PathBuf) -> JoinHandle<()> {
    thread::spawn(move || {
        let interval = Duration::from_secs(prebuild_progress_heartbeat_secs());
        let tracker = PrebuildPhaseTracker::global();
        loop {
            thread::sleep(interval);
            let (phase, app_id, detail, elapsed_ms) = {
                let guard = tracker.inner.lock().expect("prebuild phase tracker lock");
                (
                    guard.phase.clone(),
                    guard.app_id.clone(),
                    guard.detail.clone(),
                    guard.started.elapsed().as_millis() as u64,
                )
            };
            if phase == "finished" {
                break;
            }
            let label = app_id.as_deref().unwrap_or("workspace");
            let suffix = detail
                .as_deref()
                .map(|value| format!(" | {value}"))
                .unwrap_or_default();
            prebuild_emit_progress(format!(
                "heartbeat {label} phase={phase}{suffix} elapsed={elapsed_ms}ms"
            ));
            tracker.write_json(source_root.as_path());
        }
    })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) struct PrebuildPhaseSession {
    source_root: PathBuf,
}

impl PrebuildPhaseSession {
    pub(crate) fn begin(source_root: &Path) -> Self {
        PrebuildPhaseTracker::begin_session(source_root);
        Self {
            source_root: source_root.to_path_buf(),
        }
    }
}

impl Drop for PrebuildPhaseSession {
    fn drop(&mut self) {
        PrebuildPhaseTracker::end_session(self.source_root.as_path());
    }
}
