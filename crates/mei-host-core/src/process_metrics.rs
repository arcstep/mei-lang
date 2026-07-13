//! Lightweight process RSS / CPU samples for warmup and build observability.

use serde_json::{json, Value};
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessSample {
    pub rss_bytes: Option<u64>,
    pub rss_before_bytes: Option<u64>,
    pub cpu_user_ms: Option<u64>,
    pub cpu_system_ms: Option<u64>,
    pub wall_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ProcessPhaseTimer {
    started: Instant,
    before: ProcessSample,
}

impl ProcessPhaseTimer {
    pub fn start() -> Self {
        Self {
            started: Instant::now(),
            before: sample_process(),
        }
    }

    pub fn finish(self) -> ProcessSample {
        let after = sample_process();
        ProcessSample {
            rss_bytes: after.rss_bytes,
            rss_before_bytes: self.before.rss_bytes,
            cpu_user_ms: match (self.before.cpu_user_ms, after.cpu_user_ms) {
                (Some(before), Some(after)) => Some(after.saturating_sub(before)),
                _ => None,
            },
            cpu_system_ms: match (self.before.cpu_system_ms, after.cpu_system_ms) {
                (Some(before), Some(after)) => Some(after.saturating_sub(before)),
                _ => None,
            },
            wall_ms: self.started.elapsed().as_millis() as u64,
        }
    }
}

pub fn sample_process() -> ProcessSample {
    ProcessSample {
        rss_bytes: current_process_rss_bytes(),
        rss_before_bytes: None,
        cpu_user_ms: current_process_cpu_ms().map(|(user, _)| user),
        cpu_system_ms: current_process_cpu_ms().map(|(_, system)| system),
        wall_ms: 0,
    }
}

pub fn current_process_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
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
        None
    }
}

fn current_process_cpu_ms() -> Option<(u64, u64)> {
    #[cfg(target_os = "linux")]
    {
        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        // After comm (which may contain spaces inside parentheses), fields are:
        // utime stime ... (fields 14/15 in man proc, 1-indexed after pid).
        let close = stat.rfind(')')?;
        let rest = stat.get(close + 1..)?;
        let mut parts = rest.split_whitespace();
        let _state = parts.next()?;
        // skip ppid .. cutime (fields after state until utime)
        for _ in 0..11 {
            parts.next()?;
        }
        let utime: u64 = parts.next()?.parse().ok()?;
        let stime: u64 = parts.next()?.parse().ok()?;
        let ticks = ticks_per_second().unwrap_or(100);
        return Some((
            utime.saturating_mul(1000) / ticks,
            stime.saturating_mul(1000) / ticks,
        ));
    }
    #[cfg(target_os = "macos")]
    {
        let pid = std::process::id();
        let output = std::process::Command::new("ps")
            .args(["-o", "utime=,stime=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut parts = text.split_whitespace();
        let user = parse_ps_time_to_ms(parts.next()?)?;
        let system = parse_ps_time_to_ms(parts.next()?)?;
        return Some((user, system));
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
fn ticks_per_second() -> Option<u64> {
    let output = std::process::Command::new("getconf")
        .arg("CLK_TCK")
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

#[cfg(target_os = "macos")]
fn parse_ps_time_to_ms(raw: &str) -> Option<u64> {
    // Formats: SS.mm / MM:SS.mm / HH:MM:SS
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let mut total_secs = 0.0f64;
    if let Some((left, right)) = raw.rsplit_once(':') {
        if let Some((hh, mm)) = left.rsplit_once(':') {
            total_secs += hh.parse::<f64>().ok()? * 3600.0;
            total_secs += mm.parse::<f64>().ok()? * 60.0;
            total_secs += right.parse::<f64>().ok()?;
        } else {
            total_secs += left.parse::<f64>().ok()? * 60.0;
            total_secs += right.parse::<f64>().ok()?;
        }
    } else {
        total_secs = raw.parse::<f64>().ok()?;
    }
    Some((total_secs * 1000.0) as u64)
}

pub fn process_sample_json(sample: &ProcessSample) -> Value {
    json!({
        "rssBytes": sample.rss_bytes,
        "rssBeforeBytes": sample.rss_before_bytes,
        "cpuUserMs": sample.cpu_user_ms,
        "cpuSystemMs": sample.cpu_system_ms,
        "wallMs": sample.wall_ms,
    })
}
