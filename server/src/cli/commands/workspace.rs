use anyhow::Result;
use mei_lang_toolchain;
use serde_json::json;

use super::super::args::{
    WorkspaceArgs, WorkspaceBuildCommand, WorkspaceCommand, WorkspaceRuntimeCommand,
};
use super::super::util::{print_json_output, resolve_cli_source_root, resolve_package_root};

pub fn workspace_command(args: WorkspaceArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    match args.command {
        WorkspaceCommand::Bootstrap(args) => {
            let source_root = if args.source_root.is_absolute() {
                args.source_root.clone()
            } else {
                std::env::current_dir()?.join(&args.source_root)
            };
            initialize_standalone_workspace(
                source_root.as_path(),
                args.label.as_deref(),
                &package_root,
            )?;
            let runtime_report = mei_lang_toolchain::install_editor_runtime_support_files(
                &source_root,
                &package_root,
                args.force,
            )?;
            let tools = if args.tools.is_empty() {
                vec!["cursor".to_string()]
            } else {
                args.tools.clone()
            };
            let scaffold = mei_lang_toolchain::scaffold_editor_runtime_tooling(
                &source_root,
                &package_root,
                &tools,
                args.force,
            )?;
            let app_root = bootstrap_optional_app(&source_root, args.app_id.as_deref())?;
            let status = mei_lang_toolchain::workspace_runtime_status_for_workspace_root(
                &package_root,
                &source_root,
            );
            let output = json!({
                "schema_version": "mei-cli-v1",
                "command": "workspace.bootstrap",
                "source_root": source_root,
                "materialized": true,
                "runtime_install": runtime_report,
                "scaffold": scaffold,
                "app_root": app_root,
                "status": status,
            });
            print_json_output(&output, args.json)
        }
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
                )?
            };
            if args.source_root.is_some() {
                initialize_standalone_workspace(
                    source_root.as_path(),
                    args.label.as_deref(),
                    &package_root,
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
                "scaffold": scaffold,
            });
            print_json_output(&output, args.json)
        }
        WorkspaceCommand::Materialize(args) => {
            eprintln!(
                "warning: `workspace materialize` is deprecated; stock is ensured automatically during init, runtime install, prebuild, and host startup. Prefer those flows; use `--force` here only for operational overrides."
            );
            let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
            let (report, skipped) = if args.force {
                (
                    Some(mei_lang_toolchain::materialize_workspace_stock(
                        &source_root,
                        &package_root,
                        true,
                    )?),
                    false,
                )
            } else if let Some(report) = mei_lang_toolchain::ensure_workspace_stock_materialized(
                &source_root,
                &package_root,
            )? {
                (Some(report), false)
            } else {
                (None, true)
            };
            let output = json!({
                "schema_version": "mei-cli-v1",
                "command": "workspace.materialize",
                "deprecated": true,
                "skipped": skipped,
                "report": report,
            });
            print_json_output(&output, args.json)
        }
        WorkspaceCommand::Runtime(args) => match args.command {
            WorkspaceRuntimeCommand::Status(args) => {
                let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
                let report = mei_lang_toolchain::workspace_runtime_status_for_workspace_root(
                    &package_root,
                    &source_root,
                );
                let output = json!({
                    "schema_version": "mei-cli-v1",
                    "command": "workspace.runtime.status",
                    "report": report,
                });
                print_json_output(&output, args.json)
            }
            WorkspaceRuntimeCommand::Install(args) => {
                let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
                let report = mei_lang_toolchain::install_editor_runtime_support_files(
                    &source_root,
                    &package_root,
                    args.force,
                )?;
                let status = mei_lang_toolchain::workspace_runtime_status_for_workspace_root(
                    &package_root,
                    &source_root,
                );
                let output = json!({
                    "schema_version": "mei-cli-v1",
                    "command": "workspace.runtime.install",
                    "report": report,
                    "status": status,
                });
                print_json_output(&output, args.json)
            }
            WorkspaceRuntimeCommand::Update(args) => {
                let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
                let report = mei_lang_toolchain::install_editor_runtime_support_files(
                    &source_root,
                    &package_root,
                    true,
                )?;
                let status = mei_lang_toolchain::workspace_runtime_status_for_workspace_root(
                    &package_root,
                    &source_root,
                );
                let output = json!({
                    "schema_version": "mei-cli-v1",
                    "command": "workspace.runtime.update",
                    "report": report,
                    "status": status,
                    "force_requested": args.force,
                    "preserved": ["runtime/hosts/**", "runtime/agent/**"],
                });
                print_json_output(&output, args.json)
            }
        },
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
        WorkspaceCommand::Build(args) => match args.command {
            WorkspaceBuildCommand::Promote(args) => {
                let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
                let build_id = mei_lang_kernel::promote_build(
                    source_root.as_path(),
                    args.build_id.as_deref(),
                )?;
                let links = mei_lang_kernel::read_links_state(source_root.as_path())?;
                let output = json!({
                    "schema_version": "mei-cli-v1",
                    "command": "workspace.build.promote",
                    "build_id": build_id,
                    "links": links,
                });
                print_json_output(&output, args.json)
            }
            WorkspaceBuildCommand::Rollback(args) => {
                let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
                let build_id = mei_lang_kernel::rollback_build(source_root.as_path())?;
                let links = mei_lang_kernel::read_links_state(source_root.as_path())?;
                let output = json!({
                    "schema_version": "mei-cli-v1",
                    "command": "workspace.build.rollback",
                    "build_id": build_id,
                    "links": links,
                });
                print_json_output(&output, args.json)
            }
            WorkspaceBuildCommand::Status(args) => {
                let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
                let links = mei_lang_kernel::read_links_state(source_root.as_path())?;
                let apps = mei_lang_kernel::discover_apps(source_root.as_path())?;
                let build_id = links
                    .build
                    .active
                    .as_deref()
                    .or(links.build.candidate.as_deref());
                let mut app_manifests = serde_json::Map::new();
                if let Some(build_id) = build_id.map(str::trim).filter(|value| !value.is_empty()) {
                    for app in &apps {
                        let app_root =
                            mei_lang_kernel::resolve_app_root(source_root.as_path(), app.id.as_str());
                        let store = mei_lang_kernel::app_build_store_dir(app_root.as_path(), build_id);
                        if let Ok(Some(manifest)) =
                            mei_lang_kernel::read_build_manifest(store.as_path())
                        {
                            app_manifests.insert(app.id.clone(), serde_json::to_value(manifest)?);
                        }
                    }
                }
                let output = json!({
                    "schema_version": "mei-cli-v1",
                    "command": "workspace.build.status",
                    "links": links,
                    "app_manifests": app_manifests,
                });
                print_json_output(&output, args.json)
            }
        },
        WorkspaceCommand::MigrateLegacyAppMei(args) => {
            let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
            let mut migrated_apps = Vec::new();
            if args.migrate_workspace {
                mei_lang_kernel::migrate_legacy_workspace_mei(source_root.as_path())?;
            }
            if let Some(app_id) = args
                .app_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let app_root = mei_lang_kernel::resolve_app_root(source_root.as_path(), app_id);
                mei_lang_kernel::migrate_legacy_app_mei(app_root.as_path())?;
                migrated_apps.push(app_id.to_string());
            } else if args.migrate_workspace || args.all_apps {
                for app in mei_lang_kernel::discover_apps(source_root.as_path())? {
                    let app_root =
                        mei_lang_kernel::resolve_app_root(source_root.as_path(), app.id.as_str());
                    mei_lang_kernel::migrate_legacy_app_mei(app_root.as_path())?;
                    migrated_apps.push(app.id);
                }
            }
            let output = json!({
                "schema_version": "mei-cli-v1",
                "command": "workspace.migrate-legacy-app-mei",
                "migrated_workspace": args.migrate_workspace,
                "migrated_apps": migrated_apps,
            });
            print_json_output(&output, args.json)
        }
    }
}

