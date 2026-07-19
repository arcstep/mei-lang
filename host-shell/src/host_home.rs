use std::path::Path;

use axum::{
    extract::{Extension, State},
    response::{Html, IntoResponse},
};
use mei_host_auth::{account_view_for_principal, html_escape, AuthPrincipal, AuthServeState};
use mei_lang_app::{
    load_topbar_menu_context, HostAccountView, TopbarMenuContext, WorkspaceShellNav,
};
use mei_lang_kernel::WorkspaceAppMeta;

use crate::host_page_pack::{
    home_page_pack, render_home_page_body, render_native_recovery_html, HostPagePack,
};
use crate::landing::app_has_prebuilt_access_entry;
use crate::shell_chrome::apps_for_topbar;
use crate::state::SharedState;
use crate::workspace_page::render_workspace_shell_page;

fn render_host_home_slots(
    workspace_root: &Path,
    running_apps: &[WorkspaceAppMeta],
    data_plane_enabled: bool,
) -> (String, String) {
    let workspace = mei_lang_kernel::load_workspace_config(workspace_root);
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

    let app_section = if running_apps.is_empty() {
        r#"<section class="mei-host-shell__message">
  <h2>控制面已就绪</h2>
  <p>当前没有已启动的应用。顶栏与首页只展示运行中的应用；请到运行控制中心选择 launch config 并启动。</p>
  <p><a class="mei-host-shell__btn" href="/runtime">打开运行控制中心</a></p>
</section>"#
            .to_string()
    } else {
        let cards = running_apps
            .iter()
            .map(|app| {
                let access_ready = data_plane_enabled
                    && app_has_prebuilt_access_entry(workspace_root, app.id.as_str());
                let access_href =
                    crate::shell_chrome::app_access_href(workspace_root, app.id.as_str());
                let status = if access_ready { "ready" } else { "starting" };
                let status_label = if access_ready {
                    "已启动"
                } else {
                    "启动中"
                };
                format!(
                    r#"<article class="mei-host-shell__app-card">
  <header class="mei-host-shell__app-card-head">
    <h2 class="mei-host-shell__card-title">{title}</h2>
    <span class="mei-host-shell__card-badge">运行中</span>
  </header>
  <p class="mei-host-shell__card-id"><code>{app_id}</code></p>
  <p class="mei-host-shell__card-desc">{summary}</p>
  <p class="mei-host-shell__card-status" data-status="{status}">{status_label}</p>
  <div class="mei-host-shell__card-actions">{access_action}</div>
</article>"#,
                    title = html_escape(app.title.as_str()),
                    app_id = html_escape(app.id.as_str()),
                    summary = html_escape(app.title.as_str()),
                    status = status,
                    status_label = status_label,
                    access_action = if access_ready {
                        format!(
                            r#"<a class="mei-host-shell__btn" href="{}">进入应用</a>"#,
                            html_escape(access_href.as_str())
                        )
                    } else {
                        r#"<a class="mei-host-shell__btn mei-host-shell__btn--ghost" href="/runtime">查看运行状态</a>"#.to_string()
                    },
                )
            })
            .collect::<Vec<_>>()
            .join("");
        format!(
            r#"<section class="mei-host-shell__app-grid">{cards}</section>
<p class="mei-host-shell__meta"><a class="mei-host-shell__link" href="/runtime">管理全部应用启停 →</a></p>"#,
            cards = cards,
        )
    };

    (workspace_line, app_section)
}

fn render_host_home_document_with_pack(
    pack: Option<&HostPagePack>,
    workspace_root: &Path,
    running_apps: &[WorkspaceAppMeta],
    data_plane_enabled: bool,
    topbar_menu: &TopbarMenuContext,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
) -> String {
    let (workspace_line, app_cards) =
        render_host_home_slots(workspace_root, running_apps, data_plane_enabled);
    let body_html = match render_home_page_body(pack, workspace_line.as_str(), app_cards.as_str()) {
        Ok(html) => html,
        Err(error) => return render_native_recovery_html(error),
    };
    render_workspace_shell_page(
        workspace_root,
        running_apps,
        topbar_menu,
        WorkspaceShellNav::Home,
        "MeiLang 工作区",
        body_html.as_str(),
        auth_enabled,
        account_view,
    )
}

pub async fn host_home_page(
    State(state): State<SharedState>,
    State(auth): State<AuthServeState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> impl IntoResponse {
    let guard = state.read().expect("state lock");
    let workspace_root = guard.ctx.workspace_root.as_path();
    let topbar_menu = load_topbar_menu_context(workspace_root);
    // 0537: home chrome + cards only list LaunchManifest active apps.
    let running = apps_for_topbar(&guard);
    let auth_enabled = auth.auth_enforcement == mei_host_auth::AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal.as_ref().map(|Extension(p)| p));
    let html = render_host_home_document_with_pack(
        Some(home_page_pack()),
        workspace_root,
        running.as_slice(),
        guard.data_plane_enabled,
        &topbar_menu,
        auth_enabled,
        account_view.as_ref(),
    );
    Html(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_home_renders_without_running_apps() {
        let root = std::env::temp_dir().join(format!(
            "mei-host-home-empty-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        let topbar_menu = mei_lang_app::load_topbar_menu_context(root.as_path());
        let html = render_host_home_document_with_pack(
            Some(home_page_pack()),
            root.as_path(),
            &[],
            false,
            &topbar_menu,
            false,
            None,
        );
        assert!(html.contains("控制面已就绪") || html.contains("MeiLang 工作区"));
        assert!(html.contains("/runtime"));
        assert!(html.contains("topbar-shell"));
        assert!(html.contains(r#"data-mei-pagepack="host.home""#));
        assert!(html.contains(r#"data-mei-pagepack-digest="sha256:"#));
        assert!(html.contains(r#"data-mei-page-surface="document""#));
        assert!(!html.contains("app-tab") || !html.contains("data-mei-app-id"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn host_home_missing_or_invalid_pack_returns_native_recovery_document() {
        let root = std::env::temp_dir().join(format!(
            "mei-host-home-recovery-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        let topbar_menu = mei_lang_app::load_topbar_menu_context(root.as_path());
        let missing = render_host_home_document_with_pack(
            None,
            root.as_path(),
            &[],
            false,
            &topbar_menu,
            false,
            None,
        );
        let mut invalid_pack = home_page_pack().clone();
        invalid_pack.aot_body_template.push_str("<!-- corrupt -->");
        let invalid = render_host_home_document_with_pack(
            Some(&invalid_pack),
            root.as_path(),
            &[],
            false,
            &topbar_menu,
            false,
            None,
        );

        for html in [missing, invalid] {
            assert!(html.contains("data-mei-native-recovery=\"host-page-pack\""));
            assert!(html.contains("href=\"/runtime\""));
            assert!(html.contains("href=\"/login\""));
            assert!(!html.contains("topbar-shell"));
            assert!(!html.contains("/app-assets/"));
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn host_home_topbar_order_with_running_apps() {
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
                id: "zhifa".to_string(),
                title: "Zhifa".to_string(),
                root: "apps/zhifa".to_string(),
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
        assert!(html.contains("data-mei-app-switcher") || html.contains("app-group-trigger"));
        assert!(html.contains("shell-nav-link"));
        assert!(html.contains("应用中心") || html.contains("/runtime"));
        let app_toolbar = html.find("topbar-app-toolbar").expect("app toolbar region");
        let system_toolbar = html
            .find("topbar-system-toolbar")
            .expect("system toolbar region");
        assert!(app_toolbar < system_toolbar);
        let _ = std::fs::remove_dir_all(&root);
    }
}
