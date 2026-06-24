use anyhow::Result;

use super::super::args::DiagnosticsArgs;
use super::super::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};
use crate::agent_runtime;
use crate::diagnostics::collect_materialization_diagnostics;

pub fn diagnostics_command(args: DiagnosticsArgs) -> Result<()> {
    match args.command {
        super::super::args::DiagnosticsCommand::Summary(summary) => {
            diagnostics_summary_command(summary)
        }
    }
}

fn diagnostics_summary_command(
    args: super::super::args::DiagnosticsSummaryArgs,
) -> Result<()> {
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let raw_source_root =
        resolve_source_root_arg(&package_root, args.workspace.as_deref(), &args.source_root)?;
    let source_root = resolve_cli_source_root(&package_root, &raw_source_root)?;
    let report = collect_materialization_diagnostics(
        source_root.as_path(),
        args.app_id.as_str(),
        args.sections.as_slice(),
    );
    if args.json {
        print_json_output(&report, true)?;
    } else {
        println!("app={} alerts={}", report.app_id, report.alerts.len());
        println!(
            "disk: compiled_app files={} bytes={} scene_payload={} eval={}",
            report.disk.compiled_app_file_count,
            report.disk.compiled_app_bytes,
            report.disk.scene_payload_file_count,
            report.disk.eval_artifact_file_count,
        );
        println!(
            "mcg: nodes={} scene_payload={} bundles={} app_skeleton={}",
            report.mcg.node_count,
            report.mcg.scene_payload_nodes,
            report.mcg.metric_def_bundle_nodes,
            report.mcg.app_skeleton_present,
        );
        println!(
            "mrg: slots={} ready={} stale={} failed={} stale_ratio={:.1}%",
            report.mrg.slot_count,
            report.mrg.ready_slots,
            report.mrg.stale_slots,
            report.mrg.failed_slots,
            report.mrg.stale_ratio * 100.0,
        );
        for alert in &report.alerts {
            println!("ALERT: {alert}");
        }
    }
    Ok(())
}
