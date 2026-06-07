use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use mei_lang_kernel::{
    append_auth_journal_entry, load_workspace_auth_bundle, write_workspace_auth_bundle,
    AuthUserConfig, WorkspaceAuthBundle, WorkspaceAuthConfig,
};
use rand::{distributions::Alphanumeric, rngs::OsRng, seq::SliceRandom, Rng};
use rsa::{
    pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding},
    Oaep, RsaPrivateKey, RsaPublicKey,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;

use crate::AppState;

const DEFAULT_JWT_COOKIE_NAME: &str = "mei_auth_token";
const DEFAULT_JWT_TTL_SECONDS: u64 = 8 * 60 * 60;
const MIN_PASSWORD_LEN: usize = 12;
const DEFAULT_TEMP_PASSWORD_LEN: usize = 20;

static WORKSPACE_AUTH_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();

/// 宿主启动时的认证策略：默认 `disabled`；`serve --auth` 时设为 `required`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthEnforcement {
    Required,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthRole {
    Guest,
    Admin,
    Super,
}

impl AuthRole {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthRole::Guest => "guest",
            AuthRole::Admin => "admin",
            AuthRole::Super => "super",
        }
    }

    pub fn from_slug(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "guest" => Some(Self::Guest),
            "admin" => Some(Self::Admin),
            "super" => Some(Self::Super),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,
    pub profile: String,
    pub role: String,
    #[serde(default)]
    pub app_allowlist: Vec<String>,
    #[serde(default)]
    pub scene_allowlist: BTreeMap<String, Vec<String>>,
    pub iat: usize,
    pub exp: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthPrincipal {
    pub username: String,
    pub profile: String,
    pub role: AuthRole,
    pub app_allowlist: BTreeSet<String>,
    pub scene_allowlist: BTreeMap<String, BTreeSet<String>>,
}

impl AuthPrincipal {
    pub fn from_claims(claims: &AuthClaims) -> Self {
        let role = AuthRole::from_slug(&claims.role).unwrap_or(AuthRole::Guest);
        let app_allowlist = claims
            .app_allowlist
            .iter()
            .map(|value| normalize_id(value))
            .filter(|value| !value.is_empty())
            .collect::<BTreeSet<_>>();
        let mut scene_allowlist = BTreeMap::new();
        for (app, scenes) in &claims.scene_allowlist {
            let app_id = normalize_id(app);
            if app_id.is_empty() {
                continue;
            }
            let allowed = scenes
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<BTreeSet<_>>();
            if !allowed.is_empty() {
                scene_allowlist.insert(app_id, allowed);
            }
        }
        Self {
            username: claims.sub.clone(),
            profile: claims.profile.clone(),
            role,
            app_allowlist,
            scene_allowlist,
        }
    }

    pub fn role_slug(&self) -> &'static str {
        self.role.as_str()
    }

    pub fn can_use_authoring_surface(&self) -> bool {
        matches!(self.role, AuthRole::Admin | AuthRole::Super)
    }

    pub fn can_manage_sensitive_api(&self) -> bool {
        matches!(self.role, AuthRole::Super)
    }

    pub fn can_access_app(&self, app_id: &str) -> bool {
        if !matches!(self.role, AuthRole::Guest) {
            return true;
        }
        if self.app_allowlist.is_empty() {
            return false;
        }
        self.app_allowlist.contains(&normalize_id(app_id))
    }

    pub fn can_access_scene(&self, app_id: &str, scene_id: &str) -> bool {
        if !matches!(self.role, AuthRole::Guest) {
            return true;
        }
        if !self.can_access_app(app_id) {
            return false;
        }
        match self.scene_allowlist.get(&normalize_id(app_id)) {
            Some(allowed) => allowed.contains(scene_id.trim()),
            None => true,
        }
    }
}

