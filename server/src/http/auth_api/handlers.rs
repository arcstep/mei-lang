use axum::{
    extract::{Extension, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    auth::{
        authorize_next_path, clear_cookie_header_value, cookie_header_value, hash_password,
        load_auth_runtime, sanitize_next_path, update_workspace_user_password, AuthEnforcement,
        AuthPrincipal,
    },
    http::host_error_page,
    AppState,
};

use super::http_plaintext::host_allows_http_plaintext_login;
use super::pages::{change_password_page_html, login_page_html};

fn auth_login_ready(state: &AppState, _runtime: &crate::auth::AuthRuntime) -> bool {
    state.auth_enforcement == AuthEnforcement::Required
}

fn reject_if_auth_disabled(state: &AppState) -> Option<Response> {
    if state.auth_enforcement != AuthEnforcement::Required {
        Some(json_error(StatusCode::NOT_FOUND, "host auth is disabled"))
    } else {
        None
    }
}

fn reject_page_if_auth_disabled(state: &AppState) -> Option<Response> {
    if state.auth_enforcement != AuthEnforcement::Required {
        Some(StatusCode::NOT_FOUND.into_response())
    } else {
        None
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginPageQuery {
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LogoutQuery {
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    #[serde(rename = "encryptedPassword")]
    #[serde(default)]
    encrypted_password: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    #[serde(rename = "encryptedCurrentPassword")]
    encrypted_current_password: String,
    #[serde(rename = "encryptedNewPassword")]
    encrypted_new_password: String,
}

#[derive(Debug, Serialize)]
struct SessionUserPayload {
    username: String,
    profile: String,
    role: String,
}

#[derive(Debug, Serialize)]
struct SessionPayload {
    enabled: bool,
    authenticated: bool,
    user: Option<SessionUserPayload>,
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({"error": message.into()}))).into_response()
}
pub async fn login_page(
    State(state): State<AppState>,
    Query(query): Query<LoginPageQuery>,
    principal: Option<Extension<AuthPrincipal>>,
) -> impl IntoResponse {
    if let Some(response) = reject_page_if_auth_disabled(&state) {
        return response;
    }
    let runtime = match load_auth_runtime(state.source_root.as_path()) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    if let Some(Extension(principal)) = principal {
        if runtime.enabled {
            return Redirect::temporary(&authorize_next_path(query.next.as_deref(), &principal))
                .into_response();
        }
    }
    let footer_html =
        host_error_page::render_host_shell_footer_for_source_root(state.source_root.as_path());
    Html(login_page_html(
        sanitize_next_path(query.next.as_deref()).as_str(),
        auth_login_ready(&state, &runtime),
        runtime.enabled,
        footer_html.as_str(),
    ))
    .into_response()
}

pub async fn logout_page(
    State(state): State<AppState>,
    Query(query): Query<LogoutQuery>,
) -> impl IntoResponse {
    if let Some(response) = reject_page_if_auth_disabled(&state) {
        return response;
    }
    let runtime = match load_auth_runtime(state.source_root.as_path()) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let mut response =
        Redirect::temporary(&sanitize_next_path(query.next.as_deref())).into_response();
    let value = clear_cookie_header_value(runtime.cookie_name.as_str());
    if let Ok(header_value) = HeaderValue::from_str(&value) {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, header_value);
    }
    response
}

pub async fn account_change_password_page(
    State(state): State<AppState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> impl IntoResponse {
    if let Some(response) = reject_page_if_auth_disabled(&state) {
        return response;
    }
    let Some(Extension(principal)) = principal else {
        return Redirect::temporary("/login").into_response();
    };
    let footer_html =
        host_error_page::render_host_shell_footer_for_source_root(state.source_root.as_path());
    Html(change_password_page_html(
        principal.username.as_str(),
        principal.role_slug(),
        footer_html.as_str(),
    ))
    .into_response()
}

pub async fn auth_public_key(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(response) = reject_if_auth_disabled(&state) {
        return response;
    }
    let runtime = match load_auth_runtime(state.source_root.as_path()) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    if runtime.public_key_pem.trim().is_empty() {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "auth keypair is not configured",
        );
    }
    (
        StatusCode::OK,
        Json(json!({
            "algorithm": "RSA-OAEP-256",
            "public_key_pem": runtime.public_key_pem,
            "configured": runtime.enabled,
        })),
    )
        .into_response()
}

pub async fn auth_session(
    State(state): State<AppState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> impl IntoResponse {
    if let Some(response) = reject_if_auth_disabled(&state) {
        return response;
    }
    let enabled = load_auth_runtime(state.source_root.as_path())
        .map(|runtime| runtime.enabled)
        .unwrap_or(false);
    let payload = if let Some(Extension(principal)) = principal {
        let role = principal.role_slug().to_string();
        SessionPayload {
            enabled,
            authenticated: true,
            user: Some(SessionUserPayload {
                username: principal.username,
                profile: principal.profile,
                role,
            }),
        }
    } else {
        SessionPayload {
            enabled,
            authenticated: false,
            user: None,
        }
    };
    (StatusCode::OK, Json(payload)).into_response()
}

pub async fn auth_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    if let Some(response) = reject_if_auth_disabled(&state) {
        return response;
    }
    let runtime = match load_auth_runtime(state.source_root.as_path()) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    if !runtime.enabled {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "auth is not configured; add users in `.mei-workspace.json` (via `mei host auth bootstrap-users` or `add-user --password-stdin`) and ensure keys",
        );
    }
    let password = if let Some(encrypted) = body
        .encrypted_password
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        match runtime.decrypt_password_field(encrypted) {
            Ok(value) => value,
            Err(error) => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    format!("failed to decrypt password: {error}"),
                )
            }
        }
    } else if let Some(plain) = body.password.as_deref().filter(|value| !value.is_empty()) {
        let host = headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if !host_allows_http_plaintext_login(host) {
            return json_error(
                StatusCode::BAD_REQUEST,
                "password encryption is required for this host; use HTTPS or access via localhost / 23.211.135.152",
            );
        }
        plain.to_string()
    } else {
        return json_error(StatusCode::BAD_REQUEST, "password is required");
    };
    let Some(claims) = (match runtime.authenticate(body.username.as_str(), password.as_str()) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, error.to_string()),
    }) else {
        return json_error(StatusCode::UNAUTHORIZED, "invalid username or password");
    };
    let token = match runtime.issue_jwt(&claims) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let cookie = cookie_header_value(
        runtime.cookie_name.as_str(),
        token.as_str(),
        runtime.jwt_ttl_seconds,
    );
    let principal = AuthPrincipal::from_claims(&claims);
    let next = authorize_next_path(body.next.as_deref(), &principal);
    let mut response = (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "next": next,
            "user": {
                "username": claims.sub,
                "profile": claims.profile,
                "role": claims.role,
            }
        })),
    )
        .into_response();
    if let Ok(header_value) = HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, header_value);
    }
    response
}

