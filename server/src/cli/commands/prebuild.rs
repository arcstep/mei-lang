use anyhow::Result;

use super::super::args::PrebuildArgs;
use super::super::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};
use crate::agent_runtime;
use crate::prebuild::{
    run_prebuild, PrebuildMode, PrebuildOptions, PrebuildReport, PrebuildScopeProfile,
};

pub fn prebuild_command(args: PrebuildArgs) -> Result<()> {
    if args.verify && args.clean {
        anyhow::bail!("`prebuild --verify` and `--clean` cannot be used together");
    }
    if args.json && args.json_full {
        anyhow::bail!("`prebuild --json` and `--json-full` cannot be used together");
    }
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let raw_source_root =
        resolve_source_root_arg(&package_root, args.workspace.as_deref(), &args.source_root)?;
    let source_root = resolve_cli_source_root(&package_root, &raw_source_root)?;
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
            scope_profile: if args.hot_only {
                PrebuildScopeProfile::HotOnly
            } else {
                PrebuildScopeProfile::Full
            },
        },
    )?;
    if args.json_full {
        print_json_output(&report, true)?;
    } else if args.json {
        print_json_output(&report.summary(), true)?;
    } else {
        print_prebuild_human_summary(&report);
    }
    Ok(())
}

fn print_prebuild_human_summary(report: &PrebuildReport) {
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
    println!("  manifest: {}", report.manifest_path);
    println!("  tip: --json for summary JSON, --json-full > report.json for full report");
}
