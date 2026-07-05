use std::path::Path;

use axum::{
    extract::{Extension, State},
    response::{Html, IntoResponse},
};
use mei_host_auth::{
    filter_apps_for_principal, html_escape, render_auth_card_page,
    render_host_shell_footer_for_source_root, host_shell_body_theme_style, AuthPrincipal,
};
use mei_lang_app::load_topbar_menu_context;
use mei_lang_kernel::{load_workspace_config, WorkspaceAppMeta};

use crate::landing::{app_has_prebuilt_access_entry, choose_default_app, discover_workspace_apps, enrich_discovered_apps};
use crate::shell_nav::{render_shell_nav_html, ShellNavItem};
use crate::state::SharedState;

pub fn render_host_home_html(
    workspace_root: &Path,
    apps: &[WorkspaceAppMeta],
    auth_enabled: bool,
) -> String {
    let workspace = load_workspace_config(workspace_root);
    let workspace_label = workspace
        .workspace
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let footer_html = render_host_shell_footer_for_source_root(workspace_root);
    let shell_theme = host_shell_body_theme_style(workspace_root);
    let shell_nav = render_shell_nav_html(ShellNavItem::Home);

    let workspace_line = workspace_label
        .map(|label| {
            format!(
                r#"<p class="mei-host-shell__meta">工作区：{}</p>"#,
                html_escape(label)
            )
        })
        .unwrap_or_default();

    let default_app = choose_default_app(workspace_root, apps).map(|app| app.id.as_str());

    let app_section = if apps.is_empty() {
        r#"<p class="mei-host-shell__message">当前工作区尚未发现可加载的应用。可先使用上方工作区功能，或执行 prebuild 后再刷新。</p>"#
            .to_string()
    } else {
        let cards = apps
            .iter()
            .map(|app| {
                let access_ready = app_has_prebuilt_access_entry(workspace_root, app.id.as_str());
                let access_href = format!("/apps/app/{}/access", app.id);
                let build_href = format!("/apps/build/{}", app.id);
                let status = if access_ready { "ready" } else { "missing" };
                let status_label = if access_ready { "已编译" } else { "待预构建" };
                let default_mark = if default_app == Some(app.id.as_str()) {
                    r#"<span class="mei-host-shell__card-badge">默认</span>"#
                } else {
                    ""
                };
                format!(
                    r#"<article class="mei-host-shell__app-card">
  <header class="mei-host-shell__app-card-head">
    <h2 class="mei-host-shell__card-title">{title}</h2>
    {default_mark}
  </header>
  <p class="mei-host-shell__card-id"><code>{app_id}</code></p>
  <p class="mei-host-shell__card-desc">{summary}</p>
  <p class="mei-host-shell__card-status" data-status="{status}">{status_label}</p>
  <div class="mei-host-shell__card-actions">
    <a class="mei-host-shell__btn" href="{access_href}">进入应用</a>
    <a class="mei-host-shell__btn mei-host-shell__btn--ghost" href="{build_href}">开发</a>
  </div>
</article>"#,
                    title = html_escape(app.title.as_str()),
                    default_mark = default_mark,
                    app_id = html_escape(app.id.as_str()),
                    summary = html_escape(app.title.as_str()),
                    status = status,
                    status_label = status_label,
                    access_href = html_escape(access_href.as_str()),
                    build_href = html_escape(build_href.as_str()),
                )
            })
            .collect::<Vec<_>>()
            .join("");
        format!(
            r#"<p class="mei-host-shell__message">从下方卡片选择应用。顶栏 Shell 导航可随时进入配置、上传、运行与 MCG 检视。</p>
<section class="mei-host-shell__app-grid">{cards}</section>"#,
            cards = cards,
        )
    };

    let auth_actions = if auth_enabled {
        r#"<div class="mei-host-shell__actions">
  <a class="mei-host-shell__btn" href="/login">登录</a>
  <a class="mei-host-shell__btn" href="/account/password">改密</a>
</div>"#
    } else {
        ""
    };

    let body_html = format!(
        r#"{workspace_line}
{shell_nav}
<p class="mei-host-shell__tagline">梅花铜钱 · 以数据之形，载业务之实</p>
<p class="mei-host-shell__message">MeiLang 宿主已就绪。此处为工作区首页，不会自动进入某个应用。</p>
{app_section}
{auth_actions}"#,
        workspace_line = workspace_line,
        shell_nav = shell_nav,
        app_section = app_section,
        auth_actions = auth_actions,
    );

    render_auth_card_page(
        "MeiLang 工作区",
        "欢迎使用 MeiLang",
        body_html.as_str(),
        footer_html.as_str(),
        shell_theme.as_str(),
    )
}

pub async fn host_home_page(
    State(state): State<SharedState>,
    State(auth): State<mei_host_auth::AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let discovered = discover_workspace_apps(workspace_root).unwrap_or_default();
    let topbar_menu = load_topbar_menu_context(workspace_root);
    let apps = enrich_discovered_apps(
        filter_apps_for_principal(
            discovered.as_slice(),
            principal.as_ref().map(|Extension(p)| p),
        )
        .as_slice(),
        &topbar_menu,
    );
    let auth_enabled = auth.auth_enforcement == mei_host_auth::AuthEnforcement::Required;
    let html = render_host_home_html(workspace_root, apps.as_slice(), auth_enabled);
    Html(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_home_renders_without_apps() {
        let root = std::env::temp_dir().join(format!(
            "mei-host-home-empty-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        let html = render_host_home_html(root.as_path(), &[], false);
        assert!(html.contains("欢迎使用 MeiLang"));
        assert!(html.contains("/config"));
        assert!(html.contains("mei-host-shell__app-grid"));
        assert!(html.contains("host-shell.css"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
