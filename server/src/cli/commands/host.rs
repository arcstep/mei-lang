use std::collections::BTreeMap;

use anyhow::{Context, Result};
use mei_lang_kernel::host_runtime_contract_descriptor;
use serde_json::json;

use super::super::args::{
    HostArgs, HostAuthAddUserArgs, HostAuthArgs, HostAuthBootstrapUsersArgs, HostAuthCommand,
    HostAuthDescribeArgs, HostAuthEnsureKeysArgs, HostAuthHashPasswordArgs, HostAuthRotateKeysArgs,
    HostAuthSetUserEnabledArgs, HostCommand, HostDescribeArgs,
};
use super::super::util::{print_json_output, resolve_cli_source_root, resolve_package_root};
use crate::{auth, build_info};

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
    match args.command {
        HostAuthCommand::EnsureKeys(args) => host_auth_ensure_keys_command(args),
        HostAuthCommand::BootstrapUsers(args) => host_auth_bootstrap_users_command(args),
        HostAuthCommand::AddUser(args) => host_auth_add_user_command(args),
        HostAuthCommand::DisableUser(args) => host_auth_set_user_enabled_command(args, false),
        HostAuthCommand::EnableUser(args) => host_auth_set_user_enabled_command(args, true),
        HostAuthCommand::RotateKeys(args) => host_auth_rotate_keys_command(args),
        HostAuthCommand::HashPassword(args) => host_auth_hash_password_command(args),
        HostAuthCommand::Describe(args) => host_auth_describe_command(args),
    }
}

pub fn host_auth_ensure_keys_command(args: HostAuthEnsureKeysArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    let bundle = auth::ensure_workspace_auth_base(&source_root)?;
    let runtime = auth::load_auth_runtime(&source_root)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.ensure-keys",
        "source_root": source_root.display().to_string(),
        "auth_state_path": bundle.config_path.display().to_string(),
        "workspace_config_path": bundle.workspace_config_path.display().to_string(),
        "loaded_from": bundle.loaded_from,
        "loaded_from_path": bundle.loaded_from_path.map(|path| path.display().to_string()),
        "enabled": runtime.enabled,
        "user_count": runtime.user_count(),
        "cookie_name": runtime.cookie_name,
        "jwt_ttl_seconds": runtime.jwt_ttl_seconds,
        "public_key_pem_present": !runtime.public_key_pem.trim().is_empty(),
        "private_key_pem_present": !runtime.private_key_pem.trim().is_empty(),
    });
    print_json_output(&output, args.json)
}

pub fn read_password_from_stdin() -> Result<String> {
    use std::io::Read;
    let mut password = String::new();
    std::io::stdin()
        .read_to_string(&mut password)
        .context("failed to read password from stdin")?;
    let password = password.trim().to_string();
    if password.is_empty() {
        anyhow::bail!("password must not be empty");
    }
    Ok(password)
}

fn parse_scene_allow_entries(entries: &[String]) -> Result<BTreeMap<String, Vec<String>>> {
    let mut allow = BTreeMap::<String, Vec<String>>::new();
    for entry in entries {
        let trimmed = entry.trim();
        let Some((app_raw, scene_raw)) = trimmed.split_once(':') else {
            anyhow::bail!("invalid scene-allow `{trimmed}`; expected app_id:scene_id");
        };
        let app_id = auth::normalize_id(app_raw);
        let scene_id = scene_raw.trim().to_string();
        if app_id.is_empty() || scene_id.is_empty() {
            anyhow::bail!("invalid scene-allow `{trimmed}`; app and scene are required");
        }
        allow.entry(app_id).or_default().push(scene_id);
    }
    for scenes in allow.values_mut() {
        scenes.sort();
        scenes.dedup();
    }
    Ok(allow)
}

pub fn host_auth_bootstrap_users_command(args: HostAuthBootstrapUsersArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    let _ = auth::ensure_workspace_auth_base(&source_root)?;
    let guest_app_allow = args
        .guest_app_allow
        .iter()
        .map(|value| auth::normalize_id(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let guest_scene_allow = parse_scene_allow_entries(&args.guest_scene_allow)?;

    let (super_password, admin_password, guest_password, password_mode, shared_hash) =
        if args.default_password_stdin {
            let password = read_password_from_stdin()?;
            let hash = auth::hash_password(password.as_str())?;
            (
                password.clone(),
                password.clone(),
                password,
                "default_password_stdin",
                Some(hash),
            )
        } else {
            (
                auth::generate_temporary_password(),
                auth::generate_temporary_password(),
                auth::generate_temporary_password(),
                "random_temporary",
                None,
            )
        };

    let super_hash = match shared_hash.as_deref() {
        Some(hash) => hash.to_string(),
        None => auth::hash_password(super_password.as_str())?,
    };
    auth::upsert_workspace_user(
        &source_root,
        args.super_username.as_str(),
        args.super_profile.as_str(),
        auth::AuthRole::Super,
        super_hash.as_str(),
        &[],
        &[],
        &BTreeMap::new(),
    )?;

    let admin_hash = match shared_hash.as_deref() {
        Some(hash) => hash.to_string(),
        None => auth::hash_password(admin_password.as_str())?,
    };
    auth::upsert_workspace_user(
        &source_root,
        args.admin_username.as_str(),
        args.admin_profile.as_str(),
        auth::AuthRole::Admin,
        admin_hash.as_str(),
        &[],
        &[],
        &BTreeMap::new(),
    )?;

    let guest_hash = match shared_hash.as_deref() {
        Some(hash) => hash.to_string(),
        None => auth::hash_password(guest_password.as_str())?,
    };
    auth::upsert_workspace_user(
        &source_root,
        args.guest_username.as_str(),
        args.guest_profile.as_str(),
        auth::AuthRole::Guest,
        guest_hash.as_str(),
        &guest_app_allow,
        &[],
        &guest_scene_allow,
    )?;

    let runtime = auth::load_auth_runtime(&source_root)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.bootstrap-users",
        "source_root": source_root.display().to_string(),
        "auth_state_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
        "user_count": runtime.user_count(),
        "password_mode": password_mode,
        "warning": if password_mode == "random_temporary" {
            "temporary_password is shown once; rotate immediately via login change-password flow"
        } else {
            "default_password_stdin is for local debugging only; do not use in production"
        },
        "users": [
            {
                "username": args.super_username.trim(),
                "role": "super",
                "profile": args.super_profile.trim(),
                "temporary_password": super_password
            },
            {
                "username": args.admin_username.trim(),
                "role": "admin",
                "profile": args.admin_profile.trim(),
                "temporary_password": admin_password
            },
            {
                "username": args.guest_username.trim(),
                "role": "guest",
                "profile": args.guest_profile.trim(),
                "temporary_password": guest_password,
                "app_allowlist": guest_app_allow,
                "scene_allowlist": guest_scene_allow
            }
        ]
    });
    print_json_output(&output, args.json)
}

