use std::path::Path;

use axum::{
    extract::{Extension, State},
    response::{Html, IntoResponse},
};
use mei_host_auth::{
    account_view_for_principal, filter_apps_for_principal, html_escape, AuthPrincipal,
    AuthServeState,
};
use mei_lang_app::{load_topbar_menu_context, WorkspaceShellNav};
use mei_lang_kernel::{load_workspace_config, WorkspaceAppMeta};

use crate::landing::{
    app_has_prebuilt_access_entry, choose_default_app, discover_workspace_apps,
    enrich_discovered_apps,
};
use crate::state::SharedState;
use crate::workspace_page::render_workspace_shell_page;

pub fn render_host_home_body_html(workspace_root: &Path, apps: &[WorkspaceAppMeta]) -> String {
    let workspace = load_workspace_config(workspace_root);
    let workspace_label = workspace
        .workspace
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

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
        r#"<p class="mei-host-shell__message">当前工作区尚未发现可加载的应用。可先使用顶栏工作区导航进入配置、上传或运行，或执行 prebuild 后再刷新。</p>"#
            .to_string()
    } else {
        let cards = apps
            .iter()
            .map(|app| {
                let access_ready = app_has_prebuilt_access_entry(workspace_root, app.id.as_str());
                let access_href = format!("/apps/{}/home", app.id);
                let status = if access_ready { "ready" } else { "missing" };
                let status_label = if access_ready {
                    "已编译"
                } else {
                    "待预构建"
                };
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
  </div>
</article>"#,
                    title = html_escape(app.title.as_str()),
                    default_mark = default_mark,
                    app_id = html_escape(app.id.as_str()),
                    summary = html_escape(app.title.as_str()),
                    status = status,
                    status_label = status_label,
                    access_href = html_escape(access_href.as_str()),
                )
            })
            .collect::<Vec<_>>()
            .join("");
        format!(
            r#"<section class="mei-host-shell__app-grid">{cards}</section>"#,
            cards = cards,
        )
    };

    format!(
        r#"{workspace_line}{app_section}"#,
        workspace_line = workspace_line,
        app_section = app_section,
    )
}

pub async fn host_home_page(
    State(state): State<SharedState>,
    State(auth): State<AuthServeState>,
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
    let account_view = account_view_for_principal(principal.as_ref().map(|Extension(p)| p));
    let body_html = render_host_home_body_html(workspace_root, apps.as_slice());
    let html = render_workspace_shell_page(
        workspace_root,
        apps.as_slice(),
        &topbar_menu,
        WorkspaceShellNav::Home,
        "MeiLang 工作区",
        body_html.as_str(),
        auth_enabled,
        account_view.as_ref(),
    );
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
        let topbar_menu = mei_lang_app::load_topbar_menu_context(root.as_path());
        let body = render_host_home_body_html(root.as_path(), &[]);
        let html = render_workspace_shell_page(
            root.as_path(),
            &[],
            &topbar_menu,
            WorkspaceShellNav::Home,
            "MeiLang 工作区",
            body.as_str(),
            false,
            None,
        );
        assert!(html.contains("欢迎使用 MeiLang") || html.contains("MeiLang 工作区"));
        assert!(html.contains("topbar-shell"));
        assert!(html.contains("statusbar-shell"));
        assert!(html.contains("mei-workspace-page"));
        assert!(html.contains("shell-nav-link"));
        assert!(html.contains("topbar-app-toolbar"));
        assert!(html.contains("topbar-system-toolbar"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn host_home_topbar_shows_app_menu_before_system_nav() {
        let root = std::env::temp_dir().join(format!(
            "mei-host-home-apps-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        let topbar_menu = mei_lang_app::load_topbar_menu_context(root.as_path());
        let apps = vec![
            WorkspaceAppMeta {
                id: "mini-park".to_string(),
                title: "Mini Park".to_string(),
                root: "apps/mini-park".to_string(),
            },
            WorkspaceAppMeta {
                id: "pretty-panels".to_string(),
                title: "Pretty Panels".to_string(),
                root: "apps/pretty-panels".to_string(),
            },
        ];
        let html = mei_lang_app::render_workspace_page(
            "MeiLang 工作区",
            WorkspaceShellNav::Home,
            apps.as_slice(),
            Some(&topbar_menu),
            "<p>test</p>",
            false,
            None,
            "",
        );
        assert!(html.contains("app-tab") || html.contains("app-group-trigger"));
        assert!(html.contains("shell-nav-link"));
        let app_toolbar = html.find("topbar-app-toolbar").expect("app toolbar region");
        let system_toolbar = html
            .find("topbar-system-toolbar")
            .expect("system toolbar region");
        assert!(app_toolbar < system_toolbar);
        assert!(!html.contains("app-current-path"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
