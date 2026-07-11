use std::path::Path;

use anyhow::Result;
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use mei_lang_kernel::{load_workspace_auth_bundle, WorkspaceAuthBundle};
use serde_json::json;

use crate::crypto::extract_token_from_headers;
use crate::runtime::{load_auth_runtime, normalize_id};
use crate::shell_chrome;
use crate::state::AuthServeState;
use crate::types::{AuthEnforcement, AuthPrincipal, AuthRuntime};
use crate::workspace_users::ensure_workspace_auth_base;

fn is_public_path(path: &str) -> bool {
    path == "/login"
        || path == "/logout"
        || path == "/favicon.ico"
        || path.starts_with("/app-assets/")
        || path.starts_with("/app-bundles/")
        || path.starts_with("/workspace-components/bundles/")
        || path == "/gis"
        || path.starts_with("/gis/")
        || path.starts_with("/workspace-components/vendor/")
        || path == "/api/host/ready"
        || path == "/api/host/heartbeat"
        || path == "/api/host/readiness"
        || path == "/api/host/access-readiness"
        || path == "/api/host/version"
        || path == "/host/starting"
        || path == "/host"
        || path == "/host/config"
        || path == "/host/upload"
        || path == "/host/runtime"
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
    let (app_id, scene_id) = if matches!(
        mode.as_str(),
        "app" | "access" | "access-only" | "presentation" | "slides"
    ) {
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
        "/api/presentation/map/",
        "/api/presentation/scripts/",
        "/api/ops/",
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

fn is_super_only_agent_path(path: &str) -> bool {
    path == "/api/agent/start"
        || path == "/api/agent/stop"
        || path.starts_with("/api/agent/skill/sync")
}

fn is_authoring_agent_path(path: &str) -> bool {
    path.starts_with("/api/agent/session/")
        && (path.ends_with("/diff") || path.ends_with("/revert") || path.ends_with("/unrevert"))
}

fn authorize_agent_path(path: &str, caps: &mei_lang_app::HostCapabilities) -> Result<()> {
    if is_super_only_agent_path(path) {
        if !caps.agent_control {
            anyhow::bail!("current role cannot access agent control api");
        }
        return Ok(());
    }
    if is_authoring_agent_path(path) {
        if !caps.authoring_agent {
            anyhow::bail!("current role cannot access authoring agent api");
        }
        return Ok(());
    }
    if !caps.access_agent {
        anyhow::bail!("current role cannot access access agent api");
    }
    Ok(())
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
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": message,
                "status": StatusCode::FORBIDDEN.as_u16(),
            })),
        )
            .into_response();
    }
    shell_chrome::forbidden_html_response(message)
}