pub fn host_auth_add_user_command(args: HostAuthAddUserArgs) -> Result<()> {
    if !args.password_stdin {
        anyhow::bail!("--password-stdin is required; plaintext password flags are forbidden");
    }
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    let _ = auth::ensure_workspace_auth_base(&source_root)?;
    let role = auth::AuthRole::from_slug(args.role.as_str())
        .ok_or_else(|| anyhow::anyhow!("invalid role `{}`", args.role))?;
    let password = read_password_from_stdin()?;
    let password_hash = auth::hash_password(password.as_str())?;
    let app_allow = args
        .app_allow
        .iter()
        .map(|value| auth::normalize_id(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let scene_allow = parse_scene_allow_entries(&args.scene_allow)?;
    auth::upsert_workspace_user(
        &source_root,
        args.username.as_str(),
        args.profile.as_str(),
        role,
        password_hash.as_str(),
        &app_allow,
        &[],
        &scene_allow,
    )?;
    let runtime = auth::load_auth_runtime(&source_root)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.add-user",
        "source_root": source_root.display().to_string(),
        "auth_state_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
        "username": args.username.trim(),
        "role": role.as_str(),
        "app_allowlist": app_allow,
        "scene_allowlist": scene_allow,
        "password_hash_written": true,
    });
    print_json_output(&output, args.json)
}

pub fn host_auth_set_user_enabled_command(
    args: HostAuthSetUserEnabledArgs,
    enabled: bool,
) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    auth::set_workspace_user_disabled(&source_root, args.username.as_str(), !enabled)?;
    let runtime = auth::load_auth_runtime(&source_root)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": if enabled { "host.auth.enable-user" } else { "host.auth.disable-user" },
        "source_root": source_root.display().to_string(),
        "auth_state_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
        "username": args.username.trim(),
        "disabled": !enabled,
    });
    print_json_output(&output, args.json)
}

pub fn host_auth_hash_password_command(args: HostAuthHashPasswordArgs) -> Result<()> {
    let password = read_password_from_stdin()?;
    let password_hash = auth::hash_password(password.as_str())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.hash-password",
        "password_hash": password_hash,
    });
    print_json_output(&output, args.json)
}

pub fn host_auth_rotate_keys_command(args: HostAuthRotateKeysArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    auth::rotate_workspace_key_pair(&source_root)?;
    let runtime = auth::load_auth_runtime(&source_root)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.rotate-keys",
        "source_root": source_root.display().to_string(),
        "auth_state_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
    });
    print_json_output(&output, args.json)
}

pub fn host_auth_describe_command(args: HostAuthDescribeArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.source_root)?;
    let runtime = auth::load_auth_runtime(&source_root)?;
    let bundle = mei_lang_kernel::load_workspace_auth_bundle(&source_root);
    let journal = mei_lang_kernel::AuthJournal::load(&source_root);
    let users = bundle
        .auth
        .users
        .iter()
        .map(|user| {
            json!({
                "username": user.username,
                "profile": user.profile,
                "roles": user.roles,
                "disabled": user.disabled,
                "app_allowlist": user.app_allowlist,
                "app_denylist": user.app_denylist,
                "scene_allowlist": user.scene_allowlist,
                "password_hash_present": !user.password_hash.trim().is_empty(),
            })
        })
        .collect::<Vec<_>>();
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "host.auth.describe",
        "source_root": source_root.display().to_string(),
        "auth_state_path": bundle.config_path.display().to_string(),
        "workspace_config_path": bundle.workspace_config_path.display().to_string(),
        "loaded_from": bundle.loaded_from,
        "loaded_from_path": bundle.loaded_from_path.map(|path| path.display().to_string()),
        "enabled": runtime.enabled,
        "user_count": runtime.user_count(),
        "journal_revision": journal.revision,
        "users": users,
        "cookie_name": runtime.cookie_name,
        "jwt_ttl_seconds": runtime.jwt_ttl_seconds,
        "public_key_pem_present": !runtime.public_key_pem.trim().is_empty(),
        "private_key_pem_present": !runtime.private_key_pem.trim().is_empty(),
    });
    print_json_output(&output, args.json)
}
