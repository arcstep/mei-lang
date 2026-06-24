use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::AppState;

const DEFAULT_MAX_BYTES: u64 = 5 * 1024 * 1024;
const DEFAULT_MAX_FILES: usize = 5;
const DEFAULT_RETAIN_RUNS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HostLogEntry {
    #[serde(rename = "recordedAtMs")]
    pub recorded_at_ms: u128,
    pub level: String,
    pub target: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(rename = "runId", skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

struct FieldVisitor {
    message: Option<String>,
    fields: serde_json::Map<String, serde_json::Value>,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            message: None,
            fields: serde_json::Map::new(),
        }
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let key = field.name().to_string();
        if key == "message" {
            self.message = Some(format!("{value:?}").trim_matches('"').to_string());
            return;
        }
        self.fields
            .insert(key, serde_json::Value::String(format!("{value:?}").replace('"', "")));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let key = field.name().to_string();
        if key == "message" {
            self.message = Some(value.to_string());
            return;
        }
        self.fields
            .insert(key, serde_json::Value::String(value.to_string()));
    }
}

struct HostLogWriter {
    log_dir: PathBuf,
    max_bytes: u64,
    max_files: usize,
}

impl HostLogWriter {
    fn new(log_dir: PathBuf) -> Self {
        Self {
            log_dir,
            max_bytes: std::env::var("MEI_HOST_LOG_MAX_BYTES")
                .ok()
                .and_then(|raw| raw.parse().ok())
                .filter(|value| *value >= 64 * 1024)
                .unwrap_or(DEFAULT_MAX_BYTES),
            max_files: std::env::var("MEI_HOST_LOG_MAX_FILES")
                .ok()
                .and_then(|raw| raw.parse().ok())
                .filter(|value| *value >= 2)
                .unwrap_or(DEFAULT_MAX_FILES),
        }
    }

    fn active_path(&self) -> PathBuf {
        self.log_dir.join("host-events.jsonl")
    }

    fn rotate_if_needed(&self) -> io::Result<()> {
        let path = self.active_path();
        if !path.is_file() {
            return Ok(());
        }
        let size = fs::metadata(&path)?.len();
        if size < self.max_bytes {
            return Ok(());
        }
        let oldest = self.log_dir.join(format!("host-events.{}.jsonl", self.max_files - 1));
        if oldest.is_file() {
            let _ = fs::remove_file(&oldest);
        }
        for index in (1..self.max_files - 1).rev() {
            let from = self.log_dir.join(format!("host-events.{index}.jsonl"));
            if from.is_file() {
                let to = self.log_dir.join(format!("host-events.{}.jsonl", index + 1));
                let _ = fs::rename(from, to);
            }
        }
        let first = self.log_dir.join("host-events.1.jsonl");
        let _ = fs::rename(&path, first);
        Ok(())
    }

    fn append(&self, entry: &HostLogEntry) -> io::Result<()> {
        fs::create_dir_all(&self.log_dir)?;
        self.rotate_if_needed()?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.active_path())?;
        serde_json::to_writer(&mut file, entry).map_err(io::Error::other)?;
        file.write_all(b"\n")?;
        Ok(())
    }
}

fn writer() -> &'static Mutex<Option<HostLogWriter>> {
    static WRITER: OnceLock<Mutex<Option<HostLogWriter>>> = OnceLock::new();
    WRITER.get_or_init(|| Mutex::new(None))
}

pub(crate) fn initialize(source_root: &Path) {
    let log_dir = source_root.join(".mei").join("runtime").join("logs");
    if let Ok(mut guard) = writer().lock() {
        *guard = Some(HostLogWriter::new(log_dir));
    }
}

pub(crate) fn host_log_dir(source_root: &Path) -> PathBuf {
    source_root.join(".mei").join("runtime").join("logs")
}

pub struct HostLogLayer;