#[derive(Debug, Clone)]
struct AuthUserRecord {
    username: String,
    profile: String,
    password_hash: String,
    role: AuthRole,
    app_allowlist: Vec<String>,
    scene_allowlist: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct AuthRuntime {
    /// 密钥与用户齐备，可完成登录与 JWT 签发。
    pub enabled: bool,
    pub config_path: PathBuf,
    pub cookie_name: String,
    pub jwt_ttl_seconds: u64,
    pub jwt_secret: String,
    pub public_key_pem: String,
    pub private_key_pem: String,
    users: HashMap<String, AuthUserRecord>,
}

pub fn load_auth_runtime(source_root: &Path) -> Result<AuthRuntime> {
    let source_root = source_root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize source root {}",
            source_root.display()
        )
    })?;
    let bundle = load_workspace_auth_bundle(&source_root);
    let config_path = bundle.config_path;
    let auth = bundle.auth;
    let cookie_name = auth
        .cookie_name
        .clone()
        .unwrap_or_else(|| DEFAULT_JWT_COOKIE_NAME.to_string());
    let jwt_ttl_seconds = auth.jwt_ttl_seconds.unwrap_or(DEFAULT_JWT_TTL_SECONDS);
    let jwt_secret = auth.jwt_secret.clone().unwrap_or_default();
    let public_key_pem = auth.key_pair.public_key_pem.clone();
    let private_key_pem = auth.key_pair.private_key_pem.clone();

    let users = auth
        .users
        .iter()
        .filter_map(parse_user_record)
        .map(|record| (record.username.clone(), record))
        .collect::<HashMap<_, _>>();
    let enabled = !users.is_empty()
        && !jwt_secret.trim().is_empty()
        && !public_key_pem.trim().is_empty()
        && !private_key_pem.trim().is_empty();
    Ok(AuthRuntime {
        enabled,
        config_path,
        cookie_name,
        jwt_ttl_seconds,
        jwt_secret,
        public_key_pem,
        private_key_pem,
        users,
    })
}

fn parse_user_record(user: &AuthUserConfig) -> Option<AuthUserRecord> {
    if user.disabled {
        return None;
    }
    let username = user.username.trim().to_string();
    if username.is_empty() {
        return None;
    }
    let role = user
        .roles
        .iter()
        .find_map(|value| AuthRole::from_slug(value))
        .unwrap_or(AuthRole::Guest);
    Some(AuthUserRecord {
        username,
        profile: user.profile.trim().to_string(),
        password_hash: user.password_hash.clone(),
        role,
        app_allowlist: user
            .app_allowlist
            .iter()
            .map(|value| normalize_id(value))
            .filter(|value| !value.is_empty())
            .collect(),
        scene_allowlist: user
            .scene_allowlist
            .iter()
            .map(|(key, values)| {
                (
                    normalize_id(key),
                    values
                        .iter()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .collect::<Vec<_>>(),
                )
            })
            .filter(|(app, scenes)| !app.is_empty() && !scenes.is_empty())
            .collect(),
    })
}

pub fn normalize_id(value: &str) -> String {
    value.trim().trim_start_matches('/').replace('\\', "/")
}

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

fn normalize_app_allowlist(values: &[String]) -> Vec<String> {
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

pub fn now_ts() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as usize)
        .unwrap_or(0)
}

impl AuthRuntime {
    pub fn user_count(&self) -> usize {
        self.users.len()
    }

    pub fn cookie_name(&self) -> &str {
        self.cookie_name.as_str()
    }

    pub fn authenticate(&self, username: &str, password: &str) -> Result<Option<AuthClaims>> {
        if !self.enabled {
            return Ok(None);
        }
        let key = username.trim();
        let Some(user) = self.users.get(key) else {
            return Ok(None);
        };
        if !verify_password_hash(password, &user.password_hash)? {
            return Ok(None);
        }
        let iat = now_ts();
        let exp = iat + self.jwt_ttl_seconds as usize;
        Ok(Some(AuthClaims {
            sub: user.username.clone(),
            profile: user.profile.clone(),
            role: user.role.as_str().to_string(),
            app_allowlist: user.app_allowlist.clone(),
            scene_allowlist: user.scene_allowlist.clone(),
            iat,
            exp,
        }))
    }

