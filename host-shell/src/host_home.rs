use std::collections::BTreeSet;
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
use crate::shell_chrome::{active_running_app_ids, apps_for_topbar};
use crate::state::SharedState;
use crate::workspace_page::render_workspace_shell_page;

fn render_host_home_slots(
    workspace_root: &Path,
    enabled_apps: &[WorkspaceAppMeta],
    loaded_app_ids: &BTreeSet<String>,
    workspace_share_visible: bool,
) -> (String, String) {
    let workspace = mei_lang_kernel::load_workspace_config(workspace_root);
    let workspace_label = workspace
        .workspace
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let workspace_line = {
        let title = workspace_label.unwrap_or("MeiLang 工作区");
        let kicker = if workspace_label.is_some() {
            "工作区"
        } else {
            "控制面"
        };
        format!(
            r#"<header class="mei-host-shell__home-hero">
  <p class="mei-host-shell__home-kicker">{kicker}</p>
  <h1 class="mei-host-shell__home-title">{title}</h1>
</header>"#,
            kicker = html_escape(kicker),
            title = html_escape(title),
        )
    };

    let app_section = if enabled_apps.is_empty() {
        r#"<section class="mei-host-shell__home-empty" aria-labelledby="mei-home-empty-title">
  <h2 id="mei-home-empty-title" class="mei-host-shell__home-empty-title">还没有已启用的应用</h2>
  <p class="mei-host-shell__home-empty-body">顶栏与首页按启用清单展示应用。到应用中心启用后，入口会出现在这里（lazy 应用可在首次访问时自动载入）。</p>
  <p class="mei-host-shell__home-empty-actions"><a class="mei-host-shell__btn mei-host-shell__btn--primary" href="/runtime">打开应用中心</a></p>
</section>"#
            .to_string()
    } else {
        let count = enabled_apps.len();
        let cards = enabled_apps
            .iter()
            .map(|app| {
                // Align with /runtime: loaded = LaunchManifest active route → Running.
                let loaded = loaded_app_ids.contains(app.id.as_str());
                let access_href =
                    crate::shell_chrome::app_access_href(workspace_root, app.id.as_str());
                let status = if loaded { "ready" } else { "starting" };
                let status_label = if loaded {
                    "已载入"
                } else {
                    "已启用 · 未载入"
                };
                format!(
                    r#"<article class="mei-host-shell__app-card" data-status="{status}">
  <div class="mei-host-shell__app-card-body">
    <h2 class="mei-host-shell__card-title">{title}</h2>
    <p class="mei-host-shell__card-id">{app_id}</p>
  </div>
  <footer class="mei-host-shell__app-card-foot">
    <span class="mei-host-shell__card-status" data-status="{status}">{status_label}</span>
    {access_action}
  </footer>
</article>"#,
                    title = html_escape(app.title.as_str()),
                    app_id = html_escape(app.id.as_str()),
                    status = status,
                    status_label = status_label,
                    access_action = format!(
                        r#"<a class="mei-host-shell__btn mei-host-shell__btn--primary" href="{}">{}</a>"#,
                        html_escape(access_href.as_str()),
                        if loaded {
                            "进入应用"
                        } else {
                            "进入（将载入）"
                        },
                    ),
                )
            })
            .collect::<Vec<_>>()
            .join("");
        format!(
            r#"<section class="mei-host-shell__home-apps" aria-labelledby="mei-home-apps-title">
  <div class="mei-host-shell__home-section-head">
    <h2 id="mei-home-apps-title" class="mei-host-shell__home-section-title">已启用的应用 <span class="mei-host-shell__home-count">{count}</span></h2>
    <a class="mei-host-shell__home-section-link" href="/runtime">管理启用</a>
  </div>
  <div class="mei-host-shell__app-grid">{cards}</div>
</section>"#,
            count = count,
            cards = cards,
        )
    };

    let workspace_tools = if workspace_share_visible {
        r#"<section class="mei-host-shell__home-tools" aria-label="工作区工具">
  <a class="mei-host-shell__home-tool" href="/share">
    <span class="mei-host-shell__home-tool-label">资料交换</span>
    <span class="mei-host-shell__home-tool-hint">上传与共享工作资料</span>
  </a>
  <a class="mei-host-shell__home-tool" href="/runtime">
    <span class="mei-host-shell__home-tool-label">应用中心</span>
    <span class="mei-host-shell__home-tool-hint">启停与 launch 配置</span>
  </a>
</section>"#
    } else {
        r#"<section class="mei-host-shell__home-tools" aria-label="工作区工具">
  <a class="mei-host-shell__home-tool" href="/runtime">
    <span class="mei-host-shell__home-tool-label">应用中心</span>
    <span class="mei-host-shell__home-tool-hint">启停与 launch 配置</span>
  </a>
</section>"#
    };

    (workspace_line, format!("{app_section}{workspace_tools}"))
}

