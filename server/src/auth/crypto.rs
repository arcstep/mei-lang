use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::http::{header, HeaderMap};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use rand::{distributions::Alphanumeric, rngs::OsRng, seq::SliceRandom, Rng};
use rsa::{
    pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding},
    Oaep, RsaPrivateKey, RsaPublicKey,
};
use sha2::Sha256;

const MIN_PASSWORD_LEN: usize = 8;
const DEFAULT_TEMP_PASSWORD_LEN: usize = 20;

pub fn now_ts() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as usize)
        .unwrap_or(0)
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