pub async fn auth_logout(State(state): State<AppState>) -> impl IntoResponse {
    if let Some(response) = reject_if_auth_disabled(&state) {
        return response;
    }
    let runtime = match load_auth_runtime(state.source_root.as_path()) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let cookie = clear_cookie_header_value(runtime.cookie_name.as_str());
    let mut response = (StatusCode::OK, Json(json!({"ok": true}))).into_response();
    if let Ok(header_value) = HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, header_value);
    }
    response
}

pub async fn auth_change_password(
    State(state): State<AppState>,
    principal: Option<Extension<AuthPrincipal>>,
    Json(body): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    let Some(Extension(principal)) = principal else {
        return json_error(StatusCode::UNAUTHORIZED, "authentication required");
    };
    if let Some(response) = reject_if_auth_disabled(&state) {
        return response;
    }
    let runtime = match load_auth_runtime(state.source_root.as_path()) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    if !runtime.enabled {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "auth is not configured; add users in `.mei-workspace.json` (via `mei host auth bootstrap-users` or `add-user --password-stdin`) and ensure keys",
        );
    }
    let current_password =
        match runtime.decrypt_password_field(body.encrypted_current_password.as_str()) {
            Ok(value) => value,
            Err(error) => {
                return json_error(
                    StatusCode::BAD_REQUEST,
                    format!("failed to decrypt current password: {error}"),
                )
            }
        };
    let new_password = match runtime.decrypt_password_field(body.encrypted_new_password.as_str()) {
        Ok(value) => value,
        Err(error) => {
            return json_error(
                StatusCode::BAD_REQUEST,
                format!("failed to decrypt new password: {error}"),
            )
        }
    };
    if runtime
        .authenticate(principal.username.as_str(), current_password.as_str())
        .ok()
        .flatten()
        .is_none()
    {
        return json_error(StatusCode::UNAUTHORIZED, "current password is incorrect");
    }
    let new_hash = match hash_password(new_password.as_str()) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, error.to_string()),
    };
    let source_root = state.source_root.as_path();
    if let Err(error) = update_workspace_user_password(
        source_root,
        principal.username.as_str(),
        new_hash.as_str(),
        principal.username.as_str(),
    ) {
        if error.to_string().contains("not found") {
            return json_error(StatusCode::NOT_FOUND, "user not found");
        }
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to persist password: {error}"),
        );
    }
    let refreshed = match load_auth_runtime(source_root) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let Some(new_claims) =
        (match refreshed.authenticate(principal.username.as_str(), new_password.as_str()) {
            Ok(value) => value,
            Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        })
    else {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to re-issue session",
        );
    };
    let token = match refreshed.issue_jwt(&new_claims) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    let cookie = cookie_header_value(
        refreshed.cookie_name.as_str(),
        token.as_str(),
        refreshed.jwt_ttl_seconds,
    );
    let mut response = (
        StatusCode::OK,
        Json(json!({"ok": true, "message": "password changed"})),
    )
        .into_response();
    if let Ok(header_value) = HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, header_value);
    }
    response
}
