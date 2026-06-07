use axum::{
    extract::{Extension, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    auth::{
        clear_cookie_header_value, cookie_header_value, hash_password, load_auth_runtime,
        update_workspace_user_password, AuthEnforcement, AuthPrincipal,
    },
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

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    username: String,
    #[serde(rename = "encryptedPassword")]
    encrypted_password: String,
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

fn sanitize_next(next: Option<&str>) -> String {
    let raw = next.unwrap_or("/").trim();
    if raw.is_empty() {
        return "/".to_string();
    }
    if raw.starts_with('/') && !raw.starts_with("//") {
        raw.to_string()
    } else {
        "/".to_string()
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn login_page_html(next: &str, auth_ready: bool, auth_configured: bool) -> String {
    let next_escaped = html_escape(next);
    let setup_notice = if !auth_ready {
        r#"<p class="setup">当前宿主未启用登录要求（调试模式）。</p>"#
    } else if !auth_configured {
        r#"<p class="setup">认证尚未配置用户。请在工作区根目录 <code>.mei-workspace.json</code> 的 <code>auth.users[]</code> 中写入 <code>passwordHash</code>（禁止明文密码），并执行 <code>mei host auth ensure-keys</code> + <code>mei host auth bootstrap-users</code>（或 <code>add-user --password-stdin</code>）。</p>"#
    } else {
        ""
    };
    let form_disabled = if auth_ready && auth_configured {
        ""
    } else {
        " disabled"
    };
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width,initial-scale=1" />
    <title>登录 - MeiLang</title>
    <link rel="icon" href="/app-assets/favicon.svg" type="image/svg+xml" />
    <style>
      body {{
        margin: 0;
        min-height: 100vh;
        display: grid;
        place-items: center;
        background: #0b1220;
        color: #e2e8f0;
        font-family: -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;
      }}
      .card {{
        width: min(92vw, 420px);
        padding: 24px;
        border-radius: 14px;
        background: #111a2c;
        border: 1px solid #22304a;
      }}
      h1 {{ margin: 0 0 14px; font-size: 20px; }}
      p {{ margin: 0 0 14px; color: #94a3b8; font-size: 13px; line-height: 1.5; }}
      label {{ display: block; margin: 10px 0 6px; font-size: 13px; color: #cbd5e1; }}
      input {{
        width: 100%;
        box-sizing: border-box;
        border-radius: 8px;
        border: 1px solid #334155;
        background: #0f172a;
        color: #e2e8f0;
        padding: 10px;
        font-size: 14px;
      }}
      button {{
        margin-top: 14px;
        width: 100%;
        border: none;
        border-radius: 8px;
        background: #0ea5e9;
        color: #041320;
        font-size: 14px;
        font-weight: 600;
        padding: 10px;
        cursor: pointer;
      }}
      #error {{ min-height: 20px; color: #fda4af; margin-top: 10px; font-size: 12px; }}
      .setup {{ color: #fbbf24; font-size: 12px; line-height: 1.6; margin-bottom: 12px; }}
      .setup code {{ color: #fde68a; }}
    </style>
  </head>
  <body>
    <main class="card">
      <h1>MeiLang 登录</h1>
      <p>密码字段会使用宿主公钥加密后再提交。</p>
      {setup_notice}
      <form id="login-form">
        <label for="username">用户名</label>
        <input id="username" name="username" autocomplete="username" required />
        <label for="password">密码</label>
        <input id="password" name="password" type="password" autocomplete="current-password" required />
        <input id="next" type="hidden" value="{next_escaped}" />
        <button type="submit"{form_disabled}>登录</button>
      </form>
      <div id="error"></div>
    </main>
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
          const publicKeyPem = await resolvePublicKey();
          const encryptedPassword = await encryptWithPem(publicKeyPem, password);
          const next = document.getElementById('next').value || '/';
          const resp = await fetch('/api/auth/login', {{
            method: 'POST',
            credentials: 'same-origin',
            headers: {{ 'content-type': 'application/json' }},
            body: JSON.stringify({{ username, encryptedPassword, next }})
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
    </script>
  </body>
</html>"#
    )
}

fn change_password_page_html(username: &str, role: &str) -> String {
    let user = html_escape(username);
    let role = html_escape(role);
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width,initial-scale=1" />
    <title>修改密码 - MeiLang</title>
    <style>
      body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; background: #0b1220; color: #e2e8f0; font-family: -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }}
      .card {{ width: min(92vw, 460px); padding: 24px; border-radius: 14px; background: #111a2c; border: 1px solid #22304a; }}
      h1 {{ margin: 0 0 6px; font-size: 20px; }}
      .meta {{ color: #94a3b8; margin-bottom: 14px; font-size: 12px; }}
      label {{ display: block; margin: 10px 0 6px; font-size: 13px; color: #cbd5e1; }}
      input {{ width: 100%; box-sizing: border-box; border-radius: 8px; border: 1px solid #334155; background: #0f172a; color: #e2e8f0; padding: 10px; font-size: 14px; }}
      button {{ margin-top: 14px; width: 100%; border: none; border-radius: 8px; background: #22c55e; color: #052312; font-size: 14px; font-weight: 600; padding: 10px; cursor: pointer; }}
      #error {{ min-height: 20px; color: #fda4af; margin-top: 10px; font-size: 12px; }}
      #ok {{ min-height: 20px; color: #86efac; margin-top: 8px; font-size: 12px; }}
      .back {{ display: inline-block; margin-top: 12px; color: #7dd3fc; font-size: 12px; }}
    </style>
  </head>
  <body>
    <main class="card">
      <h1>修改密码</h1>
      <div class="meta">当前账户：{user}（{role}）</div>
      <form id="change-password-form">
        <label for="current-password">当前密码</label>
        <input id="current-password" type="password" autocomplete="current-password" required />
        <label for="new-password">新密码</label>
        <input id="new-password" type="password" autocomplete="new-password" required />
        <label for="confirm-password">确认新密码</label>
        <input id="confirm-password" type="password" autocomplete="new-password" required />
        <button type="submit">确认修改</button>
      </form>
      <div id="error"></div>
      <div id="ok"></div>
      <a class="back" href="/">返回应用</a>
    </main>
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
    </script>
  </body>
</html>"#
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
    if principal.is_some() && runtime.enabled {
        return Redirect::temporary(&sanitize_next(query.next.as_deref())).into_response();
    }
    Html(login_page_html(
        sanitize_next(query.next.as_deref()).as_str(),
        auth_login_ready(&state, &runtime),
        runtime.enabled,
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
    let mut response = Redirect::temporary(&sanitize_next(query.next.as_deref())).into_response();
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
    Html(change_password_page_html(
        principal.username.as_str(),
        principal.role_slug(),
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
    let password = match runtime.decrypt_password_field(body.encrypted_password.as_str()) {
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
    let mut response = (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "next": sanitize_next(body.next.as_deref()),
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
