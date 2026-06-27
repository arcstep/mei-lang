use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde_json::json;

use super::super::args::PrebuildArgs;
use super::super::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};
use crate::agent_runtime;
use crate::prebuild::{
    clean_workspace_prebuild_artifacts, persist_prebuild_report, prebuild_emit_notice,
    prebuild_set_output_quiet, run_prebuild_worker_if_requested, PrebuildPhaseSession, PrebuildPhaseTracker, PrebuildProgressSession,
    run_prebuild, PrebuildMode, PrebuildOptions, PrebuildReport, PrebuildScopeProfile,
    PrebuildWarningSummary,
};

pub fn prebuild_command(args: PrebuildArgs) -> Result<()> {
    if run_prebuild_worker_if_requested(&args)? {
        return Ok(());
    }
    if args.verify && args.clean {
        anyhow::bail!("`prebuild --verify` and `--clean` cannot be used together");
    }
    if args.json && args.json_full {
        anyhow::bail!("`prebuild --json` and `--json-full` cannot be used together");
    }
    let clean_only = args.clean && !args.prebuild;
    let wipe_before_build = args.clean && args.prebuild;
    if clean_only && (args.force_rebuild || args.hot_only) {
        anyhow::bail!("`--clean` without `--prebuild` only clears artifacts; remove --force-rebuild / --hot-only");
    }
    let json_mode = args.json || args.json_full;
    // `--json` 仅影响 stdout 摘要；stderr 仍输出分步进度，避免长时间无输出像卡死。
    prebuild_set_output_quiet(false);
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let raw_source_root =
        resolve_source_root_arg(&package_root, args.workspace.as_deref(), &args.source_root)?;
    let source_root = resolve_cli_source_root(&package_root, &raw_source_root)?;
    let app_filter = args
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if clean_only {
        let clean_report =
            clean_workspace_prebuild_artifacts(source_root.as_path(), app_filter)?;
        if json_mode {
            let payload = json!({
                "schema_version": "mei-cli-v1",
                "command": "prebuild_clean",
                "ok": true,
                "source_root": clean_report.source_root,
                "cleaned_apps": clean_report.cleaned_apps,
                "app_details": clean_report.app_details,
                "workspace_artifacts_removed": clean_report.workspace_artifacts_removed,
                "build_links_reset": clean_report.build_links_reset,
                "clean_wall_ms": clean_report.clean_wall_ms,
            });
            print_json_output(&payload, false)?;
        } else {
            println!(
                "cold-start clean done in {:.2}s | workspace={}",
                clean_report.clean_wall_ms as f64 / 1000.0,
                clean_report.source_root
            );
            if !clean_report.cleaned_apps.is_empty() {
                println!("  apps: {}", clean_report.cleaned_apps.join(", "));
            }
            for detail in &clean_report.app_details {
                println!(
                    "  {} | build={} var={} graph={} compile_cache={}",
                    detail.app_id,
                    detail.removed_build_store,
                    detail.removed_var_store,
                    detail.removed_graph_registry,
                    detail.compile_cache_entries,
                );
            }
            if !clean_report.workspace_artifacts_removed.is_empty() {
                println!(
                    "  workspace: {}",
                    clean_report.workspace_artifacts_removed.join(", ")
                );
            }
            if clean_report.build_links_reset {
                println!("  deploy/state/links.json build pointers reset");
            }
            crate::prebuild::prebuild_emit_success_banner(
                "CLEAN OK",
                &[
                    &format!(
                        "耗时 {:.2}s | apps={}",
                        clean_report.clean_wall_ms as f64 / 1000.0,
                        clean_report.cleaned_apps.len()
                    ),
                    "下一步: `./deploy/prebuild.sh --clean --prebuild` 或 `prebuild`（无 --clean）重建",
                ],
            );
        }
        return Ok(());
    }

    let _progress_session = PrebuildProgressSession::begin();
    let _phase_session = PrebuildPhaseSession::begin(source_root.as_path());
    PrebuildPhaseTracker::global().set_phase(
        source_root.as_path(),
        "author_skill",
        None,
        Some("检查/安装 workspace author skill"),
    );
    let skill_started = std::time::Instant::now();
    let skill_report = mei_lang_toolchain::ensure_workspace_author_skill_package(
        source_root.as_path(),
        package_root.as_path(),
    )?;
    prebuild_emit_notice(format!(
        "✓ author_skill | files={} installed_now={} | {}ms",
        skill_report.file_count,
        skill_report.installed_now,
        skill_started.elapsed().as_millis()
    ));
    if skill_report.installed_now {
        eprintln!(
            "installed workspace-local author skill at {}",
            skill_report.install_dir
        );
    }
    if args.dirty_only {
        std::env::set_var("MEI_PREBUILD_DIRTY_ONLY", "1");
    }
    let report = run_prebuild(
        source_root.as_path(),
        &PrebuildOptions {
            app_filter: app_filter.map(str::to_string),
            mode: if args.verify {
                PrebuildMode::Verify
            } else {
                PrebuildMode::Build
            },
            clean: wipe_before_build,
            force_rebuild: args.force_rebuild,
            scope_profile: if args.hot_only {
                PrebuildScopeProfile::HotOnly
            } else {
                PrebuildScopeProfile::Full
            },
            dirty_only: args.dirty_only,
            block_node: args
                .block_node
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            diagnose_on_fail: !args.no_diagnose_on_fail,
            continue_from: args
                .continue_from
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        },
    )?;
    let full_report_path = persist_prebuild_report(source_root.as_path(), &report).ok();
    if args.json_full {
        print_json_output(&report, true)?;
    } else if args.json {
        let warning_summary = report.aggregate_warning_summary();
        let failed_block_hints =
            crate::block::collect_prebuild_failed_block_hints(source_root.as_path(), &report);
        let payload = json!({
            "schema_version": "mei-cli-v1",
            "command": "prebuild",
            "ok": report.ok,
            "mode": match report.mode {
                PrebuildMode::Build => "build",
                PrebuildMode::Verify => "verify",
            },
            "scope_profile": match report.scope_profile {
                PrebuildScopeProfile::Full => "full",
                PrebuildScopeProfile::HotOnly => "hot_only",
                PrebuildScopeProfile::BlockScoped => "block_scoped",
            },
            "wiped_before_build": wipe_before_build,
            "total_wall_ms": report.total_wall_ms,
            "succeeded_apps": report.succeeded_apps,
            "failed_apps": report.failed_apps,
            "warning_count": warning_summary.total,
            "failedBlockHints": failed_block_hints,
            "full_report_path": full_report_path.as_ref().map(|path| path.display().to_string()),
        });
        print_json_output(&payload, false)?;
    } else {
        print_prebuild_human_summary(&report, full_report_path.as_deref(), wipe_before_build);
        if report.ok && report.failed_apps.is_empty() {
            let scope_profile = match report.scope_profile {
                PrebuildScopeProfile::Full => "full",
                PrebuildScopeProfile::HotOnly => "hot_only",
                PrebuildScopeProfile::BlockScoped => "block_scoped",
            };
            let wipe_line = if wipe_before_build {
                "clean + prebuild"
            } else {
                "prebuild"
            };
            crate::prebuild::prebuild_emit_success_banner(
                "PREBUILD OK",
                &[
                    &format!(
                        "{wipe_line} | {:.1}s | apps={} | profile={scope_profile}",
                        report.total_wall_ms as f64 / 1000.0,
                        report.succeeded_apps.len(),
                    ),
                    "下一步: `./deploy/start.sh` — 应跳过 startup 重复构建（见 STARTUP PREBUILD SKIPPED）",
                ],
            );
        }
        emit_prebuild_diagnostic_hints(source_root.as_path(), &report, false);
    }
    if json_mode {
        emit_prebuild_json_footer(&report, full_report_path.as_deref());
    }
    Ok(())
}

