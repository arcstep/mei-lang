use anyhow::{Context, Result};

use crate::block::{
    block_list, parse_block_id, parse_material_states, block_compile_hint, layer_verify_hint,
    BlockOrchestrator,
};
use crate::prebuild::CompileScope;
use crate::cli::args::{BlockArgs, BlockCommand};
use crate::cli::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};

pub fn block_command(args: BlockArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    match args.command {
        BlockCommand::Compile(compile_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                compile_args.workspace.as_deref(),
                &compile_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let block_id = parse_block_id(compile_args.node.as_str())?;
            let result = BlockOrchestrator::compile(
                source_root.as_path(),
                compile_args.app_id.as_str(),
                &block_id,
                compile_args.assemble_only,
            )?;
            if compile_args.json {
                print_json_output(&result, true)?;
            } else if result.ok {
                println!(
                    "block compile ok: {} rev={}",
                    block_id.stable_key(),
                    result.output_revision.as_deref().unwrap_or("")
                );
            } else {
                let workspace_flag = compile_args
                    .workspace
                    .as_deref()
                    .map(|value| format!("--workspace {value}"))
                    .unwrap_or_else(|| format!("--source-root {}", compile_args.source_root.display()));
                eprintln!(
                    "block compile failed: {}\n{}",
                    block_id.stable_key(),
                    result.error_chain.as_deref().unwrap_or("")
                );
                eprintln!(
                    "hint: {}",
                    block_compile_hint(
                        workspace_flag.as_str(),
                        compile_args.app_id.as_str(),
                        block_id.key.as_str()
                    )
                );
                eprintln!(
                    "hint: {}",
                    layer_verify_hint(workspace_flag.as_str(), compile_args.app_id.as_str(), "mcg")
                );
                std::process::exit(1);
            }
            Ok(())
        }
        BlockCommand::Verify(verify_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                verify_args.workspace.as_deref(),
                &verify_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let block_id = parse_block_id(verify_args.node.as_str())?;
            let result = BlockOrchestrator::verify(
                source_root.as_path(),
                verify_args.app_id.as_str(),
                &block_id,
            )?;
            if verify_args.json {
                print_json_output(&result, true)?;
            } else if result.ok {
                println!("block verify ok: {}", block_id.stable_key());
            } else {
                eprintln!(
                    "block verify failed: {}\n{}",
                    block_id.stable_key(),
                    result.error_chain.as_deref().unwrap_or("")
                );
                std::process::exit(1);
            }
            Ok(())
        }
        BlockCommand::Eval(eval_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                eval_args.workspace.as_deref(),
                &eval_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            if eval_args.verbose {
                std::env::set_var("MEI_PREBUILD_OUTPUT_VERBOSE", "1");
            }
            let owner = eval_args.owner.trim();
            if owner.is_empty() {
                anyhow::bail!("block eval requires --owner");
            }
            let scope = CompileScope {
                requested_scene_id: eval_args.scope.clone(),
                requested_target_file: eval_args.target.clone(),
            }
            .canonicalized();
            let report = BlockOrchestrator::materialize_owner(
                source_root.as_path(),
                eval_args.app_id.as_str(),
                eval_args.scope.as_deref(),
                eval_args.target.as_deref(),
                owner,
                eval_args.metrics.as_slice(),
                crate::prebuild::PrebuildMode::Build,
            )
            .with_context(|| format!("block eval scope=`{}` owner=`{owner}`", scope.key()))?;
            if eval_args.json {
                print_json_output(&report, true)?;
            } else if report.ok {
                println!(
                    "block eval ok: scope={} owner={} metrics={}",
                    report.scope_key,
                    report.owner_resource_id,
                    report.metric_ids.join(",")
                );
            } else {
                eprintln!(
                    "block eval failed: scope={} owner={}\n{}",
                    report.scope_key,
                    report.owner_resource_id,
                    report.error_chain.as_deref().unwrap_or("")
                );
                std::process::exit(1);
            }
            Ok(())
        }
        BlockCommand::Inspect(inspect_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                inspect_args.workspace.as_deref(),
                &inspect_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let block_id = parse_block_id(inspect_args.node.as_str())?;
            let result = BlockOrchestrator::inspect(
                source_root.as_path(),
                inspect_args.app_id.as_str(),
                &block_id,
            )?;
            if inspect_args.json {
                print_json_output(&result, true)?;
            } else {
                println!("block inspect: {} ok={}", block_id.stable_key(), result.ok);
                if let Some(chain) = result.error_chain.as_deref() {
                    println!("{chain}");
                }
                for (key, value) in &result.details {
                    println!("  {key}={value}");
                }
            }
            Ok(())
        }
        BlockCommand::List(list_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                list_args.workspace.as_deref(),
                &list_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let states = parse_material_states(list_args.state.as_str());
            let report =
                block_list(source_root.as_path(), list_args.app_id.as_str(), states.as_slice())?;
            if list_args.json {
                print_json_output(&report, true)?;
            } else {
                for entry in &report.blocks {
                    println!(
                        "{} state={}{}",
                        entry.block_id,
                        entry.state,
                        entry
                            .last_error
                            .as_deref()
                            .map(|value| format!(" err={value}"))
                            .unwrap_or_default()
                    );
                }
                println!("total={}", report.blocks.len());
            }
            Ok(())
        }
    }
}
