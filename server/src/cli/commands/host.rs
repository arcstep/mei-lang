use anyhow::Result;
use mei_lang_kernel::host_runtime_contract_descriptor;
use serde_json::json;

use super::super::args::{HostArgs, HostAuthArgs, HostAuthCommand, HostCommand, HostDescribeArgs};
use super::super::util::print_json_output;
use crate::build_info;

pub fn host_command(args: HostArgs) -> Result<()> {
    match args.command {
        HostCommand::Describe(args) => host_describe_command(args),
        HostCommand::Auth(args) => host_auth_command(args),
    }
}

pub fn host_describe_command(args: HostDescribeArgs) -> Result<()> {
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.describe",
        "host_build": build_info::descriptor(),
        "host_contract": host_runtime_contract_descriptor(),
    });
    print_json_output(&output, args.json)
}

pub fn host_auth_command(args: HostAuthArgs) -> Result<()> {
    let legacy_command = match args.command {
        HostAuthCommand::EnsureKeys(args) => {
            mei_host_auth::cli_args::LegacyAuthCommand::EnsureKeys(
                mei_host_auth::cli_args::LegacyAuthEnsureKeysArgs {
                    source_root: args.source_root,
                    json: args.json,
                },
            )
        }
        HostAuthCommand::BootstrapUsers(args) => {
            mei_host_auth::cli_args::LegacyAuthCommand::BootstrapUsers(
                mei_host_auth::cli_args::LegacyAuthBootstrapUsersArgs {
                    source_root: args.source_root,
                    super_username: args.super_username,
                    super_profile: args.super_profile,
                    admin_username: args.admin_username,
                    admin_profile: args.admin_profile,
                    guest_username: args.guest_username,
                    guest_profile: args.guest_profile,
                    guest_app_allow: args.guest_app_allow,
                    guest_scene_allow: args.guest_scene_allow,
                    default_password_stdin: args.default_password_stdin,
                    json: args.json,
                },
            )
        }
        HostAuthCommand::AddUser(args) => {
            mei_host_auth::cli_args::LegacyAuthCommand::AddUser(
                mei_host_auth::cli_args::LegacyAuthAddUserArgs {
                    source_root: args.source_root,
                    username: args.username,
                    role: args.role,
                    profile: args.profile,
                    app_allow: args.app_allow,
                    scene_allow: args.scene_allow,
                    password_stdin: args.password_stdin,
                    json: args.json,
                },
            )
        }
        HostAuthCommand::DisableUser(args) => {
            mei_host_auth::cli_args::LegacyAuthCommand::DisableUser(
                mei_host_auth::cli_args::LegacyAuthSetUserEnabledArgs {
                    source_root: args.source_root,
                    username: args.username,
                    json: args.json,
                },
            )
        }
        HostAuthCommand::EnableUser(args) => {
            mei_host_auth::cli_args::LegacyAuthCommand::EnableUser(
                mei_host_auth::cli_args::LegacyAuthSetUserEnabledArgs {
                    source_root: args.source_root,
                    username: args.username,
                    json: args.json,
                },
            )
        }
        HostAuthCommand::RotateKeys(args) => {
            mei_host_auth::cli_args::LegacyAuthCommand::RotateKeys(
                mei_host_auth::cli_args::LegacyAuthRotateKeysArgs {
                    source_root: args.source_root,
                    json: args.json,
                },
            )
        }
        HostAuthCommand::HashPassword(args) => {
            mei_host_auth::cli_args::LegacyAuthCommand::HashPassword(
                mei_host_auth::cli_args::AuthHashPasswordArgs { json: args.json },
            )
        }
        HostAuthCommand::Describe(args) => {
            mei_host_auth::cli_args::LegacyAuthCommand::Describe(
                mei_host_auth::cli_args::LegacyAuthDescribeArgs {
                    source_root: args.source_root,
                    json: args.json,
                },
            )
        }
    };
    mei_host_auth::run_legacy_auth_command(mei_host_auth::cli_args::LegacyAuthArgs {
        command: legacy_command,
    })
}
