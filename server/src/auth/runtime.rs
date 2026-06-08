use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use mei_lang_kernel::{load_workspace_auth_bundle, AuthUserConfig};

use super::crypto::{decrypt_base64_with_private_key, now_ts, verify_password_hash};
use super::types::{AuthClaims, AuthRole, AuthRuntime, AuthUserRecord};

pub(crate) const DEFAULT_JWT_COOKIE_NAME: &str = "mei_auth_token";
pub(crate) const DEFAULT_JWT_TTL_SECONDS: u64 = 8 * 60 * 60;

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
        app_denylist: user
            .app_denylist
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
            app_denylist: user.app_denylist.clone(),
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
