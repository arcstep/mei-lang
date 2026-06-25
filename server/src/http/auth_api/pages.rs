use crate::http::host_error_page;

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

const AUTH_RSA_BUNDLE_SCRIPT: &str =
    r#"<script src="/app-assets/dist/auth-rsa.bundle.js"></script>"#;

const AUTH_RSA_CLIENT_SCRIPT: &str = r#"
      function encryptWithPem(publicKeyPem, text) {
        const rsa = window.MeiAuthRsa;
        if (!rsa || typeof rsa.encryptPasswordWithPem !== 'function') {
          throw new Error('密码加密模块未加载');
        }
        return rsa.encryptPasswordWithPem(publicKeyPem, text);
      }
      async function resolvePublicKey() {
        const resp = await fetch('/api/auth/public-key', { credentials: 'same-origin' });
        const data = await resp.json();
        if (!resp.ok || !data.public_key_pem) {
          throw new Error(data.error || '获取公钥失败');
        }
        return data.public_key_pem;
      }"#;

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

fn password_field_html(id: &str, autocomplete: &str, include_name: bool, required: bool) -> String {
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

pub(super) fn login_page_html(
    next: &str,
    auth_ready: bool,
    auth_configured: bool,
    footer_html: &str,
    body_theme_style: &str,
) -> String {
    let password_field = password_field_html("password", "current-password", true, true);
    let next_escaped = html_escape(next);
    let setup_notice = if !auth_ready {
        r#"<p class="mei-host-shell__setup">当前宿主未启用登录要求（调试模式）。</p>"#
    } else if !auth_configured {
        r#"<p class="mei-host-shell__setup">认证尚未配置用户。请执行 <code>mei host auth ensure-keys</code> + <code>mei host auth bootstrap-users</code>（或 <code>add-user --password-stdin</code>）。宿主认证状态默认写入 <code>.mei/local/hosts/*.state.json</code>，禁止在 <code>.mei-workspace.json</code> 中保留密钥或密码哈希。</p>"#
    } else {
        ""
    };
    let form_disabled = if auth_ready && auth_configured {
        ""
    } else {
        " disabled"
    };
    let password_toggle_script = PASSWORD_TOGGLE_SCRIPT;
    let auth_rsa_bundle_script = AUTH_RSA_BUNDLE_SCRIPT;
    let auth_rsa_client_script = AUTH_RSA_CLIENT_SCRIPT;
    let card_inner = format!(
        r#"<p class="mei-host-shell__message" id="login-message">密码内容将使用服务器公钥 RSA-OAEP 加密后再提交，确保安全。</p>
      {setup_notice}
      <form class="mei-host-shell__form" id="login-form">
        <label for="username">用户名</label>
        <input id="username" name="username" autocomplete="username" required />
        <label for="password">密码</label>
        {password_field}
        <input id="next" type="hidden" value="{next_escaped}" />
        <button type="submit" id="login-submit"{form_disabled}>登录</button>
      </form>
      <div id="status" class="mei-host-shell__feedback mei-host-shell__feedback--pending" aria-live="polite"></div>
      <div id="error" class="mei-host-shell__feedback mei-host-shell__feedback--error"></div>
    {auth_rsa_bundle_script}
    <script>
      const errorBox = document.getElementById('error');
      const statusBox = document.getElementById('status');
      const loginForm = document.getElementById('login-form');
      const submitButton = document.getElementById('login-submit');
      const usernameInput = document.getElementById('username');
      const passwordInput = document.getElementById('password');
      const submitLabel = submitButton.textContent || '登录';
      const formDisabled = submitButton.disabled;
      function clearError() {{ errorBox.textContent = ''; }}
      function setError(message) {{
        errorBox.textContent = message || '登录失败';
        statusBox.textContent = '';
      }}
      function setLoginPending(pending, message) {{
        loginForm.classList.toggle('is-busy', pending);
        loginForm.setAttribute('aria-busy', pending ? 'true' : 'false');
        submitButton.disabled = pending || formDisabled;
        usernameInput.disabled = pending || formDisabled;
        passwordInput.disabled = pending || formDisabled;
        submitButton.textContent = pending ? '正在登录…' : submitLabel;
        statusBox.textContent = pending ? (message || '正在登录，请稍候…') : '';
        if (pending) {{
          clearError();
        }}
      }}
      {auth_rsa_client_script}
      document.getElementById('login-form').addEventListener('submit', async (event) => {{
        event.preventDefault();
        clearError();
        if (formDisabled) {{
          return;
        }}
        setLoginPending(true, '正在准备安全连接…');
        try {{
          const username = usernameInput.value.trim();
          const password = passwordInput.value;
          if (!username || !password) {{
            setLoginPending(false);
            setError('请输入用户名和密码');
            return;
          }}
          const next = document.getElementById('next').value || '/';
          setLoginPending(true, '正在获取加密公钥…');
          const publicKeyPem = await resolvePublicKey();
          setLoginPending(true, '正在验证账户…');
          const encryptedPassword = encryptWithPem(publicKeyPem, password);
          const body = {{ username, encryptedPassword, next }};
          const resp = await fetch('/api/auth/login', {{
            method: 'POST',
            credentials: 'same-origin',
            headers: {{ 'content-type': 'application/json' }},
            body: JSON.stringify(body)
          }});
          const data = await resp.json();
          if (!resp.ok) {{
            setLoginPending(false);
            setError(data.error || '登录失败');
            return;
          }}
          setLoginPending(true, '登录成功，正在跳转…');
          window.location.href = data.next || '/';
        }} catch (error) {{
          setLoginPending(false);
          setError(error && error.message ? error.message : '登录失败');
        }}
      }});
      {password_toggle_script}
    </script>"#,
        auth_rsa_bundle_script = auth_rsa_bundle_script,
        auth_rsa_client_script = auth_rsa_client_script,
    );
    host_error_page::render_auth_card_page(
        "登录 - MeiLang",
        "MeiLang 登录",
        card_inner.as_str(),
        footer_html,
        body_theme_style,
    )
}

pub(super) fn change_password_page_html(
    username: &str,
    role: &str,
    footer_html: &str,
    body_theme_style: &str,
) -> String {
    let user = html_escape(username);
    let role = html_escape(role);
    let current_password_field =
        password_field_html("current-password", "current-password", false, true);
    let new_password_field = password_field_html("new-password", "new-password", false, true);
    let confirm_password_field =
        password_field_html("confirm-password", "new-password", false, true);
    let password_toggle_script = PASSWORD_TOGGLE_SCRIPT;
    let auth_rsa_bundle_script = AUTH_RSA_BUNDLE_SCRIPT;
    let auth_rsa_client_script = AUTH_RSA_CLIENT_SCRIPT;
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
    {auth_rsa_bundle_script}
    <script>
      const errorBox = document.getElementById('error');
      const okBox = document.getElementById('ok');
      function setError(message) {{ errorBox.textContent = message || '修改失败'; okBox.textContent = ''; }}
      function setOk(message) {{ okBox.textContent = message || '修改成功'; errorBox.textContent = ''; }}
      {auth_rsa_client_script}
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
    </script>"#,
        auth_rsa_bundle_script = auth_rsa_bundle_script,
        auth_rsa_client_script = auth_rsa_client_script,
    );
    host_error_page::render_auth_card_page(
        "修改密码 - MeiLang",
        "修改密码",
        card_inner.as_str(),
        footer_html,
        body_theme_style,
    )
}
