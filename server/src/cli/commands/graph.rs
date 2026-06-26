use anyhow::Result;

use crate::cli::args::GraphArgs;
use crate::cli::util::{print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg};
use crate::graph::migrate::{run_graph_migrate, GraphMigrateOptions};
use crate::graph::{run_graph_doctor, run_graph_inspect, run_graph_status};

pub fn graph_command(args: GraphArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    match args.command {
        crate::cli::args::GraphCommand::Migrate(migrate_args) => {
            let source_root = resolve_source_root_arg(
                &package_root,
                migrate_args.workspace.as_deref(),
                &migrate_args.source_root,
            )?;
            let report = run_graph_migrate(GraphMigrateOptions {
                source_root: source_root.clone(),
                app_id: migrate_args.app_id.clone(),
                clean: migrate_args.clean,
            })?;
            if migrate_args.json {
                print_json_output(
                    &serde_json::json!({
                        "ok": true,
                        "sourceRoot": source_root,
                        "apps": report.apps,
                        "removedPaths": report.removed_paths,
                    }),
                    true,
                )?;
            } else {
                println!(
                    "graph migrate complete: apps={} removed={}",
                    report.apps.len(),
                    report.removed_paths.len()
                );
                for path in &report.removed_paths {
                    println!("  removed {path}");
                }
            }
            Ok(())
        }
        crate::cli::args::GraphCommand::Status(status_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                status_args.workspace.as_deref(),
                &status_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let report = run_graph_status(source_root.as_path(), status_args.app_id.as_deref());
            if status_args.json {
                print_json_output(&report, true)?;
            } else {
                for app in &report.apps {
                    println!(
                        "app={} mcg_rev={} mrg_rev={} slots={}/{}/{} nav={} cas_bytes={}",
                        app.app_id,
                        app.mcg.registry_revision,
                        app.mrg.registry_revision,
                        app.mrg.slot_ready,
                        app.mrg.slot_stale,
                        app.mrg.slot_failed,
                        app.mrg.navigation_count,
                        app.content_store.bytes
                    );
                }
            }
            Ok(())
        }
        crate::cli::args::GraphCommand::Inspect(inspect_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                inspect_args.workspace.as_deref(),
                &inspect_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let layer = match inspect_args.layer {
                crate::cli::args::GraphInspectLayer::Mcg => "mcg",
                crate::cli::args::GraphInspectLayer::Mrg => "mrg",
                crate::cli::args::GraphInspectLayer::Cas => "cas",
                crate::cli::args::GraphInspectLayer::All => "all",
            };
            let report = run_graph_inspect(
                source_root.as_path(),
                inspect_args.app_id.as_str(),
                layer,
                inspect_args.hash.as_deref(),
            );
            if inspect_args.json {
                print_json_output(&report, true)?;
            } else if let Some(nodes) = &report.mcg_nodes {
                for node in nodes {
                    println!("MCG {} key={} rev={} state={}", node.kind, node.key, node.revision, node.state);
                }
            } else if let Some(slots) = &report.mrg_slots {
                for slot in slots {
                    println!(
                        "MRG owner={} scope={} state={}",
                        slot.owner, slot.scope_key, slot.state
                    );
                }
            } else if let Some(cas) = &report.cas {
                println!("CAS bytes={} kinds={:?}", cas.bytes, cas.files_by_kind);
            }
            Ok(())
        }
        crate::cli::args::GraphCommand::Doctor(doctor_args) => {
            let raw = resolve_source_root_arg(
                &package_root,
                doctor_args.workspace.as_deref(),
                &doctor_args.source_root,
            )?;
            let source_root = resolve_cli_source_root(&package_root, &raw)?;
            let report = run_graph_doctor(source_root.as_path(), doctor_args.app_id.as_str());
            if doctor_args.json {
                print_json_output(&report, true)?;
            } else {
                println!("graph doctor app={} ok={}", report.app_id, report.ok);
                for alert in &report.alerts {
                    println!("  {}: {}", alert.layer, alert.message);
                }
            }
            if !report.ok {
                std::process::exit(1);
            }
            Ok(())
        }
    }
}
