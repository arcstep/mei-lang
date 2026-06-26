use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

use super::super::args::PrebuildArgs;
use super::super::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};
use crate::agent_runtime;
use crate::prebuild::{
    persist_prebuild_report, prebuild_set_output_quiet, run_prebuild, PrebuildMode,
    PrebuildOptions, PrebuildReport, PrebuildScopeProfile, PrebuildWarningSummary,
};

pub fn prebuild_command(args: PrebuildArgs) -> Result<()> {
    if args.verify && args.clean {
        anyhow::bail!("`prebuild --verify` and `--clean` cannot be used together");
    }
    if args.json && args.json_full {
        anyhow::bail!("`prebuild --json` and `--json-full` cannot be used together");
    }
    let json_mode = args.json || args.json_full;
    prebuild_set_output_quiet(json_mode);
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let raw_source_root =
        resolve_source_root_arg(&package_root, args.workspace.as_deref(), &args.source_root)?;
    let source_root = resolve_cli_source_root(&package_root, &raw_source_root)?;
    let skill_report = mei_lang_toolchain::ensure_workspace_author_skill_package(
        source_root.as_path(),
        package_root.as_path(),
    )?;
    if skill_report.installed_now {
        eprintln!(
            "installed workspace-local author skill at {}",
            skill_report.install_dir
        );
    }
    let report = run_prebuild(
        source_root.as_path(),
        &PrebuildOptions {
            app_filter: args
                .app_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            mode: if args.verify {
                PrebuildMode::Verify
            } else {
                PrebuildMode::Build
            },
            clean: args.clean,
            force_rebuild: args.force_rebuild,
            scope_profile: if args.hot_only {
                PrebuildScopeProfile::HotOnly
            } else {
                PrebuildScopeProfile::Full
            },
        },
    )?;
    let full_report_path = persist_prebuild_report(source_root.as_path(), &report).ok();
    if args.json_full {
        print_json_output(&report, true)?;
    } else if args.json {
        print_json_output(
            &report.summary(full_report_path.as_ref().map(|path| path.display().to_string())),
            true,
        )?;
    } else {
        print_prebuild_human_summary(&report, full_report_path.as_deref());
        emit_prebuild_diagnostic_hints(source_root.as_path(), &report, false);
    }
    if json_mode {
        emit_prebuild_json_footer(&report, full_report_path.as_deref());
    }
    Ok(())
}

fn print_prebuild_human_summary(report: &PrebuildReport, full_report_path: Option<&Path>) {
    let mode = match report.mode {
        PrebuildMode::Build => "build",
        PrebuildMode::Verify => "verify",
    };
    let status = if report.ok { "ok" } else { "FAILED" };
    let scope_profile = match report.scope_profile {
        PrebuildScopeProfile::Full => "full",
        PrebuildScopeProfile::HotOnly => "hot_only",
    };
    println!(
        "prebuild {status} ({mode}, {scope_profile}) in {:.1}s",
        report.total_wall_ms as f64 / 1000.0
    );
    if !report.succeeded_apps.is_empty() {
        println!("  succeeded: {}", report.succeeded_apps.join(", "));
    }
    if !report.failed_apps.is_empty() {
        println!("  failed: {}", report.failed_apps.join(", "));
    }
    for app in &report.apps {
        let scopes = app.compile_scopes.len();
        let cache_hits = app
            .compile_scopes
            .iter()
            .filter(|scope| scope.cache_hit)
            .count();
        println!(
            "  [{}] scopes={scopes} cache_hit={cache_hits}/{scopes} compile={:.1}s artifacts={:.1}s warmup={:.1}s",
            app.app_id,
            app.timings.compile_scopes_ms as f64 / 1000.0,
            app.timings.scope_artifacts_ms as f64 / 1000.0,
            app.timings.warmup_requests_ms as f64 / 1000.0,
        );
        println!(
            "    coverage: dataset_import={} metric_response={} metric_dataframe={}",
            app.coverage.dataset_import_artifacts_ready,
            app.coverage.metric_response_artifacts_ready,
            app.coverage.metric_dataframe_artifacts_ready,
        );
        if !app.warnings.is_empty() {
            println!("    warnings: {}", app.warnings.len());
            for warning in app.warnings.iter().take(5) {
                println!("      - {}", warning.display_message());
            }
            if app.warnings.len() > 5 {
                println!("      - ... +{}", app.warnings.len() - 5);
            }
        }
    }
    for error in &report.error_summary {
        println!("  error: {error}");
    }
    if let Some(path) = full_report_path {
        println!("  full report: {}", path.display());
    }
    println!("  manifest: {}", report.manifest_path);
}

fn emit_prebuild_json_footer(report: &PrebuildReport, full_report_path: Option<&Path>) {
    let status = if report.ok { "ok" } else { "FAILED" };
    let warning_summary = report.aggregate_warning_summary();
    let mode = match report.mode {
        PrebuildMode::Build => "build",
        PrebuildMode::Verify => "verify",
    };
    eprintln!(
        "prebuild {status} ({mode}) in {:.1}s | warnings={}",
        report.total_wall_ms as f64 / 1000.0,
        warning_summary.total
    );
    if !report.failed_apps.is_empty() {
        eprintln!("  failed apps: {}", report.failed_apps.join(", "));
    }
    for error in report.error_summary.iter().take(3) {
        eprintln!("  error: {error}");
    }
    if report.error_summary.len() > 3 {
        eprintln!(
            "  error: ... +{} more (see full report)",
            report.error_summary.len() - 3
        );
    }
    emit_warning_summary_lines(&warning_summary, &|line| eprintln!("{line}"));
    if let Some(path) = full_report_path {
        eprintln!("  full report: {}", path.display());
    }
    emit_prebuild_diagnostic_hints(
        Path::new(&report.source_root),
        report,
        true,
    );
}

fn emit_warning_summary_lines(summary: &PrebuildWarningSummary, emit: &dyn Fn(&str)) {
    if summary.total == 0 {
        return;
    }
    if !summary.by_category.is_empty() {
        emit(&format!(
            "  warning categories: {}",
            format_category_counts(&summary.by_category)
        ));
    }
    if !summary.failing_datasets.is_empty() {
        let tail = if summary.total > summary.failing_datasets.len() {
            " (+more)"
        } else {
            ""
        };
        emit(&format!(
            "  failing datasets: {}{tail}",
            summary.failing_datasets.join(", ")
        ));
    }
    for sample in summary.samples.iter().take(3) {
        emit(&format!("  sample: {}", sample.message));
    }
    if summary.truncated_sample_count > 0 {
        emit(&format!(
            "  sample: ... +{} more warnings",
            summary.truncated_sample_count
        ));
    }
}

fn format_category_counts(counts: &BTreeMap<String, usize>) -> String {
    counts
        .iter()
        .map(|(category, count)| format!("{category}={count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn emit_prebuild_diagnostic_hints(source_root: &Path, report: &PrebuildReport, to_stderr: bool) {
    let workspace_flag = format!("--workspace {}", source_root.display());
    let emit = |line: &str| {
        if to_stderr {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    emit("  diagnose:");
    emit(&format!("    mei-toolchain graph doctor {workspace_flag}"));
    emit(&format!(
        "    mei-toolchain diagnostics summary {workspace_flag} --sections gate_sweep,content_store"
    ));
    if let Some(app_id) = report
        .succeeded_apps
        .first()
        .or(report.failed_apps.first())
    {
        emit(&format!(
            "    mei-toolchain scope gate check {workspace_flag} --app {app_id} --scene home"
        ));
    }
    emit("  verbose prebuild progress: MEI_PREBUILD_VERBOSE=1");
}
