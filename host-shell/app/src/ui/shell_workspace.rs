use leptos::prelude::*;
use mei_lang_kernel::WorkspaceAppMeta;

use super::document::render_document;
use super::route::UiRouteMode;
use super::statusbar::statusbar_view;
use super::topbar::{topbar_view, ShellNavActive};
use super::{HostAccountView, TopbarMenuContext};

/// Shell 全局导航高亮项（工作区级页面，不绑定 app）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceShellNav {
    Home,
    Runtime,
    Share,
}

impl WorkspaceShellNav {
    fn shell_nav_active(self) -> ShellNavActive {
        match self {
            Self::Home => ShellNavActive::Home,
            Self::Runtime => ShellNavActive::Runtime,
            Self::Share => ShellNavActive::Share,
        }
    }

    fn document_route_mode(self) -> UiRouteMode {
        match self {
            Self::Home => UiRouteMode::App,
            Self::Runtime => UiRouteMode::Runtime,
            Self::Share => UiRouteMode::App,
        }
    }

    fn status_path(self) -> &'static str {
        match self {
            Self::Home => "/home",
            Self::Runtime => "/runtime",
            Self::Share => "/share",
        }
    }
}

pub(crate) fn workspace_shell(
    apps: &[WorkspaceAppMeta],
    topbar_menu: Option<&TopbarMenuContext>,
    shell_nav: WorkspaceShellNav,
    main_inner_html: &str,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> AnyView {
    let topbar = topbar_view(
        apps,
        "",
        topbar_menu,
        UiRouteMode::App,
        None,
        None,
        None,
        None,
        None,
        false,
        false,
        auth_enabled,
        auth_account,
        None,
        None,
        None,
        None,
        Some(shell_nav.shell_nav_active()),
        &[],
        None,
    );
    let statusbar = statusbar_view("", "workspace", shell_nav.status_path(), None);
    let main_class = match shell_nav {
        WorkspaceShellNav::Share | WorkspaceShellNav::Runtime => {
            "workspace-view-main chrome-inset min-h-0 flex flex-1 flex-col overflow-hidden px-4 py-3"
        }
        _ => "workspace-view-main chrome-inset min-h-0 flex flex-1 flex-col overflow-auto px-4 py-3",
    };
    let page_class = match shell_nav {
        WorkspaceShellNav::Share | WorkspaceShellNav::Runtime => {
            "mei-workspace-page mei-workspace-page--fill"
        }
        _ => "mei-workspace-page",
    };
    view! {
        <div class="shell shell-surface workspace-view-shell mei-text-primary min-h-0 flex flex-1 flex-col">
            <div id="mei-host-topbar-slot" data-mei-host-chrome="top">{topbar}</div>
            <main class=main_class>
                <div class=page_class inner_html=main_inner_html.to_string()></div>
            </main>
            <div id="mei-host-statusbar-slot" data-mei-host-chrome="bottom">{statusbar}</div>
        </div>
    }
    .into_any()
}

/// Topbar + statusbar HTML for workspace pages (`/home` `/runtime` …) — LaunchManifest running apps only.
pub fn render_workspace_shell_chrome_html(
    apps: &[WorkspaceAppMeta],
    topbar_menu: Option<&TopbarMenuContext>,
    shell_nav: WorkspaceShellNav,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> (String, String) {
    let topbar = topbar_view(
        apps,
        "",
        topbar_menu,
        UiRouteMode::App,
        None,
        None,
        None,
        None,
        None,
        false,
        false,
        auth_enabled,
        auth_account,
        None,
        None,
        None,
        None,
        Some(shell_nav.shell_nav_active()),
        &[],
        None,
    );
    let statusbar = statusbar_view("", "workspace", shell_nav.status_path(), None);
    (topbar.to_html(), statusbar.to_html())
}

pub fn render_workspace_page(
    page_title: &str,
    shell_nav: WorkspaceShellNav,
    apps: &[WorkspaceAppMeta],
    topbar_menu: Option<&TopbarMenuContext>,
    main_inner_html: &str,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    shell_body_theme_style: &str,
) -> String {
    let route_mode = shell_nav.document_route_mode();
    let shell = workspace_shell(
        apps,
        topbar_menu,
        shell_nav,
        main_inner_html,
        auth_enabled,
        auth_account,
    );
    let html = render_document(
        page_title,
        route_mode,
        false,
        shell,
        view! { <></> }.into_any(),
        view! { <></> }.into_any(),
        auth_enabled,
        auth_account,
        shell_body_theme_style,
        Some("workspace-view"),
        Some(r#"<link rel="stylesheet" href="/app-assets/host-shell.css" />"#),
    );
    html
}
