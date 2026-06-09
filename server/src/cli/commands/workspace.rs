use anyhow::Result;
use mei_lang_toolchain;
use serde_json::json;

use super::super::args::{WorkspaceArgs, WorkspaceCommand};
use super::super::util::{print_json_output, resolve_cli_source_root, resolve_package_root};

pub fn workspace_command(args: WorkspaceArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    match args.command {
        WorkspaceCommand::Init(args) => {
            let source_root = if let Some(source_root) = args.source_root.clone() {
                if source_root.is_absolute() {
                    source_root
                } else {
                    std::env::current_dir()?.join(source_root)
                }
            } else {
                let profile_id = args.profile_id.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("workspace init without --source-root requires <profile_id>")
                })?;
                let parent = if args.workspaces_root.is_absolute() {
                    args.workspaces_root.clone()
                } else {
                    package_root.join(args.workspaces_root)
                };
                mei_lang_toolchain::init_workspace_profile(
                    &parent,
                    profile_id,
                    args.label.as_deref(),
                    &package_root,
                    args.materialize,
                )?
            };
            if args.source_root.is_some() {
                initialize_standalone_workspace(
                    source_root.as_path(),
                    args.label.as_deref(),
                    &package_root,
                    args.materialize,
                )?;
            }
            let mut scaffold = None;
            if !args.tools.is_empty() {
                scaffold = Some(mei_lang_toolchain::scaffold_editor_runtime_tooling(
                    &source_root,
                    &package_root,
                    &args.tools,
                    false,
                )?);
            }
            let output = json!({
                "schema_version": "mei-cli-v1",
                "command": "workspace.init",
                "profile_id": args.profile_id,
                "source_root": source_root,
                "standalone": args.standalone || args.source_root.is_some(),
                "materialized": args.materialize,
                "scaffold": scaffold,
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
            let scaffold = if args.scaffold || !args.tools.is_empty() {
                Some(mei_lang_toolchain::scaffold_editor_runtime_tooling(
                    &source_root,
                    &package_root,
                    &args.tools,
                    false,
                )?)
            } else {
                None
            };
            let output = json!({
                "schema_version": "mei-cli-v1",
                "command": "workspace.create-app",
                "app_id": args.app_id,
                "app_root": app_root,
                "scaffold": scaffold,
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

fn initialize_standalone_workspace(
    source_root: &std::path::Path,
    label: Option<&str>,
    package_root: &std::path::Path,
    materialize: bool,
) -> Result<()> {
    std::fs::create_dir_all(source_root)?;
    let config_path = mei_lang_kernel::workspace_config_path(source_root);
    if !config_path.is_file() {
        let profile_id = source_root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("workspace");
        let config = mei_lang_kernel::WorkspaceConfig {
            schema_version: 1,
            workspace: mei_lang_kernel::WorkspaceProfile {
                id: Some(profile_id.to_string()),
                label: label.map(str::to_string),
                deploy_host: None,
            },
            paths: mei_lang_kernel::WorkspacePathsConfig {
                components: Some(mei_lang_kernel::DEFAULT_STOCK_COMPONENTS_REL.to_string()),
                templates: Some(mei_lang_kernel::DEFAULT_STOCK_TEMPLATES_REL.to_string()),
            },
            ..mei_lang_kernel::WorkspaceConfig::default()
        };
        mei_lang_kernel::write_workspace_config(&config_path, &config)?;
    }
    std::fs::create_dir_all(source_root.join(".mei"))?;
    if materialize {
        mei_lang_toolchain::materialize_workspace_stock(source_root, package_root, false)?;
    }
    Ok(())
}