impl<S> Layer<S> for HostLogLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = *event.metadata().level();
        if !matches!(level, Level::ERROR | Level::WARN) {
            return;
        }
        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);
        let message = visitor.message.unwrap_or_else(|| event.metadata().name().to_string());
        let entry = HostLogEntry {
            recorded_at_ms: crate::http::startup_run::now_ms_for_host_message() as u128,
            level: level.to_string().to_ascii_lowercase(),
            target: event.metadata().target().to_string(),
            message,
            fields: if visitor.fields.is_empty() {
                None
            } else {
                Some(visitor.fields)
            },
            run_id: crate::http::startup_run::current_run_id(),
        };
        if let Ok(guard) = writer().lock() {
            if let Some(writer) = guard.as_ref() {
                if let Err(error) = writer.append(&entry) {
                    eprintln!("host log append failed: {error}");
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct HostLogQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    #[serde(rename = "minLevel")]
    pub min_level: Option<String>,
    pub contains: Option<String>,
    #[serde(rename = "runId")]
    pub run_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct HostLogListResponse {
    pub count: usize,
    #[serde(rename = "logDir")]
    pub log_dir: String,
    pub entries: Vec<HostLogEntry>,
}

fn level_rank(level: &str) -> u8 {
    match level.to_ascii_lowercase().as_str() {
        "error" => 0,
        "warn" => 1,
        "info" => 2,
        "debug" => 3,
        "trace" => 4,
        _ => 5,
    }
}

fn read_jsonl_entries(path: &Path) -> io::Result<Vec<HostLogEntry>> {
    let raw = fs::read_to_string(path)?;
    let mut entries = Vec::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        if let Ok(entry) = serde_json::from_str::<HostLogEntry>(line) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

pub(crate) fn load_host_log_entries(source_root: &Path) -> Vec<HostLogEntry> {
    let log_dir = host_log_dir(source_root);
    let mut paths = Vec::new();
    for index in (1..DEFAULT_MAX_FILES).rev() {
        let path = log_dir.join(format!("host-events.{index}.jsonl"));
        if path.is_file() {
            paths.push(path);
        }
    }
    let active = log_dir.join("host-events.jsonl");
    if active.is_file() {
        paths.push(active);
    }
    let mut entries = Vec::new();
    for path in paths {
        if let Ok(mut chunk) = read_jsonl_entries(path.as_path()) {
            entries.append(&mut chunk);
        }
    }
    entries
}

fn filter_entries(entries: &[HostLogEntry], query: &HostLogQuery) -> Vec<HostLogEntry> {
    let min_rank = query
        .min_level
        .as_deref()
        .map(level_rank)
        .unwrap_or(level_rank("warn"));
    entries
        .iter()
        .rev()
        .filter(|entry| {
            if level_rank(entry.level.as_str()) > min_rank {
                return false;
            }
            if let Some(run_id) = query.run_id.as_deref() {
                if entry.run_id.as_deref() != Some(run_id) {
                    return false;
                }
            }
            if let Some(needle) = query.contains.as_deref() {
                let haystack = format!(
                    "{} {} {:?}",
                    entry.message,
                    entry.target,
                    entry.fields.as_ref()
                );
                if !haystack.contains(needle) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

pub async fn api_host_logs(
    State(state): State<AppState>,
    Query(query): Query<HostLogQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(200).clamp(1, 2000);
    let offset = query.offset.unwrap_or(0);
    let entries = load_host_log_entries(state.source_root.as_path());
    let filtered = filter_entries(&entries, &query);
    let page = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    let response = HostLogListResponse {
        count: page.len(),
        log_dir: host_log_dir(state.source_root.as_path())
            .display()
            .to_string(),
        entries: page,
    };
    (StatusCode::OK, Json(response)).into_response()
}

pub(crate) fn prune_startup_runs(source_root: &Path) {
    let retain = std::env::var("MEI_STARTUP_RUN_RETAIN")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .filter(|value| *value >= 3)
        .unwrap_or(DEFAULT_RETAIN_RUNS);
    let root = source_root
        .join(".mei")
        .join("runtime")
        .join("startup-runs");
    let Ok(entries) = fs::read_dir(root.as_path()) else {
        return;
    };
    let mut runs = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .collect::<Vec<_>>();
    runs.sort_by_key(|entry| entry.file_name());
    if runs.len() <= retain {
        return;
    }
    let remove_count = runs.len().saturating_sub(retain);
    for entry in runs.into_iter().take(remove_count) {
        let _ = fs::remove_dir_all(entry.path());
    }
}
