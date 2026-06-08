use anyhow::Result;
use mei_lang_toolchain;
use serde_json::json;

use super::super::args::{WorkspaceArgs, WorkspaceCommand};
use super::super::util::{print_json_output, resolve_cli_source_root, resolve_package_root};

pub fn workspace_command(args: WorkspaceArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    match args.command {
        WorkspaceCommand::Init(args) => {
            let parent = if args.workspaces_root.is_absolute() {
                args.workspaces_root.clone()
            } else {
                package_root.join(args.workspaces_root)
            };
            let source_root = mei_lang_toolchain::init_workspace_profile(
                &parent,
                args.profile_id.as_str(),
                args.label.as_deref(),
                &package_root,
                args.materialize,
            )?;
            let output = json!({
                "schema_version": "mei-cli-v1",
                "command": "workspace.init",
                "profile_id": args.profile_id,
                "source_root": source_root,
                "materialized": args.materialize,
            });
            print_json_output(&output, args.json)
        }
        WorkspaceCommand::Materialize(args) => {
            let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
            let report = mei_lang_toolchain::materialize_workspace_stock(
                &source_root,
                &package_root,
                args.force,
            )?;
            let output = json!({
                "schema_version": "mei-cli-v1",
                "command": "workspace.materialize",
                "report": report,
            });
            print_json_output(&output, args.json)
        }
        WorkspaceCommand::CreateApp(args) => {
            let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
            let app_root =
                mei_lang_toolchain::create_app_skeleton(&source_root, args.app_id.as_str())?;
            let output = json!({
                "schema_version": "mei-cli-v1",
                "command": "workspace.create-app",
                "app_id": args.app_id,
                "app_root": app_root,
            });
            print_json_output(&output, args.json)
        }
        WorkspaceCommand::Summary(args) => {
            let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
            let summary = mei_lang_toolchain::build_workspace_summary(&source_root)?;
            let output = json!({
                "schema_version": "mei-cli-v1",
                "command": "workspace.summary",
                "summary": summary,
            });
            print_json_output(&output, args.json)
        }
    }
}
