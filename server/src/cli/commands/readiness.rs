use anyhow::Result;

use super::super::args::ReadinessArgs;
use super::super::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};
use crate::agent_runtime;
use crate::readiness::reachability;

pub fn readiness_command(args: ReadinessArgs) -> Result<()> {
    match args.command {
        super::super::args::ReadinessCommand::Check(check_args) => readiness_check_command(check_args),
    }
}

fn readiness_check_command(args: super::super::args::ReadinessCheckArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let raw_source_root =
        resolve_source_root_arg(&package_root, args.workspace.as_deref(), &args.source_root)?;
    let source_root = resolve_cli_source_root(&package_root, &raw_source_root)?;
    let report = reachability::check_reachability(
        source_root.as_path(),
        args.bundle_snapshot_root.as_deref(),
    );
    if args.json {
        print_json_output(&report, true)?;
    } else {
        println!(
            "access_entry={}/scene/{}/{} shell_ready={} data_ready={} access_ready={}",
            report.access_entry.app_id,
            report.access_entry.scene_id,
            report.access_entry.target_file,
            report.shell_ready,
            report.data_ready,
            report.access_ready
        );
        for blocker in &report.shell_blockers {
            println!("  shell blocker: {blocker}");
        }
        for blocker in &report.data_blockers {
            println!("  data blocker: {blocker}");
        }
    }
    if !report.access_ready {
        std::process::exit(1);
    }
    Ok(())
}