pub fn sanitize_next_path(next: Option<&str>) -> String {
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

pub fn authorize_next_path(next: Option<&str>, principal: &AuthPrincipal) -> String {
    let next = sanitize_next_path(next);
    if next == "/" {
        return next;
    }
    let path = next.split('?').next().unwrap_or(next.as_str());
    if authorize_path(path, principal).is_ok() {
        next
    } else {
        "/".to_string()
    }
}

pub fn authorize_path(path: &str, principal: &AuthPrincipal) -> Result<()> {
    let caps = principal.capabilities();
    if let Some(host_mode) = match path {
        "/" | "/host" => Some("home"),
        "/host/config" => Some("config"),
        "/host/upload" => Some("upload"),
        "/host/runtime" => Some("runtime"),
        _ => None,
    } {
        let allowed = match host_mode {
            "home" => true,
            "config" | "upload" => caps.config_upload,
            "runtime" => caps.build_view,
            _ => false,
        };
        if !allowed {
            anyhow::bail!("current role cannot access host `{host_mode}` route");
        }
        return Ok(());
    }
    if let Some((mode, app_id, scene_id)) = extract_app_route_context(path) {
        if !principal.can_access_app(app_id.as_str()) {
            anyhow::bail!("app `{app_id}` is not in guest allowlist");
        }
        if let Some(scene_id) = scene_id.as_deref() {
            if !principal.can_access_scene(app_id.as_str(), scene_id) {
                anyhow::bail!("scene `{scene_id}` is not in guest allowlist");
            }
        }
        let route_allowed = match mode.as_str() {
            "app" | "access" | "access-only" | "run" | "presentation" | "slides" | "copilot"
            | "speaker" => caps.access_view,
            "upload" | "config" => caps.config_upload,
            "build" | "manage" | "runtime" => caps.build_view,
            _ => false,
        };
        if !route_allowed {
            anyhow::bail!("current role cannot access `{mode}` routes");
        }
        return Ok(());
    }
    if let Some(app_id) = extract_api_app_id(path) {
        if !principal.can_access_app(app_id.as_str()) {
            anyhow::bail!("app `{app_id}` is not in guest allowlist");
        }
    }
    if path.starts_with("/api/upload/download/") {
        if !caps.access_view {
            anyhow::bail!("current role cannot access upload media");
        }
        return Ok(());
    }
    if path.starts_with("/api/ops/theme/") {
        if !caps.access_view {
            anyhow::bail!("current role cannot access theme api");
        }
        return Ok(());
    }
    if path.starts_with("/api/ops/themes/layout/overlay/") {
        if !(caps.build_view || caps.access_view) {
            anyhow::bail!("current role cannot access theme layout overlay api");
        }
        return Ok(());
    }
    if path.starts_with("/api/ops/themes/layout/apply/") {
        if !caps.build_view {
            anyhow::bail!("current role cannot apply theme layout draft");
        }
        return Ok(());
    }
    if path.starts_with("/api/ops/boundary") || path.starts_with("/api/ops/journal/") {
        if !caps.access_view {
            anyhow::bail!("current role cannot access ops read api");
        }
        return Ok(());
    }
    if path.starts_with("/api/ops/config/") {
        if !caps.config_upload {
            anyhow::bail!("current role cannot access ops config api");
        }
        return Ok(());
    }
    if path.starts_with("/api/ops/") || path.starts_with("/api/upload/") {
        if !caps.config_upload {
            anyhow::bail!("current role cannot access write api");
        }
    }
    if path.starts_with("/api/agent/") {
        return authorize_agent_path(path, &caps);
    }
    if path == "/api/presentation/compile" {
        if !caps.access_view {
            anyhow::bail!("current role cannot access presentation compile api");
        }
        return Ok(());
    }
    if path.starts_with("/workspace-components/") {
        if !caps.runtime_components {
            anyhow::bail!("current role cannot access component assets");
        }
        return Ok(());
    }
    Ok(())
}

fn format_auth_not_ready_message(
    source_root: &Path,
    bundle: &WorkspaceAuthBundle,
    runtime: &AuthRuntime,
    cli_hint: &str,
) -> String {
    let root = source_root.display();
    let config = runtime.config_path.display();
    let auth = &bundle.auth;
    let configured_user_count = auth.users.len();
    let has_jwt_secret = auth.jwt_secret.as_deref().unwrap_or("").trim().is_empty() == false;
    let has_public_key = !auth.key_pair.public_key_pem.trim().is_empty();
    let has_private_key = !auth.key_pair.private_key_pem.trim().is_empty();
    let keys_ready = has_jwt_secret && has_public_key && has_private_key;
    let active_user_count = runtime.user_count();

    let mut lines = vec![
        "已启用 --auth，但工作区认证尚未就绪，无法启动。".to_string(),
        format!("认证状态文件：{config}"),
        String::new(),
    ];

    if !keys_ready {
        let mut missing = Vec::new();
        if !has_jwt_secret {
            missing.push("jwtSecret");
        }
        if !has_public_key || !has_private_key {
            missing.push("RSA keyPair");
        }
        lines.push(format!("缺少密钥：{}。", missing.join("、")));
        lines.push("请先执行：".to_string());
        lines.push(format!("  {cli_hint} auth ensure-keys --workspace {root}"));
        lines.push(String::new());
    }

    if active_user_count == 0 {
        if configured_user_count == 0 {
            lines.push("尚未配置登录用户（auth.users 为空）。".to_string());
        } else {
            lines.push(format!(
                "已写入 {configured_user_count} 个用户条目，但无一可用（可能 passwordHash 无效、为空，或账号被禁用）。"
            ));
            lines.push(format!(
                "请检查 `{config}` 中各用户的 passwordHash（禁止明文密码）。"
            ));
        }
        lines.push(String::new());
        lines.push(
            "初始化 super / admin / guest（推荐，密码从 stdin 读取，勿写在命令行）：".to_string(),
        );
        lines.push(format!(
            "  printf '%s' 'YourPwd1!complex' | {cli_hint} auth bootstrap-users --workspace {root} --default-password-stdin"
        ));
        lines.push("或生成随机临时密码（仅当次输出，适合首次部署）：".to_string());
        lines.push(format!(
            "  {cli_hint} auth bootstrap-users --workspace {root} --json"
        ));
        lines.push("仅新增单个用户：".to_string());
        lines.push(format!(
            "  printf '%s' 'YourPwd1!complex' | {cli_hint} auth add-user --workspace {root} --username guest01 --role guest --password-stdin"
        ));
        lines.push(String::new());
        lines.push("密码规则：至少 8 位，且须含大写 / 小写 / 数字 / 符号。".to_string());
    }

    lines.push(String::new());
    lines.push(format!(
        "完成后重新启动：{cli_hint} serve --auth --workspace {root}"
    ));
    if keys_ready && active_user_count > 0 {
        lines
            .push("（若仍失败，请检查上述用户 passwordHash 是否为有效 Argon2 哈希。）".to_string());
    }

    lines.join("\n")
}

pub fn prepare_auth_for_serve(
    source_root: &Path,
    enforcement: AuthEnforcement,
    cli_hint: &str,
) -> Result<()> {
    if enforcement != AuthEnforcement::Required {
        return Ok(());
    }
    let _ = ensure_workspace_auth_base(source_root)?;
    let bundle = load_workspace_auth_bundle(source_root);
    let runtime = load_auth_runtime(source_root)?;
    if !runtime.enabled {
        anyhow::bail!(
            "{}",
            format_auth_not_ready_message(source_root, &bundle, &runtime, cli_hint)
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
    State(state): State<AuthServeState>,
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

    if is_public_path(path.as_str()) {
        if let Some(principal) = principal {
            request.extensions_mut().insert(principal);
        }
        return next.run(request).await;
    }

    if let Some(ref principal) = principal {
        if let Err(error) = authorize_path(&path, principal) {
            return forbidden_response(path.as_str(), &error.to_string());
        }
    }

    let Some(principal) = principal else {
        return unauthorized_response(path.as_str(), request.uri());
    };
    request.extensions_mut().insert(principal);
    next.run(request).await
}
