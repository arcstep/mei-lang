use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::report::{
    BuildDiagnosticsSection, LastBuildSummary, LAST_BUILD_SUMMARY_REL,
};

#[derive(Debug, Clone, Deserialize, Default)]
struct ParsedCompileIndexStats {
    #[serde(default)]
    hits: usize,
    #[serde(default)]
    misses: usize,
    #[serde(default)]
    stale_entries: usize,
    #[serde(default)]
    mrg_eval_skips: usize,
    #[serde(default)]
    dataframe_eval_skips: usize,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ParsedStartupDiagnostics {
    #[serde(default)]
    peak_rss_bytes: u64,
    current_rss_bytes: Option<u64>,
    #[serde(default)]
    compile_index: ParsedCompileIndexStats,
}

#[derive(Debug, Deserialize)]
struct StartupPrebuildAppReport {
    #[serde(rename = "app_id", alias = "appId")]
    app_id: String,
    diagnostics: ParsedStartupDiagnostics,
}

#[derive(Debug, Deserialize)]
struct StartupPrebuildReport {
    #[serde(default)]
    apps: Vec<StartupPrebuildAppReport>,
    #[serde(default)]
    fingerprint_skip: bool,
    #[serde(default)]
    diagnostics: ParsedStartupDiagnostics,
    #[serde(default)]
    succeeded_apps: Vec<String>,
}

pub fn last_build_summary_path(app_root: &Path) -> PathBuf {
    app_root.join(LAST_BUILD_SUMMARY_REL)
}

pub fn persist_last_build_summary(
    app_root: &Path,
    summary: &LastBuildSummary,
) -> anyhow::Result<()> {
    let path = last_build_summary_path(app_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, serde_json::to_string_pretty(summary)?)?;
    fs::rename(&tmp_path, &path)?;
    Ok(())
}

pub fn load_last_build_summary(app_root: &Path) -> Option<LastBuildSummary> {
    let path = last_build_summary_path(app_root);
    let raw = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn collect_build_diagnostics(
    source_root: &Path,
    app_root: &Path,
    app_id: &str,
) -> BuildDiagnosticsSection {
    let compile_index = load_compile_index_meta(app_root);
    let mut section = BuildDiagnosticsSection {
        compile_index_entries: compile_index.as_ref().map(|meta| meta.entries),
        compile_index_generated_at_ms: compile_index.as_ref().map(|meta| meta.generated_at_ms),
        ..Default::default()
    };

    if let Some(summary) = load_last_build_summary(app_root) {
        if summary.app_id == app_id {
            apply_build_from_parsed(
                &mut section,
                "last-build-summary",
                Some(last_build_summary_path(app_root).display().to_string()),
                Some(summary.recorded_at_ms),
                ParsedStartupDiagnostics {
                    peak_rss_bytes: summary.peak_rss_bytes,
                    current_rss_bytes: summary.current_rss_bytes,
                    compile_index: ParsedCompileIndexStats {
                        hits: summary.compile_index_hits,
                        misses: summary.compile_index_misses,
                        stale_entries: summary.compile_index_stale_entries,
                        mrg_eval_skips: summary.mrg_eval_skips,
                        dataframe_eval_skips: summary.dataframe_eval_skips,
                    },
                },
            );
            return section;
        }
    }

    if let Some((path, diagnostics, recorded_at_ms)) =
        load_latest_startup_prebuild_diagnostics(source_root, app_id)
    {
        apply_build_from_parsed(
            &mut section,
            "startup-run",
            Some(path),
            Some(recorded_at_ms),
            diagnostics,
        );
    } else {
        section.source = "none".to_string();
    }

    section
}

struct CompileIndexMeta {
    entries: usize,
    generated_at_ms: u64,
}

fn load_compile_index_meta(app_root: &Path) -> Option<CompileIndexMeta> {
    #[derive(Deserialize)]
    struct PersistedCompileIndex {
        #[serde(rename = "generated_at_ms", alias = "generatedAtMs")]
        generated_at_ms: u64,
        entries: Vec<serde_json::Value>,
    }
    let path = app_root.join(".mei/prebuild/compile-index.json");
    let raw = fs::read_to_string(&path).ok()?;
    let persisted = serde_json::from_str::<PersistedCompileIndex>(&raw).ok()?;
    Some(CompileIndexMeta {
        entries: persisted.entries.len(),
        generated_at_ms: persisted.generated_at_ms,
    })
}

fn apply_build_from_parsed(
    section: &mut BuildDiagnosticsSection,
    source: &str,
    report_path: Option<String>,
    recorded_at_ms: Option<u64>,
    diagnostics: ParsedStartupDiagnostics,
) {
    section.source = source.to_string();
    section.report_path = report_path;
    section.recorded_at_ms = recorded_at_ms;
    section.peak_rss_bytes = Some(diagnostics.peak_rss_bytes);
    section.current_rss_bytes = diagnostics.current_rss_bytes;
    section.compile_index_hits = Some(diagnostics.compile_index.hits);
    section.compile_index_misses = Some(diagnostics.compile_index.misses);
    section.compile_index_stale_entries = Some(diagnostics.compile_index.stale_entries);
    section.mrg_eval_skips = Some(diagnostics.compile_index.mrg_eval_skips);
    section.dataframe_eval_skips = Some(diagnostics.compile_index.dataframe_eval_skips);
}

fn load_latest_startup_prebuild_diagnostics(
    source_root: &Path,
    app_id: &str,
) -> Option<(String, ParsedStartupDiagnostics, u64)> {
    let runs_root = source_root
        .join(".mei")
        .join("runtime")
        .join("startup-runs");
    let mut candidates = Vec::new();
    let entries = fs::read_dir(&runs_root).ok()?;
    for entry in entries.flatten() {
        let run_dir = entry.path();
        if !run_dir.is_dir() {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        for file_name in ["prebuild-full.json", "prebuild-hot.json"] {
            let path = run_dir.join(file_name);
            if path.is_file() {
                candidates.push((modified, path));
            }
        }
    }
    candidates.sort_by(|left, right| right.0.cmp(&left.0));
    for (_, path) in candidates {
        if let Some(result) = parse_startup_prebuild_report(&path, app_id) {
            return Some(result);
        }
    }
    None
}

fn parse_startup_prebuild_report(
    path: &Path,
    app_id: &str,
) -> Option<(String, ParsedStartupDiagnostics, u64)> {
    let raw = fs::read_to_string(path).ok()?;
    let report = serde_json::from_str::<StartupPrebuildReport>(&raw).ok()?;
    if report.fingerprint_skip {
        return None;
    }
    let app_diag = report
        .apps
        .iter()
        .find(|app| app.app_id == app_id)
        .map(|app| app.diagnostics.clone());
    let diagnostics = app_diag.filter(|diag| diag.peak_rss_bytes > 0).or_else(|| {
        if report.succeeded_apps.iter().any(|id| id == app_id)
            && report.diagnostics.peak_rss_bytes > 0
        {
            Some(report.diagnostics.clone())
        } else {
            None
        }
    })?;
    let recorded_at_ms = path
        .metadata()
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    Some((path.display().to_string(), diagnostics, recorded_at_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_app_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mei-diag-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn collect_build_prefers_last_build_summary() {
        let app_root = temp_app_root("build");
        fs::create_dir_all(app_root.join(".mei/prebuild")).expect("mkdir");
        let summary = LastBuildSummary {
            schema_version: LastBuildSummary::SCHEMA.to_string(),
            app_id: "demo".to_string(),
            recorded_at_ms: 1_700_000_000_000,
            peak_rss_bytes: 2_000_000_000,
            current_rss_bytes: Some(500_000_000),
            compile_index_hits: 7,
            compile_index_misses: 3,
            compile_index_stale_entries: 1,
            mrg_eval_skips: 4,
            dataframe_eval_skips: 2,
        };
        persist_last_build_summary(&app_root, &summary).expect("persist");
        let build = collect_build_diagnostics(&app_root, &app_root, "demo");
        assert_eq!(build.source, "last-build-summary");
        assert_eq!(build.peak_rss_bytes, Some(2_000_000_000));
        assert_eq!(build.compile_index_hits, Some(7));
        let _ = fs::remove_dir_all(&app_root);
    }

    #[test]
    fn load_compile_index_meta_counts_entries() {
        let app_root = temp_app_root("compile-index");
        fs::create_dir_all(app_root.join(".mei/prebuild")).expect("mkdir");
        fs::write(
            app_root.join(".mei/prebuild/compile-index.json"),
            r#"{"schema_version":"v8","generated_at_ms":123,"entries":[{},{}]}"#,
        )
        .expect("write");
        let meta = load_compile_index_meta(&app_root).expect("meta");
        assert_eq!(meta.entries, 2);
        assert_eq!(meta.generated_at_ms, 123);
        let _ = fs::remove_dir_all(&app_root);
    }
}
