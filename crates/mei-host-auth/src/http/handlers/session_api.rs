use axum::{
    extract::{Extension, State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::state::AuthServeState;
use crate::{
    authorize_next_path, clear_cookie_header_value, cookie_header_value, hash_password,
    load_auth_runtime, update_workspace_user_password, AuthPrincipal,
};

use super::support::*;

pub async fn auth_session(
    State(state): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> impl IntoResponse {
    if let Some(response) = reject_if_auth_disabled(&state) {
        return response;
    }
    let runtime = match load_auth_runtime(state.source_root.as_path()) {
        Ok(value) => value,
        Err(_) => {
            return (
                StatusCode::OK,
                Json(SessionPayload {
                    enabled: false,
                    authenticated: false,
                    user: None,
                    expires_at: None,
                    jwt_ttl_seconds: None,
                    refresh_lead_seconds: None,
                }),
            )
                .into_response()
        }
    };
    let enabled = runtime.enabled;
    let jwt_ttl_seconds = Some(runtime.jwt_ttl_seconds);
    let refresh_lead_seconds = Some(session_refresh_lead_seconds());
    let payload = if let Some(Extension(principal)) = principal {
        let role = principal.role_slug().to_string();
        let expires_at = if principal.session_exp > 0 {
            Some(principal.session_exp)
        } else {
            None
        };
        SessionPayload {
            enabled,
            authenticated: true,
            user: Some(SessionUserPayload {
                username: principal.username,
                profile: principal.profile,
                role,
            }),
            expires_at,
            jwt_ttl_seconds,
            refresh_lead_seconds,
        }
    } else {
        SessionPayload {
            enabled,
            authenticated: false,
            user: None,
            expires_at: None,
            jwt_ttl_seconds,
            refresh_lead_seconds,
        }
    };
    (StatusCode::OK, Json(payload)).into_response()
}

pub async fn auth_refresh(
    State(state): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> impl IntoResponse {
    if let Some(response) = reject_if_auth_disabled(&state) {
        return response;
    }
    let Some(Extension(principal)) = principal else {
        return json_error(StatusCode::UNAUTHORIZED, "authentication required");
    };
    let runtime = match load_auth_runtime(state.source_root.as_path()) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    if !runtime.enabled {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "auth is not configured; initialize `.mei/local/hosts/*.state.json` via `mei-host-shell auth ensure-keys` and `mei-host-shell auth bootstrap-users` (or `add-user --password-stdin`)",
        );
    }
    let claims = match runtime.refresh_claims_for_user(principal.username.as_str()) {
        Ok(value) => value,
        Err(error) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to refresh session: {error}"),
            )
        }
    };
    let Some(claims) = claims else {
        return json_error(StatusCode::UNAUTHORIZED, "session refresh failed");
    };
    let token = match runtime.issue_jwt(&claims) {
        Ok(value) => value,
        Err(error) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to issue refreshed session: {error}"),
            )
        }
    };
    let cookie = cookie_header_value(
        runtime.cookie_name.as_str(),
        token.as_str(),
        runtime.jwt_ttl_seconds,
    );
    let payload = RefreshPayload {
        ok: true,
        expires_at: claims.exp,
        jwt_ttl_seconds: runtime.jwt_ttl_seconds,
        refresh_lead_seconds: session_refresh_lead_seconds(),
        user: SessionUserPayload {
            username: claims.sub,
            profile: claims.profile,
            role: claims.role,
        },
    };
    let mut response = (StatusCode::OK, Json(payload)).into_response();
    if let Ok(header_value) = HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, header_value);
    }
    response
}

pub async fn auth_login(
    State(state): State<AuthServeState>,
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
            "auth is not configured; initialize `.mei/local/hosts/*.state.json` via `mei-host-shell auth ensure-keys` and `mei-host-shell auth bootstrap-users` (or `add-user --password-stdin`)",
        );
    }
    if body
        .password
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return json_error(
            StatusCode::BAD_REQUEST,
            "plaintext password is not accepted; submit encryptedPassword only",
        );
    }
    let Some(encrypted) = body
        .encrypted_password
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return json_error(StatusCode::BAD_REQUEST, "encryptedPassword is required");
    };
    let password = match runtime.decrypt_password_field(encrypted) {
        Ok(value) => value,
        Err(error) => {
            return json_error(
                StatusCode::BAD_REQUEST,
                format!("failed to decrypt password: {error}"),
            )
        }
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

pub async fn auth_logout(State(state): State<AuthServeState>) -> impl IntoResponse {
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
    State(state): State<AuthServeState>,
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
            "auth is not configured; initialize `.mei/local/hosts/*.state.json` via `mei-host-shell auth ensure-keys` and `mei-host-shell auth bootstrap-users` (or `add-user --password-stdin`)",
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
