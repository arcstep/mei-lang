//! Login / error shell chrome shared by auth pages and middleware.

use std::path::Path;

use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use mei_lang_app::shell_body_theme_style;
use mei_lang_kernel::{load_workspace_config, WorkspaceComplianceConfig, WorkspaceConfig};

pub const MEI_COIN_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" role="img" aria-hidden="true"><path fill="currentColor" fill-rule="evenodd" d="M16.000 1.400L17.255 1.656L18.402 2.378L19.366 3.437L20.130 4.652L20.740 5.834L21.294 6.830L21.913 7.556L22.704 8.010L23.728 8.272L24.974 8.470L26.352 8.751L27.713 9.238L28.880 9.994L29.689 11.018L30.032 12.240L29.883 13.552L29.308 14.836L28.450 16.000L27.497 17.006L26.639 17.876L26.020 18.685L25.710 19.534L25.687 20.517L25.851 21.687L26.045 23.034L26.101 24.475L25.878 25.878L25.301 27.085L24.370 27.953L23.156 28.395L21.783 28.401L20.386 28.050L19.078 27.488L17.922 26.900L16.915 26.461L16.000 26.300L15.085 26.461L14.078 26.900L12.922 27.488L11.614 28.050L10.217 28.401L8.844 28.395L7.630 27.953L6.699 27.085L6.122 25.878L5.899 24.475L5.955 23.034L6.149 21.688L6.313 20.517L6.290 19.534L5.980 18.685L5.361 17.876L4.503 17.006L3.550 16.000L2.692 14.836L2.117 13.552L1.968 12.240L2.311 11.018L3.120 9.994L4.287 9.238L5.648 8.751L7.026 8.470L8.272 8.272L9.296 8.010L10.087 7.556L10.706 6.830L11.260 5.834L11.870 4.652L12.634 3.437L13.598 2.378L14.745 1.656L16.000 1.400ZM12.9 11.75H19.1A1.15 1.15 0 0 1 20.25 12.9V19.1A1.15 1.15 0 0 1 19.1 20.25H12.9A1.15 1.15 0 0 1 11.75 19.1V12.9A1.15 1.15 0 0 1 12.9 11.75Z"/></svg>"#;

#[derive(Debug, Clone)]
pub struct HostShellFooterInfo {
    pub version_label: String,
    pub workspace_label: Option<String>,
    pub compliance: WorkspaceComplianceConfig,
}

impl HostShellFooterInfo {
    pub fn from_workspace(source_root: &Path, cfg: &WorkspaceConfig) -> Self {
        Self {
            version_label: mei_lang_kernel::resolve_build_footer_label(source_root),
            workspace_label: cfg.workspace.label.clone(),
            compliance: cfg.compliance.clone(),
        }
    }
}

pub fn footer_info_from_source_root(source_root: &Path) -> HostShellFooterInfo {
    let cfg = load_workspace_config(source_root);
    HostShellFooterInfo::from_workspace(source_root, &cfg)
}

pub fn render_host_shell_footer_for_source_root(source_root: &Path) -> String {
    render_host_shell_footer(&footer_info_from_source_root(source_root))
}

pub fn render_host_shell_footer(info: &HostShellFooterInfo) -> String {
    let mut headline_parts = Vec::new();
    if let Some(label) = info
        .workspace_label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        headline_parts.push(html_escape(label));
    }
    headline_parts.push(html_escape(info.version_label.as_str()));
    let mut lines = vec![format!(
        r#"<div class="mei-host-shell__footer-line">{}</div>"#,
        headline_parts.join(" · ")
    )];
    let mut compliance_parts = Vec::new();
    if let Some(value) = info.compliance.icp_record_trimmed() {
        compliance_parts.push(html_escape(value));
    }
    if let Some(value) = info.compliance.psb_record_trimmed() {
        compliance_parts.push(html_escape(value));
    }
    if let Some(value) = info.compliance.copyright_trimmed() {
        compliance_parts.push(html_escape(value));
    }
    if !compliance_parts.is_empty() {
        lines.push(format!(
            r#"<div class="mei-host-shell__footer-line">{}</div>"#,
            compliance_parts.join(" · ")
        ));
    }
    format!(
        r#"<footer class="mei-host-shell__footer" role="contentinfo">{lines}</footer>"#,
        lines = lines.join("")
    )
}

#[derive(Debug, Clone)]
pub struct HostShellAction {
    pub href: String,
    pub label: String,
    pub primary: bool,
}

