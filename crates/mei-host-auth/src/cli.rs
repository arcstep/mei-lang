use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::cli_args::{
    AuthAddUserArgs, AuthArgs, AuthBootstrapUsersArgs, AuthCommand, AuthDescribeArgs,
    AuthEnsureKeysArgs, AuthHashPasswordArgs, AuthRotateKeysArgs, AuthSetUserEnabledArgs,
    LegacyAuthArgs, LegacyAuthCommand,
};
use crate::{
    ensure_workspace_auth_base, generate_temporary_password, hash_password, load_auth_runtime,
    normalize_id, rotate_workspace_key_pair, set_workspace_user_disabled, upsert_workspace_user,
    AuthRole,
};

pub fn print_json_output(value: &Value, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        print_human_json(value);
    }
    Ok(())
}

fn print_human_json(value: &Value) {
    match value.get("command").and_then(Value::as_str) {
        Some("host.auth.bootstrap-users") | Some("auth.bootstrap-users") => {
            if let Some(users) = value.get("users").and_then(Value::as_array) {
                for user in users {
                    let username = user.get("username").and_then(Value::as_str).unwrap_or("");
                    let role = user.get("role").and_then(Value::as_str).unwrap_or("");
                    let password = user
                        .get("temporary_password")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    println!("{username} ({role}): {password}");
                }
            }
        }
        Some("auth.hash-password") | Some("host.auth.hash-password") => {
            if let Some(hash) = value.get("password_hash").and_then(Value::as_str) {
                println!("{hash}");
            }
        }
        _ => {
            if let Ok(text) = serde_json::to_string_pretty(value) {
                println!("{text}");
            }
        }
    }
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

fn canonicalize_workspace(workspace: &Path) -> Result<PathBuf> {
    workspace
        .canonicalize()
        .with_context(|| format!("failed to canonicalize workspace {}", workspace.display()))
}

fn parse_scene_allow_entries(entries: &[String]) -> Result<BTreeMap<String, Vec<String>>> {
    let mut allow = BTreeMap::<String, Vec<String>>::new();
    for entry in entries {
        let trimmed = entry.trim();
        let Some((app_raw, scene_raw)) = trimmed.split_once(':') else {
            anyhow::bail!("invalid scene-allow `{trimmed}`; expected app_id:scene_id");
        };
        let app_id = normalize_id(app_raw);
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

pub fn run_auth_command(args: AuthArgs) -> Result<()> {
    match args.command {
        AuthCommand::EnsureKeys(args) => auth_ensure_keys_command(args),
        AuthCommand::BootstrapUsers(args) => auth_bootstrap_users_command(args),
        AuthCommand::AddUser(args) => auth_add_user_command(args),
        AuthCommand::DisableUser(args) => auth_set_user_enabled_command(args, false),
        AuthCommand::EnableUser(args) => auth_set_user_enabled_command(args, true),
        AuthCommand::RotateKeys(args) => auth_rotate_keys_command(args),
        AuthCommand::HashPassword(args) => auth_hash_password_command(args),
        AuthCommand::Describe(args) => auth_describe_command(args),
    }
}

pub fn run_legacy_auth_command(args: LegacyAuthArgs) -> Result<()> {
    match args.command {
        LegacyAuthCommand::EnsureKeys(args) => {
            auth_ensure_keys_command(AuthEnsureKeysArgs {
                workspace: args.source_root,
                json: args.json,
            })
        }
        LegacyAuthCommand::BootstrapUsers(args) => auth_bootstrap_users_command(AuthBootstrapUsersArgs {
            workspace: args.source_root,
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
        }),
        LegacyAuthCommand::AddUser(args) => auth_add_user_command(AuthAddUserArgs {
            workspace: args.source_root,
            username: args.username,
            role: args.role,
            profile: args.profile,
            app_allow: args.app_allow,
            scene_allow: args.scene_allow,
            password_stdin: args.password_stdin,
            json: args.json,
        }),
        LegacyAuthCommand::DisableUser(args) => auth_set_user_enabled_command(
            AuthSetUserEnabledArgs {
                workspace: args.source_root,
                username: args.username,
                json: args.json,
            },
            false,
        ),
        LegacyAuthCommand::EnableUser(args) => auth_set_user_enabled_command(
            AuthSetUserEnabledArgs {
                workspace: args.source_root,
                username: args.username,
                json: args.json,
            },
            true,
        ),
        LegacyAuthCommand::RotateKeys(args) => auth_rotate_keys_command(AuthRotateKeysArgs {
            workspace: args.source_root,
            json: args.json,
        }),
        LegacyAuthCommand::HashPassword(args) => auth_hash_password_command(args),
        LegacyAuthCommand::Describe(args) => auth_describe_command(AuthDescribeArgs {
            workspace: args.source_root,
            json: args.json,
        }),
    }
}

fn auth_ensure_keys_command(args: AuthEnsureKeysArgs) -> Result<()> {
    let workspace = canonicalize_workspace(&args.workspace)?;
    let bundle = ensure_workspace_auth_base(&workspace)?;
    let runtime = load_auth_runtime(&workspace)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "auth.ensure-keys",
        "workspace": workspace.display().to_string(),
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

fn auth_bootstrap_users_command(args: AuthBootstrapUsersArgs) -> Result<()> {
    let workspace = canonicalize_workspace(&args.workspace)?;
    let _ = ensure_workspace_auth_base(&workspace)?;
    let guest_app_allow = args
        .guest_app_allow
        .iter()
        .map(|value| normalize_id(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let guest_scene_allow = parse_scene_allow_entries(&args.guest_scene_allow)?;

    let (super_password, admin_password, guest_password, password_mode, shared_hash) =
        if args.default_password_stdin {
            let password = read_password_from_stdin()?;
            let hash = hash_password(password.as_str())?;
            (
                password.clone(),
                password.clone(),
                password,
                "default_password_stdin",
                Some(hash),
            )
        } else {
            (
                generate_temporary_password(),
                generate_temporary_password(),
                generate_temporary_password(),
                "random_temporary",
                None,
            )
        };

    let super_hash = match shared_hash.as_deref() {
        Some(hash) => hash.to_string(),
        None => hash_password(super_password.as_str())?,
    };
    upsert_workspace_user(
        &workspace,
        args.super_username.as_str(),
        args.super_profile.as_str(),
        AuthRole::Super,
        super_hash.as_str(),
        &[],
        &[],
        &BTreeMap::new(),
    )?;

    let admin_hash = match shared_hash.as_deref() {
        Some(hash) => hash.to_string(),
        None => hash_password(admin_password.as_str())?,
    };
    upsert_workspace_user(
        &workspace,
        args.admin_username.as_str(),
        args.admin_profile.as_str(),
        AuthRole::Admin,
        admin_hash.as_str(),
        &[],
        &[],
        &BTreeMap::new(),
    )?;

    let guest_hash = match shared_hash.as_deref() {
        Some(hash) => hash.to_string(),
        None => hash_password(guest_password.as_str())?,
    };
    upsert_workspace_user(
        &workspace,
        args.guest_username.as_str(),
        args.guest_profile.as_str(),
        AuthRole::Guest,
        guest_hash.as_str(),
        &guest_app_allow,
        &[],
        &guest_scene_allow,
    )?;

    let runtime = load_auth_runtime(&workspace)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "auth.bootstrap-users",
        "workspace": workspace.display().to_string(),
        "auth_state_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
        "user_count": runtime.user_count(),
        "password_mode": password_mode,
        "warning": if password_mode == "random_temporary" {
            "temporary password is shown once; rotate immediately via login change-password flow"
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

fn auth_add_user_command(args: AuthAddUserArgs) -> Result<()> {
    if !args.password_stdin {
        anyhow::bail!("--password-stdin is required; plaintext password flags are forbidden");
    }
    let workspace = canonicalize_workspace(&args.workspace)?;
    let _ = ensure_workspace_auth_base(&workspace)?;
    let role = AuthRole::from_slug(args.role.as_str())
        .ok_or_else(|| anyhow::anyhow!("invalid role `{}`", args.role))?;
    let password = read_password_from_stdin()?;
    let password_hash = hash_password(password.as_str())?;
    let app_allow = args
        .app_allow
        .iter()
        .map(|value| normalize_id(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let scene_allow = parse_scene_allow_entries(&args.scene_allow)?;
    upsert_workspace_user(
        &workspace,
        args.username.as_str(),
        args.profile.as_str(),
        role,
        password_hash.as_str(),
        &app_allow,
        &[],
        &scene_allow,
    )?;
    let runtime = load_auth_runtime(&workspace)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "auth.add-user",
        "workspace": workspace.display().to_string(),
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

fn auth_set_user_enabled_command(args: AuthSetUserEnabledArgs, enabled: bool) -> Result<()> {
    let workspace = canonicalize_workspace(&args.workspace)?;
    set_workspace_user_disabled(&workspace, args.username.as_str(), !enabled)?;
    let runtime = load_auth_runtime(&workspace)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": if enabled { "auth.enable-user" } else { "auth.disable-user" },
        "workspace": workspace.display().to_string(),
        "auth_state_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
        "username": args.username.trim(),
        "disabled": !enabled,
    });
    print_json_output(&output, args.json)
}

fn auth_hash_password_command(args: AuthHashPasswordArgs) -> Result<()> {
    let password = read_password_from_stdin()?;
    let password_hash = hash_password(password.as_str())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "auth.hash-password",
        "password_hash": password_hash,
    });
    print_json_output(&output, args.json)
}

fn auth_rotate_keys_command(args: AuthRotateKeysArgs) -> Result<()> {
    let workspace = canonicalize_workspace(&args.workspace)?;
    rotate_workspace_key_pair(&workspace)?;
    let runtime = load_auth_runtime(&workspace)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "auth.rotate-keys",
        "workspace": workspace.display().to_string(),
        "auth_state_path": runtime.config_path.display().to_string(),
        "enabled": runtime.enabled,
    });
    print_json_output(&output, args.json)
}

fn auth_describe_command(args: AuthDescribeArgs) -> Result<()> {
    let workspace = canonicalize_workspace(&args.workspace)?;
    let runtime = load_auth_runtime(&workspace)?;
    let bundle = mei_lang_kernel::load_workspace_auth_bundle(&workspace);
    let journal = mei_lang_kernel::AuthJournal::load(&workspace);
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
        "command": "auth.describe",
        "workspace": workspace.display().to_string(),
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
