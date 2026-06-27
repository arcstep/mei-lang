use anyhow::Result;

use crate::block::{
    layer_compile, layer_inspect, layer_status, layer_verify, BlockLayer, LayerCompileOptions,
};
use crate::cli::args::{LayerArgs, LayerCommand, LayerTarget};
use crate::cli::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};

pub fn layer_command(args: LayerArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    match args.command {
        LayerCommand::Compile(compile_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                compile_args.workspace.as_deref(),
                &compile_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let layer = match compile_args.layer {
                LayerTarget::L2 => BlockLayer::L2,
                LayerTarget::L3 => BlockLayer::L3,
                LayerTarget::L4 => BlockLayer::L4,
            };
            let results = layer_compile(
                source_root.as_path(),
                compile_args.app_id.as_str(),
                layer,
                LayerCompileOptions {
                    target_file: compile_args.target.clone(),
                    continue_on_error: compile_args.continue_on_error,
                },
            )?;
            if compile_args.json {
                print_json_output(&results, true)?;
            } else {
                let ok = results.iter().filter(|result| result.ok).count();
                println!(
                    "layer compile {} ok={}/{}",
                    layer.slug(),
                    ok,
                    results.len()
                );
            }
            Ok(())
        }
        LayerCommand::Verify(verify_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                verify_args.workspace.as_deref(),
                &verify_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let report = layer_verify(
                source_root.as_path(),
                verify_args.app_id.as_str(),
                verify_args.layer.as_str(),
            )?;
            if verify_args.json {
                print_json_output(&report, true)?;
            } else if report.ok {
                println!("layer verify ok: app={} layer={}", report.app_id, report.layer);
            } else {
                eprintln!(
                    "layer verify failed: app={} layer={} alerts={}",
                    report.app_id,
                    report.layer,
                    report.alerts.len()
                );
                for alert in &report.alerts {
                    eprintln!("  [{}] {} {}", alert.layer, alert.block_id, alert.message);
                }
                std::process::exit(1);
            }
            Ok(())
        }
        LayerCommand::Inspect(inspect_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                inspect_args.workspace.as_deref(),
                &inspect_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let layer = match inspect_args.layer {
                LayerTarget::L2 => BlockLayer::L2,
                LayerTarget::L3 => BlockLayer::L3,
                LayerTarget::L4 => BlockLayer::L4,
            };
            let report = layer_inspect(
                source_root.as_path(),
                inspect_args.app_id.as_str(),
                layer,
                inspect_args.node.as_deref(),
            )?;
            if inspect_args.json {
                print_json_output(&report, true)?;
            } else {
                println!("layer inspect {} (use --json for detail)", layer.slug());
            }
            Ok(())
        }
        LayerCommand::Status(status_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                status_args.workspace.as_deref(),
                &status_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let report = layer_status(source_root.as_path(), status_args.app_id.as_str())?;
            if status_args.json {
                print_json_output(&report, true)?;
            } else {
                println!(
                    "app={} mcg_nodes={} mrg ready/stale/failed={}/{}/{} dirty={}",
                    report.app_id,
                    report.mcg_nodes,
                    report.mrg_slots_ready,
                    report.mrg_slots_stale,
                    report.mrg_slots_failed,
                    report.dirty_slot_count
                );
            }
            Ok(())
        }
    }
}
