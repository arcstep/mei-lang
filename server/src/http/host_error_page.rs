//! 宿主独立 HTML 壳页：登录、改密与 4xx/5xx 错误反馈（梅花铜钱视觉）。

use std::path::Path;

use axum::{
    http::{Method, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    Json,
};
use mei_lang_app::shell_body_theme_style;
use mei_lang_kernel::{load_workspace_config, WorkspaceComplianceConfig, WorkspaceConfig};
use serde_json::json;

pub const MEI_COIN_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" role="img" aria-hidden="true"><path fill="currentColor" fill-rule="evenodd" d="M16.000 1.400L17.255 1.656L18.402 2.378L19.366 3.437L20.130 4.652L20.740 5.834L21.294 6.830L21.913 7.556L22.704 8.010L23.728 8.272L24.974 8.470L26.352 8.751L27.713 9.238L28.880 9.994L29.689 11.018L30.032 12.240L29.883 13.552L29.308 14.836L28.450 16.000L27.497 17.006L26.639 17.876L26.020 18.685L25.710 19.534L25.687 20.517L25.851 21.687L26.045 23.034L26.101 24.475L25.878 25.878L25.301 27.085L24.370 27.953L23.156 28.395L21.783 28.401L20.386 28.050L19.078 27.488L17.922 26.900L16.915 26.461L16.000 26.300L15.085 26.461L14.078 26.900L12.922 27.488L11.614 28.050L10.217 28.401L8.844 28.395L7.630 27.953L6.699 27.085L6.122 25.878L5.899 24.475L5.955 23.034L6.149 21.688L6.313 20.517L6.290 19.534L5.980 18.685L5.361 17.876L4.503 17.006L3.550 16.000L2.692 14.836L2.117 13.552L1.968 12.240L2.311 11.018L3.120 9.994L4.287 9.238L5.648 8.751L7.026 8.470L8.272 8.272L9.296 8.010L10.087 7.556L10.706 6.830L11.260 5.834L11.870 4.652L12.634 3.437L13.598 2.378L14.745 1.656L16.000 1.400ZM12.9 11.75H19.1A1.15 1.15 0 0 1 20.25 12.9V19.1A1.15 1.15 0 0 1 19.1 20.25H12.9A1.15 1.15 0 0 1 11.75 19.1V12.9A1.15 1.15 0 0 1 12.9 11.75Z"/></svg>"#;

#[derive(Debug, Clone)]
pub struct HostShellFooterInfo {
    pub version_label: String,
    pub compliance: WorkspaceComplianceConfig,
}

impl HostShellFooterInfo {
    pub fn version_only() -> Self {
        Self {
            version_label: crate::build_info::version_label(),
            compliance: WorkspaceComplianceConfig::default(),
        }
    }

    pub fn from_workspace(cfg: &WorkspaceConfig) -> Self {
        Self {
            version_label: crate::build_info::version_label(),
            compliance: cfg.compliance.clone(),
        }
    }
}

pub fn footer_info_from_source_root(source_root: &Path) -> HostShellFooterInfo {
    let cfg = load_workspace_config(source_root);
    let mut info = HostShellFooterInfo::from_workspace(&cfg);
    info.version_label = mei_lang_kernel::resolve_build_footer_label(source_root);
    info
}

pub fn render_host_shell_footer_for_source_root(source_root: &Path) -> String {
    render_host_shell_footer(&footer_info_from_source_root(source_root))
}

pub fn render_host_shell_footer(info: &HostShellFooterInfo) -> String {
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
    let compliance = if compliance_parts.is_empty() {
        String::new()
    } else {
        format!(
            r#"<span class="status-chip status-chip-compliance" data-tone="neutral">{}</span>"#,
            compliance_parts.join(" · ")
        )
    };
    format!(
        r#"<footer class="mei-host-shell__footer statusbar statusbar-shell chrome-safe-x" role="contentinfo"><div class="statusbar-layout"><div class="statusbar-track statusbar-track-left"></div><div class="statusbar-track statusbar-track-center">{compliance}</div><span class="status-chip status-chip-host statusbar-right-anchor" data-tone="neutral">{version}</span></div></footer>"#,
        version = html_escape(info.version_label.as_str()),
    )
}

#[derive(Debug, Clone)]
pub struct HostShellAction {
    pub href: String,
    pub label: String,
    pub primary: bool,
}

#[derive(Debug, Clone)]
pub struct HostScopedRebuildContext {
    pub app_id: String,
    pub scene_id: Option<String>,
    pub target_file: Option<String>,
}

pub fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render_scoped_rebuild_block(ctx: &HostScopedRebuildContext) -> String {
    let scene = html_escape(ctx.scene_id.as_deref().unwrap_or(""));
    let target = html_escape(ctx.target_file.as_deref().unwrap_or(""));
    let app_id = html_escape(ctx.app_id.as_str());
    format!(
        r#"<p class="mei-host-shell__actions"><button type="button" class="mei-host-shell__btn mei-host-shell__btn--primary" data-mei-scoped-rebuild="1" data-app-id="{app_id}" data-scene-id="{scene}" data-target-file="{target}">重建此 scope</button></p>"#
    )
}

pub fn render_scoped_rebuild_script() -> String {
    r#"<script>(function(){function bind(){document.querySelectorAll("[data-mei-scoped-rebuild]").forEach(function(btn){if(btn.__meiRebuildBound)return;btn.__meiRebuildBound=true;btn.addEventListener("click",async function(){var appId=btn.getAttribute("data-app-id")||"";if(!appId)return;btn.disabled=true;var prev=btn.textContent;btn.textContent="重建中…";try{var body={appId:appId,mode:"build"};var scene=btn.getAttribute("data-scene-id")||"";var target=btn.getAttribute("data-target-file")||"";if(scene)body.sceneId=scene;if(target)body.targetFile=target;var res=await fetch("/api/host/build",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify(body)});if(!res.ok){var err=await res.text();throw new Error(err||res.statusText);}location.reload();}catch(err){btn.disabled=false;btn.textContent=prev;alert(String(err&&err.message?err.message:err));}});});}if(document.readyState==="loading")document.addEventListener("DOMContentLoaded",bind);else bind();})();</script>"#.to_string()
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
    extra_head: &str,
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
    {extra_head}
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

pub fn render_error_page(
    status: StatusCode,
    headline: &str,
    message: &str,
    detail: Option<&str>,
    actions: &[HostShellAction],
) -> String {
    render_error_page_with_footer(
        status,
        headline,
        message,
        detail,
        actions,
        &render_host_shell_footer(&HostShellFooterInfo::version_only()),
        &mei_lang_app::default_shell_body_theme_style(),
    )
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
    let status_code = status.as_u16();
    let document_title = format!("{status_code} {headline} - MeiLang");
    let message_esc = html_escape(message);
    let detail_block = detail
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            format!(
                r#"<p class="mei-host-shell__detail"><code>{}</code></p>"#,
                html_escape(value)
            )
        })
        .unwrap_or_default();
    let feedback = format!(
        r#"<p class="mei-host-shell__hint">请向管理员反馈错误代码 <strong>HTTP {status_code}</strong>，并说明操作路径与时间。</p>"#
    );
    let body = format!(
        r#"<p class="mei-host-shell__message">{message_esc}</p>{detail_block}{feedback}{actions}"#,
        actions = render_actions(actions)
    );
    shell_layout(
        document_title.as_str(),
        Some(status_code),
        headline,
        body.as_str(),
        "",
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
        "",
        footer_html,
        body_theme_style,
    )
}