    pub fn issue_jwt(&self, claims: &AuthClaims) -> Result<String> {
        let mut header = Header::new(Algorithm::HS256);
        header.typ = Some("JWT".to_string());
        encode(
            &header,
            claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .context("failed to encode jwt")
    }

    pub fn decode_jwt(&self, token: &str) -> Result<AuthClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        decode::<AuthClaims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &validation,
        )
        .map(|data| data.claims)
        .context("failed to decode jwt")
    }

    pub fn decrypt_password_field(&self, encrypted: &str) -> Result<String> {
        decrypt_base64_with_private_key(self.private_key_pem.as_str(), encrypted)
    }
}

pub fn generate_key_pair_pem() -> Result<(String, String)> {
    let mut rng = OsRng;
    let private = RsaPrivateKey::new(&mut rng, 2048).context("failed to generate rsa keypair")?;
    let public = RsaPublicKey::from(&private);
    let private_pem = private
        .to_pkcs8_pem(LineEnding::LF)
        .context("failed to encode private key pem")?
        .to_string();
    let public_pem = public
        .to_public_key_pem(LineEnding::LF)
        .context("failed to encode public key pem")?;
    Ok((public_pem, private_pem))
}

pub fn random_jwt_secret() -> String {
    let mut rng = OsRng;
    (0..48).map(|_| rng.sample(Alphanumeric) as char).collect()
}

pub fn hash_password(password: &str) -> Result<String> {
    validate_password_complexity(password)?;
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|value| value.to_string())
        .map_err(|error| anyhow::anyhow!("failed to hash password: {error}"))
}