pub fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_actions(actions: &[HostShellAction]) -> String {
    if actions.is_empty() {
        return String::new();
    }
    let buttons = actions
        .iter()
        .map(|action| {
            let class = if action.primary {
                "mei-host-shell__btn mei-host-shell__btn--primary"
            } else {
                "mei-host-shell__btn"
            };
            format!(
                r#"<a class="{class}" href="{}">{}</a>"#,
                html_escape(action.href.as_str()),
                html_escape(action.label.as_str())
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(r#"<div class="mei-host-shell__actions">{buttons}</div>"#)
}

fn shell_layout(
    document_title: &str,
    status_code: Option<u16>,
    headline: &str,
    body_html: &str,
    footer_html: &str,
    body_theme_style: &str,
) -> String {
    let title_esc = html_escape(document_title);
    let headline_esc = html_escape(headline);
    let status_block = if let Some(code) = status_code {
        format!(r#"<div class="mei-host-shell__status" aria-label="HTTP 状态码">{code}</div>"#)
    } else {
        String::new()
    };
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width,initial-scale=1" />
    <title>{title_esc}</title>
    <link rel="icon" href="/app-assets/favicon.svg" type="image/svg+xml" />
    <link rel="stylesheet" href="/app-assets/host-shell.css" />
  </head>
  <body class="mei-host-shell" style="{body_style}">
    <div class="mei-host-shell__stage">
      <div class="mei-host-shell__watermark" aria-hidden="true">{mei_coin}</div>
      <main class="mei-host-shell__card" role="main">
        <div class="mei-host-shell__brand">
          <span class="mei-host-shell__coin">{mei_coin}</span>
          <span class="mei-host-shell__brand-text">MeiLang</span>
        </div>
        {status_block}
        <h1 class="mei-host-shell__title">{headline_esc}</h1>
        {body_html}
      </main>
    </div>
    {footer_html}
  </body>
</html>"#,
        mei_coin = MEI_COIN_SVG,
        body_style = html_escape(body_theme_style),
    )
}

pub fn host_shell_body_theme_style(source_root: &Path) -> String {
    shell_body_theme_style(&load_workspace_config(source_root))
}

pub fn render_error_page_with_footer(
    status: StatusCode,
    headline: &str,
    message: &str,
    detail: Option<&str>,
    actions: &[HostShellAction],
    footer_html: &str,
    body_theme_style: &str,
) -> String {
    let mut card_inner = format!(
        r#"<p class="mei-host-shell__message">{}</p>"#,
        html_escape(message)
    );
    if let Some(detail) = detail.filter(|value| !value.trim().is_empty()) {
        card_inner.push_str(&format!(
            r#"<pre class="mei-host-shell__detail">{}</pre>"#,
            html_escape(detail)
        ));
    }
    card_inner.push_str(&render_actions(actions));
    shell_layout(
        headline,
        Some(status.as_u16()),
        headline,
        card_inner.as_str(),
        footer_html,
        body_theme_style,
    )
}

pub fn render_auth_card_page(
    document_title: &str,
    headline: &str,
    card_inner: &str,
    footer_html: &str,
    body_theme_style: &str,
) -> String {
    shell_layout(
        document_title,
        None,
        headline,
        card_inner,
        footer_html,
        body_theme_style,
    )
}

const STARTUP_WARMING_SCRIPT_TEMPLATE: &str = r#"<script>(function(){var delay=2000;var returnTo=__RETURN_TO__;var poll={app:"__APP__",scene:"__SCENE__",mode:"__MODE__"};function readinessUrl(){return"/api/host/access-readiness?app="+encodeURIComponent(poll.app)+"&scene="+encodeURIComponent(poll.scene)+"&mode="+encodeURIComponent(poll.mode);}function updateStatus(text){var el=document.querySelector(".mei-host-shell__startup-status");if(el&&text)el.textContent=text;}function tick(){fetch(readinessUrl(),{cache:"no-store",credentials:"same-origin",headers:{Accept:"application/json"}}).then(function(res){if(!res.ok){throw new Error("readiness "+res.status);}return res.json();}).then(function(body){body=body||{};if(body.startupError){updateStatus(body.startupError);setTimeout(tick,delay*2);return;}if(body.ready){updateStatus(body.startupDetail||"访问态已就绪，正在返回…");location.replace(returnTo);return;}var hint=body.startupDetail||body.reason||"准备中";if(body.bootstrapReason)hint+=" ("+body.bootstrapReason+")";updateStatus(hint);setTimeout(tick,delay);}).catch(function(){setTimeout(tick,delay);});}setTimeout(tick,delay);})();</script>"#;

pub fn render_startup_warming_page(
    source_root: &Path,
    status_line: &str,
    return_path: &str,
    poll_app_id: &str,
    poll_scene_id: &str,
    poll_mode: &str,
) -> String {
    let footer = render_host_shell_footer_for_source_root(source_root);
    let body_theme = host_shell_body_theme_style(source_root);
    let status_esc = html_escape(status_line.trim());
    let return_to_js = serde_json::to_string(return_path.trim()).unwrap_or_else(|_| "\"/\"".to_string());
    let script = STARTUP_WARMING_SCRIPT_TEMPLATE
        .replace("__RETURN_TO__", return_to_js.as_str())
        .replace("__APP__", poll_app_id.trim())
        .replace("__SCENE__", poll_scene_id.trim())
        .replace("__MODE__", poll_mode.trim());
    let body = format!(
        r#"<p class="mei-host-shell__message">服务正在准备启动中，请耐心等候。MeiLang 正在装载工作区、装配场景并预热访问态。</p>
<p class="mei-host-shell__tagline">梅花铜钱 · 以数据之形，载业务之实</p>
<p class="mei-host-shell__startup-status" aria-live="polite">{status_esc}</p>
<div class="mei-host-shell__progress" aria-hidden="true"><span></span><span></span><span></span></div>
<p class="mei-host-shell__hint">目标页面就绪后将自动返回 <code>{return_esc}</code>；亦可查看 <a class="mei-host-shell__link" href="{readiness_href}">access-readiness</a> 接口。</p>
{script}"#,
        return_esc = html_escape(return_path.trim()),
        readiness_href = html_escape(
            format!(
                "/api/host/access-readiness?app={}&scene={}&mode={}",
                poll_app_id.trim(),
                poll_scene_id.trim(),
                poll_mode.trim()
            )
            .as_str(),
        ),
        script = script,
    );
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width,initial-scale=1" />
    <title>服务准备中 - MeiLang</title>
    <link rel="icon" href="/app-assets/favicon.svg" type="image/svg+xml" />
    <link rel="stylesheet" href="/app-assets/host-shell.css" />
  </head>
  <body class="mei-host-shell mei-host-shell--warming" style="{body_style}">
    <div class="mei-host-shell__stage">
      <div class="mei-host-shell__watermark mei-host-shell__watermark--pulse" aria-hidden="true">{mei_coin}</div>
      <main class="mei-host-shell__card" role="main">
        <div class="mei-host-shell__brand">
          <span class="mei-host-shell__coin mei-host-shell__coin--spin">{mei_coin}</span>
          <span class="mei-host-shell__brand-text">MeiLang</span>
        </div>
        <h1 class="mei-host-shell__title">服务正在准备中</h1>
        {body}
      </main>
    </div>
    {footer}
  </body>
</html>"#,
        mei_coin = MEI_COIN_SVG,
        body_style = html_escape(body_theme.as_str()),
        body = body,
        footer = footer,
    )
}

pub fn render_startup_failed_page(source_root: &Path, message: &str) -> String {
    render_error_page_with_footer(
        StatusCode::SERVICE_UNAVAILABLE,
        "启动未完成",
        "宿主未能完成工作区装载，请查看下方详情或重新执行 prebuild。",
        Some(message),
        &[HostShellAction {
            href: "/".to_string(),
            label: "重试".to_string(),
            primary: true,
        }],
        &render_host_shell_footer_for_source_root(source_root),
        &host_shell_body_theme_style(source_root),
    )
}

pub fn host_starting_html_response(
    source_root: &Path,
    status_line: &str,
    return_path: &str,
    poll_app_id: &str,
    poll_scene_id: &str,
    poll_mode: &str,
) -> Response {
    let html = render_startup_warming_page(
        source_root,
        status_line,
        return_path,
        poll_app_id,
        poll_scene_id,
        poll_mode,
    );
    (StatusCode::OK, Html(html)).into_response()
}

pub fn startup_warming_html_response(
    source_root: &Path,
    status_line: &str,
    return_path: &str,
    poll_app_id: &str,
    poll_scene_id: &str,
    poll_mode: &str,
) -> Response {
    host_starting_html_response(
        source_root,
        status_line,
        return_path,
        poll_app_id,
        poll_scene_id,
        poll_mode,
    )
}

pub fn startup_failed_html_response(source_root: &Path, message: &str) -> Response {
    let html = render_startup_failed_page(source_root, message);
    (StatusCode::SERVICE_UNAVAILABLE, Html(html)).into_response()
}

pub fn forbidden_html_response(message: &str) -> Response {
    let html = render_error_page_with_footer(
        StatusCode::FORBIDDEN,
        "访问被拒绝",
        "当前账号无权执行此操作，或会话已失效。",
        Some(message),
        &[
            HostShellAction {
                href: "/login".to_string(),
                label: "重新登录".to_string(),
                primary: true,
            },
            HostShellAction {
                href: "/".to_string(),
                label: "返回首页".to_string(),
                primary: false,
            },
        ],
        "",
        &mei_lang_app::default_shell_body_theme_style(),
    );
    (StatusCode::FORBIDDEN, Html(html)).into_response()
}