fn render_host_home_document_with_pack(
    pack: Option<&HostPagePack>,
    workspace_root: &Path,
    enabled_apps: &[WorkspaceAppMeta],
    loaded_app_ids: &BTreeSet<String>,
    topbar_menu: &TopbarMenuContext,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
) -> String {
    let workspace_share_visible = !auth_enabled
        || account_view.is_some_and(|account| account.capabilities.workspace_share_view);
    let (workspace_line, app_cards) = render_host_home_slots(
        workspace_root,
        enabled_apps,
        loaded_app_ids,
        workspace_share_visible,
    );
    let body_html = match render_home_page_body(pack, workspace_line.as_str(), app_cards.as_str()) {
        Ok(html) => html,
        Err(error) => return render_native_recovery_html(error),
    };
    render_workspace_shell_page(
        workspace_root,
        enabled_apps,
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
    // 0537: home lists enabled apps; load labels match /runtime (LaunchManifest Running).
    let enabled = apps_for_topbar(&guard);
    let loaded = active_running_app_ids(&guard.launch_manifest);
    let auth_enabled = auth.auth_enforcement == mei_host_auth::AuthEnforcement::Required;
    let account_view = account_view_for_principal(principal.as_ref().map(|Extension(p)| p));
    let html = render_host_home_document_with_pack(
        Some(home_page_pack()),
        workspace_root,
        enabled.as_slice(),
        &loaded,
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
        let loaded = BTreeSet::new();
        let html = render_host_home_document_with_pack(
            Some(home_page_pack()),
            root.as_path(),
            &[],
            &loaded,
            &topbar_menu,
            false,
            None,
        );
        assert!(html.contains("还没有已启用的应用") || html.contains("MeiLang 工作区"));
        assert!(html.contains("mei-host-shell__home-hero"));
        assert!(html.contains("/runtime"));
        assert!(html.contains("topbar-shell"));
        assert!(html.contains(r#"data-mei-pagepack="host.home""#));
        assert!(html.contains(r#"data-mei-pagepack-digest="sha256:"#));
        assert!(html.contains(r#"data-mei-page-surface="document""#));
        assert!(!html.contains("app-tab") || !html.contains("data-mei-app-id"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn host_home_load_labels_match_runtime_running_set() {
        let root = std::env::temp_dir().join(format!(
            "mei-host-home-load-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        let apps = vec![
            WorkspaceAppMeta {
                id: "zhifa".to_string(),
                title: "Zhifa".to_string(),
                short_title: None,
                root: "apps/zhifa".to_string(),
            },
            WorkspaceAppMeta {
                id: "thunder".to_string(),
                title: "Thunder".to_string(),
                short_title: None,
                root: "apps/thunder".to_string(),
            },
        ];
        let loaded = BTreeSet::from(["zhifa".to_string()]);
        let (_hero, cards) =
            render_host_home_slots(root.as_path(), apps.as_slice(), &loaded, false);
        let zhifa_card = cards
            .split("<article")
            .find(|chunk| chunk.contains(">zhifa<") || chunk.contains(">zhifa</"))
            .or_else(|| {
                cards
                    .split("<article")
                    .find(|chunk| chunk.contains("zhifa"))
            })
            .expect("zhifa card");
        let thunder_card = cards
            .split("<article")
            .find(|chunk| chunk.contains("thunder"))
            .expect("thunder card");
        assert!(zhifa_card.contains("已载入"));
        assert!(zhifa_card.contains("进入应用"));
        assert!(!zhifa_card.contains("已启用 · 未载入"));
        assert!(thunder_card.contains("已启用 · 未载入"));
        assert!(thunder_card.contains("进入（将载入）"));
        assert!(!thunder_card.contains("已载入"));
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
        let loaded = BTreeSet::new();
        let missing = render_host_home_document_with_pack(
            None,
            root.as_path(),
            &[],
            &loaded,
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
            &loaded,
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
                short_title: None,
                root: "apps/mini-park".to_string(),
            },
            WorkspaceAppMeta {
                id: "zhifa".to_string(),
                title: "Zhifa".to_string(),
                short_title: None,
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
