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
use mei_lang_kernel::{load_workspace_config, WorkspaceAppMeta, resolve_default_scene_from_root};

use crate::landing::{choose_default_app, discover_workspace_apps, enrich_discovered_apps};
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

    let workspace_line = workspace_label
        .map(|label| {
            format!(
                r#"<p class="mei-host-shell__meta">工作区：{}</p>"#,
                html_escape(label)
            )
        })
        .unwrap_or_default();

    let host_tools = r#"<div class="mei-host-shell__actions">
  <a class="mei-host-shell__btn" href="/host/config">配置</a>
  <a class="mei-host-shell__btn" href="/host/upload">上传</a>
  <a class="mei-host-shell__btn" href="/host/runtime">运行</a>
</div>"#;

    let default_app = choose_default_app(workspace_root, apps)
        .map(|app| app.id.as_str());

    let app_section = if apps.is_empty() {
        r#"<p class="mei-host-shell__message">当前工作区尚未发现可加载的应用。可先使用上方工作区功能，或执行 prebuild 后再刷新。</p>"#
            .to_string()
    } else {
        let rows = apps
            .iter()
            .map(|app| {
                let app_root = workspace_root.join("apps").join(app.id.as_str());
                let scene = resolve_default_scene_from_root(&app_root)
                    .ok()
                    .flatten()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "home".to_string());
                let access_href = format!("/apps/app/{}/scene/{}", app.id, scene);
                let default_mark = if default_app == Some(app.id.as_str()) {
                    " <span class=\"mei-host-shell__meta\">(默认)</span>"
                } else {
                    ""
                };
                format!(
                    r#"<tr>
  <td><code>{app_id}</code>{default_mark}</td>
  <td>{title}</td>
  <td><a class="mei-host-shell__link" href="{access_href}">进入应用</a></td>
</tr>"#,
                    app_id = html_escape(app.id.as_str()),
                    default_mark = default_mark,
                    title = html_escape(app.title.as_str()),
                    access_href = html_escape(access_href.as_str()),
                )
            })
            .collect::<Vec<_>>()
            .join("");
        format!(
            r#"<p class="mei-host-shell__message">请从下方列表选择要进入的应用。顶栏中的配置、上传、运行等工作区功能无需先进入应用。</p>
<table class="mei-host-shell__table">
  <thead>
    <tr>
      <th>App</th>
      <th>标题</th>
      <th>入口</th>
    </tr>
  </thead>
  <tbody>{rows}</tbody>
</table>"#,
            rows = rows,
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
<p class="mei-host-shell__tagline">梅花铜钱 · 以数据之形，载业务之实</p>
<p class="mei-host-shell__message">MeiLang 宿主已就绪。此处为工作区入口，不会自动进入某个应用。</p>
{host_tools}
{app_section}
{auth_actions}"#,
        workspace_line = workspace_line,
        host_tools = host_tools,
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
        assert!(html.contains("/host/config"));
        assert!(html.contains("host-shell.css"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
