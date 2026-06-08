use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

use anyhow::{Context, Result};
use argon2::password_hash::PasswordHash;
use mei_lang_kernel::{
    append_auth_journal_entry, load_workspace_auth_bundle, write_workspace_auth_bundle,
    AuthUserConfig, WorkspaceAuthBundle, WorkspaceAuthConfig,
};
use serde_json::json;

use super::crypto::{generate_key_pair_pem, random_jwt_secret};
use super::runtime::{normalize_id, DEFAULT_JWT_COOKIE_NAME, DEFAULT_JWT_TTL_SECONDS};
use super::types::AuthRole;

static WORKSPACE_AUTH_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

fn canonicalize_source_root(source_root: &Path) -> Result<PathBuf> {
    source_root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize source root {}",
            source_root.display()
        )
    })
}

fn workspace_auth_lock(source_root: &Path) -> Result<Arc<Mutex<()>>> {
    let locks = WORKSPACE_AUTH_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = locks
        .lock()
        .map_err(|_| anyhow::anyhow!("workspace auth lock registry is poisoned"))?;
    Ok(guard
        .entry(source_root.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone())
}

fn normalize_app_id_list(values: &[String]) -> Vec<String> {
    let mut normalized = values
        .iter()
        .map(|value| normalize_id(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalize_scene_allowlist(
    values: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    values
        .iter()
        .filter_map(|(app_id, scenes)| {
            let app = normalize_id(app_id);
            if app.is_empty() {
                return None;
            }
            let mut normalized_scenes = scenes
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            normalized_scenes.sort();
            normalized_scenes.dedup();
            if normalized_scenes.is_empty() {
                None
            } else {
                Some((app, normalized_scenes))
            }
        })
        .collect()
}

fn validate_password_hash_format(password_hash: &str) -> Result<()> {
    if password_hash.trim().is_empty() {
        anyhow::bail!("password hash is required");
    }
    PasswordHash::new(password_hash)
        .map_err(|error| anyhow::anyhow!("invalid password hash format: {error}"))?;
    Ok(())
}
pub fn apply_workspace_auth_mutation<T>(
    source_root: &Path,
    action: &str,
    actor: &str,
    summary: &str,
    patch: serde_json::Value,
    mutate: impl FnOnce(&mut WorkspaceAuthConfig) -> Result<(T, bool)>,
) -> Result<(T, WorkspaceAuthBundle)> {
    let source_root = canonicalize_source_root(source_root)?;
    let lock = workspace_auth_lock(&source_root)?;
    let _guard = lock
        .lock()
        .map_err(|_| anyhow::anyhow!("workspace auth lock is poisoned"))?;
    let mut bundle = load_workspace_auth_bundle(&source_root);
    let (result, changed) = mutate(&mut bundle.auth)?;
    if changed {
        let path = write_workspace_auth_bundle(&source_root, &bundle.auth)?;
        bundle.config_path = path;
        if let Err(error) = append_auth_journal_entry(&source_root, action, actor, summary, patch) {
            tracing::warn!(
                error = %error,
                source_root = %source_root.display(),
                action,
                "failed to append auth journal entry after config write"
            );
        }
    }
    Ok((result, bundle))
}

pub fn upsert_workspace_user(
    source_root: &Path,
    username: &str,
    profile: &str,
    role: AuthRole,
    password_hash: &str,
    app_allowlist: &[String],
    app_denylist: &[String],
    scene_allowlist: &BTreeMap<String, Vec<String>>,
) -> Result<()> {
    let normalized_username = username.trim().to_string();
    if normalized_username.is_empty() {
        anyhow::bail!("username is required");
    }
    validate_password_hash_format(password_hash)?;
    let role_list = vec![role.as_str().to_string()];
    let normalized_app_allowlist = normalize_app_id_list(app_allowlist);
    let normalized_app_denylist = normalize_app_id_list(app_denylist);
    let normalized_scene_allowlist = normalize_scene_allowlist(scene_allowlist);
    let profile_value = profile.trim().to_string();
    let password_hash_value = password_hash.trim().to_string();
    apply_workspace_auth_mutation(
        source_root,
        "auth.user_upsert",
        "system",
        format!("upsert auth user `{normalized_username}`").as_str(),
        json!({
            "username": normalized_username,
            "role": role.as_str(),
            "app_allowlist_count": normalized_app_allowlist.len(),
            "app_denylist_count": normalized_app_denylist.len(),
            "scene_allowlist_count": normalized_scene_allowlist.len(),
        }),
        |auth| {
            let mut changed = false;
            let normalized_username_id = normalize_id(&normalized_username);
            let mut updated = false;
            for user in &mut auth.users {
                if normalize_id(&user.username) == normalized_username_id {
                    let current_app = normalize_app_id_list(&user.app_allowlist);
                    let current_deny = normalize_app_id_list(&user.app_denylist);
                    let current_scene = normalize_scene_allowlist(&user.scene_allowlist);
                    let needs_update = user.profile.trim() != profile_value
                        || user.password_hash.trim() != password_hash_value
                        || user.roles != role_list
                        || current_app != normalized_app_allowlist
                        || current_deny != normalized_app_denylist
                        || current_scene != normalized_scene_allowlist
                        || user.disabled;
                    if needs_update {
                        user.profile = profile_value.clone();
                        user.roles = role_list.clone();
                        user.password_hash = password_hash_value.clone();
                        user.app_allowlist = normalized_app_allowlist.clone();
                        user.app_denylist = normalized_app_denylist.clone();
                        user.scene_allowlist = normalized_scene_allowlist.clone();
                        user.disabled = false;
                        changed = true;
                    }
                    updated = true;
                    break;
                }
            }
            if !updated {
                auth.users.push(AuthUserConfig {
                    username: normalized_username.clone(),
                    profile: profile_value.clone(),
                    password_hash: password_hash_value.clone(),
                    roles: role_list.clone(),
                    app_allowlist: normalized_app_allowlist.clone(),
                    app_denylist: normalized_app_denylist.clone(),
                    scene_allowlist: normalized_scene_allowlist.clone(),
                    disabled: false,
                });
                changed = true;
            }
            Ok(((), changed))
        },
    )?;
    Ok(())
}

pub fn set_workspace_user_disabled(
    source_root: &Path,
    username: &str,
    disabled: bool,
) -> Result<()> {
    let normalized_username = username.trim().to_string();
    if normalized_username.is_empty() {
        anyhow::bail!("username is required");
    }
    let (_, _) = apply_workspace_auth_mutation(
        source_root,
        if disabled {
            "auth.user_disabled"
        } else {
            "auth.user_enabled"
        },
        "system",
        format!(
            "{} auth user `{normalized_username}`",
            if disabled { "disable" } else { "enable" }
        )
        .as_str(),
        json!({
            "username": normalized_username,
            "disabled": disabled,
        }),
        |auth| {
            let normalized_username_id = normalize_id(&normalized_username);
            for user in &mut auth.users {
                if normalize_id(&user.username) == normalized_username_id {
                    let changed = user.disabled != disabled;
                    if changed {
                        user.disabled = disabled;
                    }
                    return Ok(((), changed));
                }
            }
            anyhow::bail!("user `{}` not found", normalized_username);
        },
    )?;
    Ok(())
}

pub fn update_workspace_user_password(
    source_root: &Path,
    username: &str,
    password_hash: &str,
    actor: &str,
) -> Result<()> {
    validate_password_hash_format(password_hash)?;
    let normalized_username = username.trim().to_string();
    if normalized_username.is_empty() {
        anyhow::bail!("username is required");
    }
    let password_hash_value = password_hash.trim().to_string();
    let (_, _) = apply_workspace_auth_mutation(
        source_root,
        "auth.password_change",
        actor,
        format!("change password for `{normalized_username}`").as_str(),
        json!({
            "username": normalized_username,
        }),
        |auth| {
            let normalized_username_id = normalize_id(&normalized_username);
            for user in &mut auth.users {
                if normalize_id(&user.username) == normalized_username_id {
                    let changed = user.password_hash.trim() != password_hash_value || user.disabled;
                    if changed {
                        user.password_hash = password_hash_value.clone();
                        user.disabled = false;
                    }
                    return Ok(((), changed));
                }
            }
            anyhow::bail!("user `{}` not found", normalized_username);
        },
    )?;
    Ok(())
}

pub fn ensure_workspace_auth_base(source_root: &Path) -> Result<WorkspaceAuthBundle> {
    let (_, bundle) = apply_workspace_auth_mutation(
        source_root,
        "auth.ensure_keys",
        "system",
        "ensure auth key and jwt base",
        json!({}),
        |auth| {
            let mut changed = false;
            if auth.jwt_secret.as_deref().unwrap_or("").trim().is_empty() {
                auth.jwt_secret = Some(random_jwt_secret());
                changed = true;
            }
            if auth.jwt_ttl_seconds.is_none() {
                auth.jwt_ttl_seconds = Some(DEFAULT_JWT_TTL_SECONDS);
                changed = true;
            }
            if auth.cookie_name.is_none() {
                auth.cookie_name = Some(DEFAULT_JWT_COOKIE_NAME.to_string());
                changed = true;
            }
            if auth.key_pair.public_key_pem.trim().is_empty()
                || auth.key_pair.private_key_pem.trim().is_empty()
            {
                let (public, private) = generate_key_pair_pem()?;
                auth.key_pair.public_key_pem = public;
                auth.key_pair.private_key_pem = private;
                auth.key_pair.created_at = Some(chrono::Utc::now().to_rfc3339());
                changed = true;
            }
            Ok(((), changed))
        },
    )?;
    Ok(bundle)
}

pub fn rotate_workspace_key_pair(source_root: &Path) -> Result<()> {
    let (_, _) = apply_workspace_auth_mutation(
        source_root,
        "auth.rotate_keys",
        "system",
        "rotate auth rsa key pair",
        json!({}),
        |auth| {
            let (public, private) = generate_key_pair_pem()?;
            auth.key_pair.public_key_pem = public;
            auth.key_pair.private_key_pem = private;
            auth.key_pair.created_at = Some(chrono::Utc::now().to_rfc3339());
            Ok(((), true))
        },
    )?;
    Ok(())
}