fn bootstrap_optional_app(
    source_root: &std::path::Path,
    app_id: Option<&str>,
) -> Result<Option<std::path::PathBuf>> {
    let Some(app_id) = app_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if source_root.join(app_id).exists() {
        return Ok(Some(source_root.join(app_id)));
    }
    mei_lang_toolchain::create_app_skeleton(source_root, app_id).map(Some)
}

fn initialize_standalone_workspace(
    source_root: &std::path::Path,
    label: Option<&str>,
    package_root: &std::path::Path,
) -> Result<()> {
    std::fs::create_dir_all(source_root)?;
    let config_path = mei_lang_kernel::workspace_config_path(source_root);
    if !config_path.is_file() {
        let profile_id = source_root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("workspace");
        let config = mei_lang_kernel::WorkspaceConfig {
            schema_version: 2,
            workspace: mei_lang_kernel::WorkspaceProfile {
                id: Some(profile_id.to_string()),
                label: label.map(str::to_string),
                deploy_host: None,
                default_app: None,
            },
            paths: mei_lang_kernel::WorkspacePathsConfig {
                apps: Some(mei_lang_kernel::DEFAULT_APPS_REL.to_string()),
                components: Some(mei_lang_kernel::DEFAULT_STOCK_COMPONENTS_REL.to_string()),
                templates: Some(mei_lang_kernel::DEFAULT_STOCK_TEMPLATES_REL.to_string()),
                authoring: Some(mei_lang_kernel::DEFAULT_STOCK_AUTHORING_REL.to_string()),
                ..mei_lang_kernel::WorkspacePathsConfig::default()
            },
            ..mei_lang_kernel::WorkspaceConfig::default()
        };
        mei_lang_kernel::write_workspace_config(&config_path, &config)?;
    }
    std::fs::create_dir_all(source_root.join(mei_lang_kernel::DEFAULT_APPS_REL))?;
    std::fs::create_dir_all(source_root.join(mei_lang_kernel::WORKSPACE_HOSTS_DIR_REL))?;
    mei_lang_toolchain::ensure_workspace_stock_materialized(source_root, package_root)?;
    Ok(())
}