pub fn forbidden_html_response(message: &str) -> Response {
    let html = render_error_page(
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
    );
    (StatusCode::FORBIDDEN, Html(html)).into_response()
}

pub async fn fallback_handler(method: Method, uri: Uri) -> Response {
    if uri.path().starts_with("/api/") {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "not found",
                "status": 404,
                "method": method.as_str(),
                "path": uri.path(),
            })),
        )
            .into_response();
    }
    let path = uri.path().to_string();
    let html = render_error_page(
        StatusCode::NOT_FOUND,
        "页面不存在",
        "请求的地址在宿主中未找到，可能已被移动或输入有误。",
        Some(path.as_str()),
        &[HostShellAction {
            href: "/".to_string(),
            label: "返回首页".to_string(),
            primary: true,
        }],
    );
    (StatusCode::NOT_FOUND, Html(html)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_page_includes_status_and_coin() {
        let html = render_error_page(
            StatusCode::FORBIDDEN,
            "访问被拒绝",
            "无权访问",
            Some("demo detail"),
            &[],
        );
        assert!(html.contains("mei-host-shell__status"));
        assert!(html.contains(">403<"));
        assert!(html.contains("mei-host-shell__coin"));
        assert!(html.contains("HTTP 403"));
    }

    #[test]
    fn auth_card_page_omits_status_code() {
        let footer = render_host_shell_footer(&HostShellFooterInfo::version_only());
        let html = render_auth_card_page(
            "登录 - MeiLang",
            "MeiLang 登录",
            "<p>form</p>",
            footer.as_str(),
            &mei_lang_app::default_shell_body_theme_style(),
        );
        assert!(!html.contains("mei-host-shell__status"));
        assert!(html.contains("mei-host-shell__coin"));
        assert!(html.contains("mei-host-shell__footer"));
    }

    #[test]
    fn footer_includes_version_and_compliance() {
        let info = HostShellFooterInfo {
            version_label: "Mei 1.0.0 · demo".to_string(),
            compliance: WorkspaceComplianceConfig {
                icp_record: Some("渝ICP备12345678号".to_string()),
                psb_record: Some("渝公网安备 12345678号".to_string()),
                copyright: None,
            },
        };
        let html = render_host_shell_footer(&info);
        assert!(html.contains("Mei 1.0.0 · demo"));
        assert!(html.contains("渝ICP备12345678号"));
        assert!(html.contains("渝公网安备 12345678号"));
        assert!(html.contains("statusbar-track-left"));
        assert!(html.contains("statusbar-right-anchor"));
    }
}