fn print_prebuild_human_summary(
    report: &PrebuildReport,
    full_report_path: Option<&Path>,
    wiped_before_build: bool,
) {
    let mode = match report.mode {
        PrebuildMode::Build => "build",
        PrebuildMode::Verify => "verify",
    };
    let status = if report.ok { "ok" } else { "FAILED" };
    let scope_profile = match report.scope_profile {
        PrebuildScopeProfile::Full => "full",
        PrebuildScopeProfile::HotOnly => "hot_only",
        PrebuildScopeProfile::BlockScoped => "block_scoped",
    };
    let wipe_hint = if wiped_before_build {
        " (after --clean --prebuild wipe)"
    } else {
        ""
    };
    println!(
        "prebuild {status} ({mode}, {scope_profile}){wipe_hint} in {:.1}s",
        report.total_wall_ms as f64 / 1000.0
    );
    if report.clean_wall_ms > 0 {
        println!(
            "  clean phase: {:.2}s",
            report.clean_wall_ms as f64 / 1000.0
        );
    }
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
    use crate::block::{
        block_list_hint, fast_loop_hints, layer_verify_hint, prebuild_warning_hint,
    };

    let workspace_flag = format!("--workspace {}", source_root.display());
    let emit = |line: &str| {
        if to_stderr {
            eprintln!("{line}");
        } else {
            println!("{line}");
        }
    };
    let warning_summary = report.aggregate_warning_summary();
    let app_id = report
        .succeeded_apps
        .first()
        .or(report.failed_apps.first())
        .cloned()
        .or_else(|| report.apps.first().map(|app| app.app_id.clone()));

    emit("  diagnose (fast-loop):");
    if let Some(app_id) = app_id.as_deref() {
        if let Some(warning) = report
            .apps
            .iter()
            .flat_map(|app| app.warnings.iter())
            .next()
        {
            if let Some(hint) = prebuild_warning_hint(workspace_flag.as_str(), app_id, warning) {
                tracing::info!(target: "mei.prebuild.hint", hint = %hint, "prebuild warning hint");
                emit(&format!("    {hint}"));
            }
            if let Some(chain) = warning.error_chain.as_deref().filter(|value| !value.is_empty()) {
                let first_line = chain.lines().next().unwrap_or(chain);
                emit(&format!("    error_chain: {first_line}"));
            }
        }
        emit(&format!(
            "    {}",
            layer_verify_hint(workspace_flag.as_str(), app_id, "mcg")
        ));
        emit(&format!(
            "    {}",
            layer_verify_hint(workspace_flag.as_str(), app_id, "mrg")
        ));
        emit(&format!(
            "    {}",
            block_list_hint(workspace_flag.as_str(), app_id, "failed")
        ));
        if warning_summary.total > 0 {
            emit("  fast-loop:");
            for hint in fast_loop_hints(workspace_flag.as_str(), app_id) {
                emit(&format!("    {hint}"));
            }
        }
    }
    emit("  diagnose (graph):");
    emit(&format!("    mei-toolchain graph doctor {workspace_flag}"));
    emit(&format!(
        "    mei-toolchain diagnostics summary {workspace_flag} --sections gate_sweep,content_store"
    ));
    if let Some(app_id) = app_id.as_deref() {
        emit(&format!(
            "    mei-toolchain scope gate check {workspace_flag} --app {app_id} --scene home"
        ));
        emit(&format!(
            "    mei-toolchain block eval {workspace_flag} --app {app_id} --scope home --target src/scenes/home.mei --owner <owner> --verbose"
        ));
    }
    emit("  verbose prebuild progress: MEI_PREBUILD_VERBOSE=1");
}
