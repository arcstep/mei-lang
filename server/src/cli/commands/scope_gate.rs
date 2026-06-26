use anyhow::Result;

use crate::cli::args::ScopeArgs;
use crate::cli::util::{print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg};
use crate::graph::run_scope_gate_check;

pub fn scope_command(args: ScopeArgs) -> Result<()> {
    match args.command {
        crate::cli::args::ScopeCommand::Gate(gate_args) => match gate_args.command {
            crate::cli::args::ScopeGateCommand::Check(check_args) => scope_gate_check_command(check_args),
        },
    }
}

fn scope_gate_check_command(args: crate::cli::args::ScopeGateCheckArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let raw_source_root =
        resolve_source_root_arg(&package_root, args.workspace.as_deref(), &args.source_root)?;
    let source_root = resolve_cli_source_root(&package_root, &raw_source_root)?;
    let report = run_scope_gate_check(
        source_root.as_path(),
        args.app_id.as_str(),
        args.scene.as_deref(),
        args.target_file.as_deref(),
    );
    if args.json {
        print_json_output(&report, true)?;
    } else {
        println!(
            "scope={}/{} navigation_ready={} assembly_ready={} data_ready={} access_ready={}",
            report.scope.scene_id,
            report.scope.target_file,
            report.navigation_ready,
            report.assembly_ready,
            report.data_ready,
            report.access_ready
        );
        for blocker in &report.blockers {
            println!("  blocker: {blocker}");
        }
    }
    if !report.access_ready {
        std::process::exit(1);
    }
    Ok(())
}
