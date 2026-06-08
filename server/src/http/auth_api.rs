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

/// HTTP 明文登录仅允许受信主机（浏览器在 HTTP 非 localhost 下无 `crypto.subtle`）。
const HTTP_PLAINTEXT_LOGIN_HOSTS: &[&str] =
    &["localhost", "127.0.0.1", "[::1]", "23.211.135.152"];

fn host_only_from_header(host_header: &str) -> &str {
    let trimmed = host_header.trim();
    if let Some(inner) = trimmed.strip_prefix('[') {
        if let Some((host, _)) = inner.split_once(']') {
            return host;
        }
    }
    trimmed.split(':').next().unwrap_or(trimmed)
}

fn host_allows_http_plaintext_login(host_header: &str) -> bool {
    let host_only = host_only_from_header(host_header);
    HTTP_PLAINTEXT_LOGIN_HOSTS.iter().any(|allowed| {
        let allowed_host = allowed.trim_matches(|c| c == '[' || c == ']');
        host_only.eq_ignore_ascii_case(allowed_host)
    })
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

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const PASSWORD_TOGGLE_BUTTON: &str = r#"<button type="button" class="mei-host-shell__password-toggle" aria-label="显示密码" title="显示密码" data-password-target="__TARGET__">
        <svg class="icon-eye" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7Z" />
          <circle cx="12" cy="12" r="3" />
        </svg>
        <svg class="icon-eye-off" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M9.88 9.88a3 3 0 1 0 4.24 4.24" />
          <path d="M10.73 5.08A10.43 10.43 0 0 1 12 5c7 0 10 7 10 7a13.16 13.16 0 0 1-1.67 2.68" />
          <path d="M6.61 6.61A13.526 13.526 0 0 0 2 12s3 7 10 7a9.74 9.74 0 0 0 5.39-1.61" />
          <line x1="2" x2="22" y1="2" y2="22" />
        </svg>
      </button>"#;

const PASSWORD_TOGGLE_SCRIPT: &str = r#"
      document.querySelectorAll('.mei-host-shell__password-toggle').forEach((button) => {
        button.addEventListener('click', () => {
          const targetId = button.getAttribute('data-password-target');
          const input = targetId ? document.getElementById(targetId) : null;
          if (!input) return;
          const visible = input.type === 'text';
          input.type = visible ? 'password' : 'text';
          button.classList.toggle('is-visible', !visible);
          const label = visible ? '显示密码' : '隐藏密码';
          button.setAttribute('aria-label', label);
          button.setAttribute('title', label);
        });
      });"#;

fn password_field_html(
    id: &str,
    autocomplete: &str,
    include_name: bool,
    required: bool,
) -> String {
    let name_attr = if include_name {
        format!(r#" name="{id}""#)
    } else {
        String::new()
    };
    let required_attr = if required { " required" } else { "" };
    let toggle = PASSWORD_TOGGLE_BUTTON.replace("__TARGET__", id);
    format!(
        r#"<div class="mei-host-shell__password-field">
        <input id="{id}"{name_attr} type="password" autocomplete="{autocomplete}"{required_attr} />
        {toggle}
      </div>"#
    )
}

fn login_page_html(
    next: &str,
    auth_ready: bool,
    auth_configured: bool,
    footer_html: &str,
) -> String {
    let password_field = password_field_html("password", "current-password", true, true);
    let next_escaped = html_escape(next);
    let setup_notice = if !auth_ready {
        r#"<p class="mei-host-shell__setup">当前宿主未启用登录要求（调试模式）。</p>"#
    } else if !auth_configured {
        r#"<p class="mei-host-shell__setup">认证尚未配置用户。请在工作区根目录 <code>.mei-workspace.json</code> 的 <code>auth.users[]</code> 中写入 <code>passwordHash</code>（禁止明文密码），并执行 <code>mei host auth ensure-keys</code> + <code>mei host auth bootstrap-users</code>（或 <code>add-user --password-stdin</code>）。</p>"#
    } else {
        ""
    };
    let form_disabled = if auth_ready && auth_configured {
        ""
    } else {
        " disabled"
    };
    let password_toggle_script = PASSWORD_TOGGLE_SCRIPT;
    let card_inner = format!(
        r#"<p class="mei-host-shell__message" id="login-message">密码字段会使用宿主公钥加密后再提交。</p>
      {setup_notice}
      <form class="mei-host-shell__form" id="login-form">
        <label for="username">用户名</label>
        <input id="username" name="username" autocomplete="username" required />
        <label for="password">密码</label>
        {password_field}
        <input id="next" type="hidden" value="{next_escaped}" />
        <button type="submit"{form_disabled}>登录</button>
      </form>
      <div id="error" class="mei-host-shell__feedback mei-host-shell__feedback--error"></div>
    <script>
      const errorBox = document.getElementById('error');
      function clearError() {{ errorBox.textContent = ''; }}
      function setError(message) {{ errorBox.textContent = message || '登录失败'; }}
      function pemToArrayBuffer(pem) {{
        const body = pem.replace(/-----BEGIN PUBLIC KEY-----/g, '').replace(/-----END PUBLIC KEY-----/g, '').replace(/\s+/g, '');
        const binary = atob(body);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
        return bytes.buffer;
      }}
      async function encryptWithPem(publicKeyPem, text) {{
        const keyData = pemToArrayBuffer(publicKeyPem);
        const cryptoKey = await crypto.subtle.importKey(
          'spki',
          keyData,
          {{ name: 'RSA-OAEP', hash: 'SHA-256' }},
          false,
          ['encrypt']
        );
        const encoded = new TextEncoder().encode(text);
        const encrypted = await crypto.subtle.encrypt({{ name: 'RSA-OAEP' }}, cryptoKey, encoded);
        const bytes = new Uint8Array(encrypted);
        let bin = '';
        bytes.forEach((b) => {{ bin += String.fromCharCode(b); }});
        return btoa(bin);
      }}
      const canEncrypt = !!(window.crypto && window.crypto.subtle);
      const loginMessage = document.getElementById('login-message');
      if (!canEncrypt && loginMessage) {{
        loginMessage.textContent = '当前为 HTTP 访问，浏览器无法使用 Web Crypto；将在受信主机（localhost 或 23.211.135.152）上直接提交密码。建议生产环境使用 HTTPS。';
      }}
      async function resolvePublicKey() {{
        const resp = await fetch('/api/auth/public-key', {{ credentials: 'same-origin' }});
        const data = await resp.json();
        if (!resp.ok || !data.public_key_pem) {{
          throw new Error(data.error || '获取公钥失败');
        }}
        return data.public_key_pem;
      }}
      document.getElementById('login-form').addEventListener('submit', async (event) => {{
        event.preventDefault();
        clearError();
        try {{
          const username = document.getElementById('username').value.trim();
          const password = document.getElementById('password').value;
          if (!username || !password) {{
            setError('请输入用户名和密码');
            return;
          }}
          const next = document.getElementById('next').value || '/';
          let body;
          if (canEncrypt) {{
            const publicKeyPem = await resolvePublicKey();
            const encryptedPassword = await encryptWithPem(publicKeyPem, password);
            body = {{ username, encryptedPassword, next }};
          }} else {{
            body = {{ username, password, next }};
          }}
          const resp = await fetch('/api/auth/login', {{
            method: 'POST',
            credentials: 'same-origin',
            headers: {{ 'content-type': 'application/json' }},
            body: JSON.stringify(body)
          }});
          const data = await resp.json();
          if (!resp.ok) {{
            setError(data.error || '登录失败');
            return;
          }}
          window.location.href = data.next || '/';
        }} catch (error) {{
          setError(error && error.message ? error.message : '登录失败');
        }}
      }});
      {password_toggle_script}
    </script>"#
    );
    host_error_page::render_auth_card_page(
        "登录 - MeiLang",
        "MeiLang 登录",
        card_inner.as_str(),
        footer_html,
    )
}

fn change_password_page_html(username: &str, role: &str, footer_html: &str) -> String {
    let user = html_escape(username);
    let role = html_escape(role);
    let current_password_field = password_field_html("current-password", "current-password", false, true);
    let new_password_field = password_field_html("new-password", "new-password", false, true);
    let confirm_password_field =
        password_field_html("confirm-password", "new-password", false, true);
    let password_toggle_script = PASSWORD_TOGGLE_SCRIPT;
    let card_inner = format!(
        r#"<div class="mei-host-shell__meta">当前账户：{user}（{role}）</div>
      <form class="mei-host-shell__form" id="change-password-form">
        <label for="current-password">当前密码</label>
        {current_password_field}
        <label for="new-password">新密码</label>
        {new_password_field}
        <label for="confirm-password">确认新密码</label>
        {confirm_password_field}
        <button type="submit">确认修改</button>
      </form>
      <div id="error" class="mei-host-shell__feedback mei-host-shell__feedback--error"></div>
      <div id="ok" class="mei-host-shell__feedback mei-host-shell__feedback--ok"></div>
      <a class="mei-host-shell__link" href="/">返回首页</a>
    <script>
      const errorBox = document.getElementById('error');
      const okBox = document.getElementById('ok');
      function setError(message) {{ errorBox.textContent = message || '修改失败'; okBox.textContent = ''; }}
      function setOk(message) {{ okBox.textContent = message || '修改成功'; errorBox.textContent = ''; }}
      function pemToArrayBuffer(pem) {{
        const body = pem.replace(/-----BEGIN PUBLIC KEY-----/g, '').replace(/-----END PUBLIC KEY-----/g, '').replace(/\s+/g, '');
        const binary = atob(body);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
        return bytes.buffer;
      }}
      async function encryptWithPem(publicKeyPem, text) {{
        const keyData = pemToArrayBuffer(publicKeyPem);
        const cryptoKey = await crypto.subtle.importKey(
          'spki',
          keyData,
          {{ name: 'RSA-OAEP', hash: 'SHA-256' }},
          false,
          ['encrypt']
        );
        const encoded = new TextEncoder().encode(text);
        const encrypted = await crypto.subtle.encrypt({{ name: 'RSA-OAEP' }}, cryptoKey, encoded);
        const bytes = new Uint8Array(encrypted);
        let bin = '';
        bytes.forEach((b) => {{ bin += String.fromCharCode(b); }});
        return btoa(bin);
      }}
      async function resolvePublicKey() {{
        const resp = await fetch('/api/auth/public-key', {{ credentials: 'same-origin' }});
        const data = await resp.json();
        if (!resp.ok || !data.public_key_pem) {{
          throw new Error(data.error || '获取公钥失败');
        }}
        return data.public_key_pem;
      }}
      document.getElementById('change-password-form').addEventListener('submit', async (event) => {{
        event.preventDefault();
        setError('');
        setOk('');
        const currentPassword = document.getElementById('current-password').value;
        const newPassword = document.getElementById('new-password').value;
        const confirmPassword = document.getElementById('confirm-password').value;
        if (!currentPassword || !newPassword) {{
          setError('请填写完整密码信息');
          return;
        }}
        if (newPassword !== confirmPassword) {{
          setError('两次输入的新密码不一致');
          return;
        }}
        try {{
          const publicKeyPem = await resolvePublicKey();
          const encryptedCurrentPassword = await encryptWithPem(publicKeyPem, currentPassword);
          const encryptedNewPassword = await encryptWithPem(publicKeyPem, newPassword);
          const resp = await fetch('/api/auth/change-password', {{
            method: 'POST',
            credentials: 'same-origin',
            headers: {{ 'content-type': 'application/json' }},
            body: JSON.stringify({{ encryptedCurrentPassword, encryptedNewPassword }})
          }});
          const data = await resp.json();
          if (!resp.ok) {{
            setError(data.error || '修改失败');
            return;
          }}
          setOk('密码修改成功，已刷新登录态');
          document.getElementById('current-password').value = '';
          document.getElementById('new-password').value = '';
          document.getElementById('confirm-password').value = '';
        }} catch (error) {{
          setError(error && error.message ? error.message : '修改失败');
        }}
      }});
      {password_toggle_script}
    </script>"#
    );
    host_error_page::render_auth_card_page(
        "修改密码 - MeiLang",
        "修改密码",
        card_inner.as_str(),
        footer_html,
    )
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
            return Redirect::temporary(&authorize_next_path(
                query.next.as_deref(),
                &principal,
            ))
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
        response.headers_mut().insert(header::SET_COOKIE, header_value);
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
        return json_error(StatusCode::SERVICE_UNAVAILABLE, "auth keypair is not configured");
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
    let Some(new_claims) = (match refreshed.authenticate(principal.username.as_str(), new_password.as_str()) {
        Ok(value) => value,
        Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }) else {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, "failed to re-issue session");
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

#[cfg(test)]
mod tests {
    use super::host_allows_http_plaintext_login;

    #[test]
    fn plaintext_login_host_allowlist() {
        assert!(host_allows_http_plaintext_login("localhost"));
        assert!(host_allows_http_plaintext_login("localhost:3002"));
        assert!(host_allows_http_plaintext_login("127.0.0.1:9527"));
        assert!(host_allows_http_plaintext_login("[::1]:3002"));
        assert!(host_allows_http_plaintext_login("23.211.135.152:3002"));
        assert!(!host_allows_http_plaintext_login("zw-spbjw:3002"));
        assert!(!host_allows_http_plaintext_login("example.com"));
    }
}
