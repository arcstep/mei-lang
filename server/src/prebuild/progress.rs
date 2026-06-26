use super::prelude::*;
use super::*;

pub(crate) const PREBUILD_REPORT_SCHEMA_VERSION: &str = "mei-prebuild-report-v1";
pub(crate) const PREBUILD_MAX_PARALLELISM: usize = 16;
pub(crate) const CANONICAL_PREBUILD_NODE_BUDGET: usize = 90;
pub(crate) const STARTUP_PREBUILD_WALL_MS_BUDGET_MS: u64 = 60_000;

pub(crate) fn prebuild_max_parallelism_cap() -> usize {
    static CAP: OnceLock<usize> = OnceLock::new();
    *CAP.get_or_init(|| {
        std::env::var("MEI_PREBUILD_MAX_PARALLELISM")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(PREBUILD_MAX_PARALLELISM)
    })
}

pub(crate) fn prebuild_disk_diagnostics_enabled() -> bool {
    std::env::var("MEI_PREBUILD_DISK_DIAGNOSTICS")
        .ok()
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

pub(crate) fn prebuild_progress_every_n() -> usize {
    static EVERY_N: OnceLock<usize> = OnceLock::new();
    *EVERY_N.get_or_init(|| {
        std::env::var("MEI_PREBUILD_PROGRESS_EVERY_N")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(5)
    })
}

pub(crate) fn prebuild_progress_heartbeat_secs() -> u64 {
    static HEARTBEAT_SECS: OnceLock<u64> = OnceLock::new();
    *HEARTBEAT_SECS.get_or_init(|| {
        std::env::var("MEI_PREBUILD_PROGRESS_HEARTBEAT_SECS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .filter(|value| *value >= 5)
            .unwrap_or(30)
    })
}

pub(crate) fn format_eta_secs(secs: f64) -> String {
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

pub(crate) fn emit_compile_batch_progress(
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

pub(crate) fn compile_scope_key_from_parts(
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

pub(crate) fn format_bytes(bytes: u64) -> String {
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

pub(crate) fn current_process_rss_bytes() -> Option<u64> {
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

pub(crate) fn sample_peak_rss_bytes(peak: &AtomicUsize) {
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

pub(crate) struct DirSizeSummary {
    pub(crate) files: usize,
    pub(crate) bytes: u64,
}

pub(crate) fn dir_size_summary(root: &Path) -> DirSizeSummary {
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

pub(crate) fn prebuild_progress_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn prebuild_progress_origin() -> &'static Mutex<Option<Instant>> {
    static ORIGIN: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    ORIGIN.get_or_init(|| Mutex::new(None))
}

pub(crate) struct PrebuildProgressSession;

impl PrebuildProgressSession {
    pub(crate) fn begin() -> Self {
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

pub(crate) fn ansi_wrap(text: &str, code: &str) -> String {
    if supports_ansi_stderr() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub(crate) fn prebuild_emit_progress(message: impl AsRef<str>) {
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

pub(crate) fn format_scope_file(scene: &str, requested_target: &str, active_target: Option<&str>) -> String {
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

pub(crate) fn short_metric_id(metric_id: &str) -> &str {
    metric_id.rsplit("::").next().unwrap_or(metric_id)
}

pub(crate) fn short_dataset_id(dataset_id: &str) -> String {
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

pub(crate) fn emit_slow_compile_report(_app_id: &str, reports: &[PrebuildScopeReport]) {
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