pub fn verify_password_hash(password: &str, hash: &str) -> Result<bool> {
    if hash.trim().is_empty() {
        return Ok(false);
    }
    let parsed = PasswordHash::new(hash)
        .map_err(|error| anyhow::anyhow!("invalid password hash format: {error}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn validate_password_complexity(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        anyhow::bail!("password must be at least {MIN_PASSWORD_LEN} characters");
    }
    let mut has_upper = false;
    let mut has_lower = false;
    let mut has_digit = false;
    let mut has_symbol = false;
    for ch in password.chars() {
        if ch.is_ascii_uppercase() {
            has_upper = true;
        } else if ch.is_ascii_lowercase() {
            has_lower = true;
        } else if ch.is_ascii_digit() {
            has_digit = true;
        } else {
            has_symbol = true;
        }
    }
    if !has_upper || !has_lower || !has_digit || !has_symbol {
        anyhow::bail!("password must include upper/lower/digit/symbol");
    }
    Ok(())
}

pub fn decrypt_base64_with_private_key(private_pem: &str, encrypted: &str) -> Result<String> {
    let private_key = RsaPrivateKey::from_pkcs8_pem(private_pem)
        .context("invalid private key pem for auth decrypt")?;
    let cipher = BASE64_STANDARD
        .decode(encrypted.trim())
        .context("invalid base64 encrypted payload")?;
    let plain = private_key
        .decrypt(Oaep::new::<Sha256>(), &cipher)
        .context("failed to decrypt encrypted payload")?;
    String::from_utf8(plain).context("decrypted payload is not utf8")
}

pub fn generate_temporary_password() -> String {
    let mut rng = OsRng;
    let upper = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
    let lower = b"abcdefghijkmnopqrstuvwxyz";
    let digit = b"23456789";
    let symbol = b"!@#$%^&*_+-=";
    let all = [upper.as_slice(), lower.as_slice(), digit.as_slice(), symbol.as_slice()]
        .concat();
    let mut chars = vec![
        *upper.choose(&mut rng).unwrap_or(&b'A'),
        *lower.choose(&mut rng).unwrap_or(&b'a'),
        *digit.choose(&mut rng).unwrap_or(&b'2'),
        *symbol.choose(&mut rng).unwrap_or(&b'!'),
    ];
    while chars.len() < DEFAULT_TEMP_PASSWORD_LEN {
        chars.push(*all.choose(&mut rng).unwrap_or(&b'A'));
    }
    chars.shuffle(&mut rng);
    chars.into_iter().map(char::from).collect()
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
    scene_allowlist: &BTreeMap<String, Vec<String>>,
) -> Result<()> {
    let normalized_username = username.trim().to_string();
    if normalized_username.is_empty() {
        anyhow::bail!("username is required");
    }
    validate_password_hash_format(password_hash)?;
    let role_list = vec![role.as_str().to_string()];
    let normalized_app_allowlist = normalize_app_allowlist(app_allowlist);
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
            "scene_allowlist_count": normalized_scene_allowlist.len(),
        }),
        |auth| {
            let mut changed = false;
            let normalized_username_id = normalize_id(&normalized_username);
            let mut updated = false;
            for user in &mut auth.users {
                if normalize_id(&user.username) == normalized_username_id {
                    let current_app = normalize_app_allowlist(&user.app_allowlist);
                    let current_scene = normalize_scene_allowlist(&user.scene_allowlist);
                    let needs_update = user.profile.trim() != profile_value
                        || user.password_hash.trim() != password_hash_value
                        || user.roles != role_list
                        || current_app != normalized_app_allowlist
                        || current_scene != normalized_scene_allowlist
                        || user.disabled;
                    if needs_update {
                        user.profile = profile_value.clone();
                        user.roles = role_list.clone();
                        user.password_hash = password_hash_value.clone();
                        user.app_allowlist = normalized_app_allowlist.clone();
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

pub fn set_workspace_user_disabled(source_root: &Path, username: &str, disabled: bool) -> Result<()> {
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
                    let changed =
                        user.password_hash.trim() != password_hash_value || user.disabled;
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

pub fn cookie_header_value(cookie_name: &str, token: &str, max_age: u64) -> String {
    format!(
        "{cookie_name}={token}; HttpOnly; Path=/; Max-Age={max_age}; SameSite=Lax"
    )
}

pub fn clear_cookie_header_value(cookie_name: &str) -> String {
    format!("{cookie_name}=; HttpOnly; Path=/; Max-Age=0; SameSite=Lax")
}

pub fn extract_token_from_headers(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    if let Some(cookie_header) = headers.get(header::COOKIE).and_then(|value| value.to_str().ok()) {
        for pair in cookie_header.split(';') {
            let mut it = pair.trim().splitn(2, '=');
            let key = it.next().unwrap_or("").trim();
            let value = it.next().unwrap_or("").trim();
            if key == cookie_name && !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    if let Some(auth) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

fn is_public_path(path: &str) -> bool {
    path == "/login"
        || path == "/logout"
        || path == "/favicon.ico"
        || path.starts_with("/app-assets/")
        || path.starts_with("/app-bundles/")
        || path == "/api/auth/public-key"
        || path == "/api/auth/login"
        || path == "/api/auth/session"
        || path == "/api/auth/logout"
}

fn extract_wildcard_app_id(path: &str, prefix: &str) -> Option<String> {
    let rest = path.strip_prefix(prefix)?;
    let app_id = rest.split('/').next().unwrap_or("").trim();
    if app_id.is_empty() {
        None
    } else {
        Some(normalize_id(app_id))
    }
}

fn extract_app_route_context(path: &str) -> Option<(String, String, Option<String>)> {
    let rest = path.strip_prefix("/apps/")?;
    let mut segments = rest.splitn(2, '/');
    let mode = segments.next().unwrap_or("").trim().to_ascii_lowercase();
    let app_raw = segments.next().unwrap_or("").trim();
    if app_raw.is_empty() {
        return None;
    }
    let (app_id, scene_id) = if mode == "app" || mode == "access" || mode == "access-only" {
        if let Some((app, scene)) = app_raw.split_once("/scene/") {
            (normalize_id(app), Some(scene.trim().to_string()))
        } else {
            (normalize_id(app_raw), None)
        }
    } else {
        (normalize_id(app_raw), None)
    };
    if app_id.is_empty() {
        return None;
    }
    Some((mode, app_id, scene_id.filter(|value| !value.is_empty())))
}

fn extract_api_app_id(path: &str) -> Option<String> {
    for prefix in [
        "/api/projection/",
        "/api/world/context/",
        "/api/world/assets/",
        "/api/world/asset/",
        "/api/world/runtime/",
        "/api/sim/step/",
        "/api/datasets/query/",
        "/api/datasets/metrics/",
        "/api/datasets/recompute/",
        "/api/ops/config/",
        "/api/ops/journal/",
        "/api/upload/",
        "/workspace-app-assets/",
    ] {
        if let Some(app_id) = extract_wildcard_app_id(path, prefix) {
            return Some(app_id);
        }
    }
    None
}

fn is_api_path(path: &str) -> bool {
    path.starts_with("/api/")
}

fn percent_encode_component(raw: &str) -> String {
    let mut out = String::new();
    for b in raw.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(char::from(*b));
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

fn unauthorized_response(path: &str, uri: &axum::http::Uri) -> Response {
    if is_api_path(path) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"authentication required"})),
        )
            .into_response();
    }
    let next = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or(path);
    Redirect::temporary(&format!("/login?next={}", percent_encode_component(next))).into_response()
}

fn forbidden_response(path: &str, message: &str) -> Response {
    if is_api_path(path) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": message}))).into_response();
    }
    (
        StatusCode::FORBIDDEN,
        format!("forbidden: {message}. <a href=\"/login\">重新登录</a>"),
    )
        .into_response()
}

fn authorize_path(path: &str, principal: &AuthPrincipal) -> Result<()> {
    if let Some((mode, app_id, scene_id)) = extract_app_route_context(path) {
        if !principal.can_access_app(app_id.as_str()) {
            anyhow::bail!("app `{app_id}` is not in guest allowlist");
        }
        if let Some(scene_id) = scene_id.as_deref() {
            if !principal.can_access_scene(app_id.as_str(), scene_id) {
                anyhow::bail!("scene `{scene_id}` is not in guest allowlist");
            }
        }
        if mode == "build" || mode == "config" || mode == "upload" {
            if !principal.can_use_authoring_surface() {
                anyhow::bail!("current role cannot access authoring routes");
            }
        }
        return Ok(());
    }
    if let Some(app_id) = extract_api_app_id(path) {
        if !principal.can_access_app(app_id.as_str()) {
            anyhow::bail!("app `{app_id}` is not in guest allowlist");
        }
    }
    if path.starts_with("/api/ops/") || path.starts_with("/api/upload/") {
        if !principal.can_use_authoring_surface() {
            anyhow::bail!("current role cannot access write api");
        }
    }
    if path.starts_with("/api/agent/") {
        if !principal.can_manage_sensitive_api() {
            anyhow::bail!("current role cannot access agent control api");
        }
    }
    if path.starts_with("/workspace-components/") {
        if !principal.can_use_authoring_surface() {
            anyhow::bail!("current role cannot access component assets");
        }
    }
    Ok(())
}

pub fn prepare_auth_for_serve(source_root: &Path, enforcement: AuthEnforcement) -> Result<()> {
    if enforcement != AuthEnforcement::Required {
        return Ok(());
    }
    let _ = ensure_workspace_auth_base(source_root)?;
    let runtime = load_auth_runtime(source_root)?;
    if !runtime.enabled {
        anyhow::bail!(
            "host auth is enabled (--auth) but workspace auth is not ready (no users or incomplete key material); \
             configure users in `{}` (passwordHash only) and run `mei host auth ensure-keys --source-root {}`; config_path={}",
            source_root.join(".mei-workspace.json").display(),
            source_root.display(),
            runtime.config_path.display()
        );
    }
    tracing::info!(
        config_path = %runtime.config_path.display(),
        user_count = runtime.user_count(),
        "host auth enabled: login enforced for all protected routes"
    );
    Ok(())
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    if state.auth_enforcement == AuthEnforcement::Disabled {
        return next.run(request).await;
    }
    let runtime = match load_auth_runtime(state.source_root.as_path()) {
        Ok(value) => value,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("failed to load auth config: {error}")})),
            )
                .into_response()
        }
    };
    let path = request.uri().path().to_string();
    let maybe_token = extract_token_from_headers(request.headers(), runtime.cookie_name());
    let principal = maybe_token
        .as_deref()
        .and_then(|token| runtime.decode_jwt(token).ok())
        .map(|claims| AuthPrincipal::from_claims(&claims));

    if let Some(ref principal) = principal {
        if let Err(error) = authorize_path(&path, principal) {
            return forbidden_response(path.as_str(), &error.to_string());
        }
    }

    if is_public_path(path.as_str()) {
        if let Some(principal) = principal {
            request.extensions_mut().insert(principal);
        }
        return next.run(request).await;
    }

    let Some(principal) = principal else {
        return unauthorized_response(path.as_str(), request.uri());
    };
    request.extensions_mut().insert(principal);
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
    use rsa::pkcs8::DecodePublicKey;
    use tower::ServiceExt;
    use mei_lang_kernel::AuthJournal;

    use crate::{agent_runtime, mei_agent, AppState, SessionContextSnapshot};

    use super::*;

    fn temp_source_root(label: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        dir.push(format!("mei-auth-test-{label}-{stamp}"));
        fs::create_dir_all(&dir).expect("create temp source root");
        dir
    }

    fn make_state(source_root: PathBuf, enforcement: AuthEnforcement) -> AppState {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("server crate parent")
            .to_path_buf();
        let native_agent =
            Arc::new(mei_agent::NativeAgent::open(source_root.clone()).expect("native agent"));
        AppState {
            package_root: Arc::new(package_root),
            source_root: Arc::new(source_root),
            agent_preferred_mode: Arc::new("native".to_string()),
            agent_preferred_server_url: Arc::new(String::new()),
            agent_auto_start: false,
            auth_enforcement: enforcement,
            agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
            agent_session_context: Arc::new(Mutex::new(HashMap::<
                String,
                SessionContextSnapshot,
            >::new())),
            native_agent,
        }
    }

    fn bootstrap_guest_user(source_root: &Path, app_allow: &[&str]) {
        ensure_workspace_auth_base(source_root).expect("ensure auth base");
        let hash = hash_password("GuestPwd1!safe").expect("hash guest password");
        let app_allow = app_allow.iter().map(|value| value.to_string()).collect::<Vec<_>>();
        upsert_workspace_user(
            source_root,
            "guest01",
            "访客",
            AuthRole::Guest,
            hash.as_str(),
            &app_allow,
            &BTreeMap::new(),
        )
        .expect("upsert guest");
    }

    fn bootstrap_admin_user(source_root: &Path) {
        ensure_workspace_auth_base(source_root).expect("ensure auth base");
        let hash = hash_password("AdminPwd1!safe").expect("hash admin password");
        upsert_workspace_user(
            source_root,
            "admin01",
            "管理员",
            AuthRole::Admin,
            hash.as_str(),
            &[],
            &BTreeMap::new(),
        )
        .expect("upsert admin");
    }

    fn token_for(source_root: &Path, username: &str, password: &str) -> String {
        let runtime = load_auth_runtime(source_root).expect("load runtime");
        let claims = runtime
            .authenticate(username, password)
            .expect("auth result")
            .expect("claims");
        runtime.issue_jwt(&claims).expect("issue jwt")
    }

    #[test]
    fn password_complexity_requires_multiple_char_classes() {
        assert!(validate_password_complexity("Aa1!12345678").is_ok());
        assert!(validate_password_complexity("aaaaaa").is_err());
        assert!(validate_password_complexity("NO_LOWER_123!").is_err());
    }

    #[test]
    fn temporary_password_meets_complexity() {
        let password = generate_temporary_password();
        assert!(validate_password_complexity(password.as_str()).is_ok());
    }

    #[test]
    fn rsa_roundtrip_for_sensitive_payload() {
        let (public_pem, private_pem) = generate_key_pair_pem().expect("generate keypair");
        let public = RsaPublicKey::from_public_key_pem(public_pem.as_str()).expect("public key");
        let mut rng = rand::rngs::OsRng;
        let encrypted = public
            .encrypt(&mut rng, Oaep::new::<Sha256>(), b"Hello#Sensitive1")
            .expect("encrypt");
        let encrypted_b64 = BASE64_STANDARD.encode(encrypted);
        let decrypted =
            decrypt_base64_with_private_key(private_pem.as_str(), encrypted_b64.as_str()).expect("decrypt");
        assert_eq!(decrypted, "Hello#Sensitive1");
    }

    #[test]
    fn prepare_auth_fails_without_users_when_required() {
        let source_root = temp_source_root("prepare-no-users");
        let err = prepare_auth_for_serve(source_root.as_path(), AuthEnforcement::Required)
            .expect_err("should fail without users");
        assert!(err.to_string().contains("host auth is enabled"));
    }

    #[test]
    fn auth_mutation_appends_workspace_journal_entry() {
        let source_root = temp_source_root("auth-journal");
        ensure_workspace_auth_base(source_root.as_path()).expect("ensure auth base");
        let hash = hash_password("GuestPwd1!safe").expect("hash");
        upsert_workspace_user(
            source_root.as_path(),
            "guest-journal",
            "访客",
            AuthRole::Guest,
            hash.as_str(),
            &[],
            &BTreeMap::new(),
        )
        .expect("upsert");
        let journal = AuthJournal::load(source_root.as_path());
        assert!(journal.revision >= 1);
        assert!(!journal.entries.is_empty());
    }

    #[tokio::test]
    async fn disabled_auth_session_api_returns_not_found() {
        let source_root = temp_source_root("disabled-auth-api");
        let state = make_state(source_root.clone(), AuthEnforcement::Disabled);
        let app = Router::new()
            .route(
                "/api/auth/session",
                get(crate::http::auth_api::auth_session),
            )
            .with_state(state);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/auth/session")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn prepare_auth_skips_when_disabled() {
        let source_root = temp_source_root("prepare-disabled");
        prepare_auth_for_serve(source_root.as_path(), AuthEnforcement::Disabled)
            .expect("disabled should pass");
    }

    #[tokio::test]
    async fn required_auth_blocks_even_when_users_not_configured() {
        let source_root = temp_source_root("required-no-users");
        ensure_workspace_auth_base(source_root.as_path()).expect("ensure base");
        let state = make_state(source_root.clone(), AuthEnforcement::Required);
        let app = Router::new()
            .route("/", get(|| async { "home" }))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
    }

    #[tokio::test]
    async fn disabled_auth_allows_anonymous_access() {
        let source_root = temp_source_root("auth-disabled");
        let state = make_state(source_root.clone(), AuthEnforcement::Disabled);
        let app = Router::new()
            .route("/apps/build/demo", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/apps/build/demo")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_redirects_unauthenticated_pages_with_next() {
        let source_root = temp_source_root("redirect");
        bootstrap_guest_user(source_root.as_path(), &["demo"]);
        let state = make_state(source_root.clone(), AuthEnforcement::Required);
        let app = Router::new()
            .route("/apps/build/demo", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state);

        let req = Request::builder()
            .uri("/apps/build/demo?tab=preview")
            .body(Body::empty())
            .expect("request");
        let resp = app.clone().oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
        let location = resp
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        assert!(
            location.contains("/login?next=/apps/build/demo%3Ftab%3Dpreview"),
            "unexpected location: {location}"
        );

        let api_req = Request::builder()
            .uri("/api/ops/config/demo")
            .body(Body::empty())
            .expect("api request");
        let api_resp = app.oneshot(api_req).await.expect("api response");
        assert_eq!(api_resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_blocks_guest_authoring_and_unauthorized_app() {
        let source_root = temp_source_root("guest-deny");
        bootstrap_guest_user(source_root.as_path(), &["demo"]);
        let token = token_for(source_root.as_path(), "guest01", "GuestPwd1!safe");
        let runtime = load_auth_runtime(source_root.as_path()).expect("runtime");
        let state = make_state(source_root.clone(), AuthEnforcement::Required);
        let app = Router::new()
            .route("/apps/build/demo", get(|| async { "ok" }))
            .route("/apps/app/blocked/scene/home", get(|| async { "ok" }))
            .route("/api/ops/config/demo", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state);

        let cookie = format!("{}={}", runtime.cookie_name, token);
        let req_authoring = Request::builder()
            .uri("/apps/build/demo")
            .header(header::COOKIE, cookie.as_str())
            .body(Body::empty())
            .expect("authoring request");
        let resp_authoring = app
            .clone()
            .oneshot(req_authoring)
            .await
            .expect("authoring response");
        assert_eq!(resp_authoring.status(), StatusCode::FORBIDDEN);

        let req_blocked_app = Request::builder()
            .uri("/apps/app/blocked/scene/home")
            .header(header::COOKIE, cookie.as_str())
            .body(Body::empty())
            .expect("blocked app request");
        let resp_blocked_app = app
            .clone()
            .oneshot(req_blocked_app)
            .await
            .expect("blocked app response");
        assert_eq!(resp_blocked_app.status(), StatusCode::FORBIDDEN);

        let req_ops_api = Request::builder()
            .uri("/api/ops/config/demo")
            .header(header::COOKIE, cookie.as_str())
            .body(Body::empty())
            .expect("ops request");
        let resp_ops_api = app.oneshot(req_ops_api).await.expect("ops response");
        assert_eq!(resp_ops_api.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn middleware_allows_admin_authoring_routes() {
        let source_root = temp_source_root("admin-allow");
        bootstrap_admin_user(source_root.as_path());
        let token = token_for(source_root.as_path(), "admin01", "AdminPwd1!safe");
        let runtime = load_auth_runtime(source_root.as_path()).expect("runtime");
        let state = make_state(source_root.clone(), AuthEnforcement::Required);
        let app = Router::new()
            .route("/apps/build/demo", get(|| async { "ok" }))
            .route("/api/ops/config/demo", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state);
        let cookie = format!("{}={}", runtime.cookie_name, token);

        let build_req = Request::builder()
            .uri("/apps/build/demo")
            .header(header::COOKIE, cookie.as_str())
            .body(Body::empty())
            .expect("build req");
        let build_resp = app.clone().oneshot(build_req).await.expect("build resp");
        assert_eq!(build_resp.status(), StatusCode::OK);

        let api_req = Request::builder()
            .uri("/api/ops/config/demo")
            .header(header::COOKIE, cookie.as_str())
            .body(Body::empty())
            .expect("api req");
        let api_resp = app.oneshot(api_req).await.expect("api resp");
        assert_eq!(api_resp.status(), StatusCode::OK);
    }
}

