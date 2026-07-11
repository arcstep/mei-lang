use anyhow::Result;

use super::super::args::DiagnosticsArgs;
use super::super::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};
use crate::agent_runtime;
use crate::diagnostics::{
    collect_materialization_diagnostics, format_age_ms, format_bytes_human,
    MaterializationDiagnosticsReport,
};
use crate::http::startup_run::now_ms_for_host_message;

pub fn diagnostics_command(args: DiagnosticsArgs) -> Result<()> {
    match args.command {
        super::super::args::DiagnosticsCommand::Summary(summary) => {
            diagnostics_summary_command(summary)
        }
    }
}

fn diagnostics_summary_command(args: super::super::args::DiagnosticsSummaryArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let raw_source_root =
        resolve_source_root_arg(&package_root, args.workspace.as_deref(), &args.source_root)?;
    let source_root = resolve_cli_source_root(&package_root, &raw_source_root)?;
    let report = collect_materialization_diagnostics(
        source_root.as_path(),
        args.app_id.as_str(),
        args.sections.as_slice(),
        None,
        None,
    );
    if args.json {
        print_json_output(&report, true)?;
    } else {
        print_human_summary(&report);
    }
    Ok(())
}

fn print_human_summary(report: &MaterializationDiagnosticsReport) {
    let sections = report
        .sections
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let show = |name: &str| sections.iter().any(|section| *section == name);

    println!("app={} alerts={}", report.app_id, report.alerts.len());

    if show("disk") {
        println!();
        println!("[disk]");
        println!(
            "  .mei total          {}",
            format_bytes_human(report.disk.app_root_bytes)
        );
        println!(
            "  compiled_app        {}  ({} files)",
            format_bytes_human(report.disk.compiled_app_bytes),
            report.disk.compiled_app_file_count
        );
        println!(
            "  graph               {}",
            format_bytes_human(report.disk.graph_bytes)
        );
        println!(
            "  eval-artifacts      {} ({} files)",
            format_bytes_human(report.disk.eval_artifact_bytes),
            report.disk.eval_artifact_file_count
        );
        println!(
            "  data-snapshots      {}",
            format_bytes_human(report.disk.data_snapshots_bytes)
        );
        println!(
            "  prebuild            {}",
            format_bytes_human(report.disk.prebuild_bytes)
        );
        println!(
            "  scene_payload       {} files / {}",
            report.disk.scene_payload_file_count,
            format_bytes_human(report.disk.scene_payload_bytes)
        );
    }

    if show("eval") {
        println!();
        println!("[eval]");
        println!(
            "  metric-response     {} files / {}",
            report.eval.metric_response_files,
            format_bytes_human(report.eval.metric_response_bytes)
        );
        println!(
            "  metric-dataframe    {} files / {}",
            report.eval.metric_dataframe_files,
            format_bytes_human(report.eval.metric_dataframe_bytes)
        );
        println!(
            "  eval total          {} files / {}",
            report.eval.eval_total_files,
            format_bytes_human(report.eval.eval_total_bytes)
        );
    }

    if show("mcg") {
        println!();
        println!("[mcg]");
        println!(
            "  nodes={} scene_payload={} bundles={} app_skeleton={}",
            report.mcg.node_count,
            report.mcg.scene_payload_nodes,
            report.mcg.metric_def_bundle_nodes,
            report.mcg.app_skeleton_present
        );
    }

    if show("mrg") {
        println!();
        println!("[mrg]");
        println!(
            "  slots={} ready={} stale={} failed={} stale_ratio={:.1}%",
            report.mrg.slot_count,
            report.mrg.ready_slots,
            report.mrg.stale_slots,
            report.mrg.failed_slots,
            report.mrg.stale_ratio * 100.0
        );
    }

    if show("cache") {
        println!();
        println!("[cache]");
        println!(
            "  access_slim={} canonical_persist={} graph_registry_dedup={}",
            report.cache.access_slim_artifacts,
            report.cache.canonical_artifact_persist,
            report.cache.graph_registry_dedup
        );
    }

    if show("build") {
        println!();
        print_build_section(report);
    }

    if !report.alerts.is_empty() {
        println!();
        for alert in &report.alerts {
            println!("ALERT: {alert}");
        }
    }
}

fn print_build_section(report: &MaterializationDiagnosticsReport) {
    let build = &report.build;
    let age = build
        .recorded_at_ms
        .map(|recorded_at| format_age_ms(recorded_at, now_ms_for_host_message()))
        .unwrap_or_else(|| "unknown".to_string());
    println!("[build]  source={}  age={}", build.source, age);
    if let Some(peak) = build.peak_rss_bytes {
        println!("  peak_rss={}", format_bytes_human(peak));
    } else {
        println!("  peak_rss=(none — run prebuild or start host with full warmup)");
    }
    if let (Some(hits), Some(misses)) = (build.compile_index_hits, build.compile_index_misses) {
        let stale = build
            .compile_index_stale_entries
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!("  compile_index  hits={hits}  misses={misses}  stale={stale}");
    }
    if let (Some(response), Some(dataframe)) = (build.mrg_eval_skips, build.dataframe_eval_skips) {
        println!("  mrg_eval_skips={response}  dataframe_eval_skips={dataframe}");
    }
    if let Some(entries) = build.compile_index_entries {
        let generated = build
            .compile_index_generated_at_ms
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string());
        println!("  compile-index.json  entries={entries}  generated_at_ms={generated}");
    }
}
