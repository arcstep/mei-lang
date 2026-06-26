use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    auth::{
        AuthEnforcement, SESSION_REFRESH_LEAD_SECONDS,
    },
    AppState,
};


pub(super) fn auth_login_ready(state: &AppState, _runtime: &crate::auth::AuthRuntime) -> bool {
    state.auth_enforcement == AuthEnforcement::Required
}

pub(super) fn reject_if_auth_disabled(state: &AppState) -> Option<Response> {
    if state.auth_enforcement != AuthEnforcement::Required {
        Some(json_error(StatusCode::NOT_FOUND, "host auth is disabled"))
    } else {
        None
    }
}

pub(super) fn reject_page_if_auth_disabled(state: &AppState) -> Option<Response> {
    if state.auth_enforcement != AuthEnforcement::Required {
        Some(StatusCode::NOT_FOUND.into_response())
    } else {
        None
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginPageQuery {
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LogoutQuery {
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    #[serde(rename = "encryptedPassword")]
    #[serde(default)]
    pub encrypted_password: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    #[serde(rename = "encryptedCurrentPassword")]
    pub encrypted_current_password: String,
    #[serde(rename = "encryptedNewPassword")]
    pub encrypted_new_password: String,
}

#[derive(Debug, Serialize)]
pub(super) struct SessionUserPayload {
    pub username: String,
    pub profile: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub(super) struct SessionPayload {
    pub enabled: bool,
    pub authenticated: bool,
    pub user: Option<SessionUserPayload>,
    #[serde(rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<usize>,
    #[serde(rename = "jwtTtlSeconds", skip_serializing_if = "Option::is_none")]
    pub jwt_ttl_seconds: Option<u64>,
    #[serde(rename = "refreshLeadSeconds", skip_serializing_if = "Option::is_none")]
    pub refresh_lead_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
pub(super) struct RefreshPayload {
    pub ok: bool,
    #[serde(rename = "expiresAt")]
    pub expires_at: usize,
    #[serde(rename = "jwtTtlSeconds")]
    pub jwt_ttl_seconds: u64,
    #[serde(rename = "refreshLeadSeconds")]
    pub refresh_lead_seconds: u64,
    pub user: SessionUserPayload,
}

pub(super) fn session_refresh_lead_seconds() -> u64 {
    SESSION_REFRESH_LEAD_SECONDS
}

pub(super) fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({"error": message.into()}))).into_response()
}
