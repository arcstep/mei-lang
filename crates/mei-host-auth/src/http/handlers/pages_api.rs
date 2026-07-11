use axum::{
    extract::{Extension, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect},
    Json,
};
use serde_json::json;

use crate::http::pages::{change_password_page_html, login_page_html};
use crate::shell_chrome;
use crate::state::AuthServeState;
use crate::{
    authorize_next_path, clear_cookie_header_value, load_auth_runtime, sanitize_next_path,
    AuthPrincipal,
};

use super::support::*;

pub async fn login_page(
    State(state): State<AuthServeState>,
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
        shell_chrome::render_host_shell_footer_for_source_root(state.source_root.as_path());
    let shell_theme = shell_chrome::host_shell_body_theme_style(state.source_root.as_path());
    Html(login_page_html(
        sanitize_next_path(query.next.as_deref()).as_str(),
        auth_login_ready(&state, &runtime),
        runtime.enabled,
        footer_html.as_str(),
        shell_theme.as_str(),
    ))
    .into_response()
}

pub async fn logout_page(
    State(state): State<AuthServeState>,
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
    State(state): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> impl IntoResponse {
    if let Some(response) = reject_page_if_auth_disabled(&state) {
        return response;
    }
    let Some(Extension(principal)) = principal else {
        return Redirect::temporary("/login").into_response();
    };
    let footer_html =
        shell_chrome::render_host_shell_footer_for_source_root(state.source_root.as_path());
    let shell_theme = shell_chrome::host_shell_body_theme_style(state.source_root.as_path());
    Html(change_password_page_html(
        principal.username.as_str(),
        principal.role_slug(),
        footer_html.as_str(),
        shell_theme.as_str(),
    ))
    .into_response()
}

pub async fn auth_public_key(State(state): State<AuthServeState>) -> impl IntoResponse {
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
