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
    /// `/host/starting` 等门闩页：不要挂 Access SPA（否则会并行打 scene API → 404 toast）。
    Starting,
}

impl WorkspaceShellNav {
    fn shell_nav_active(self) -> Option<ShellNavActive> {
        match self {
            Self::Home => Some(ShellNavActive::Home),
            Self::Runtime => Some(ShellNavActive::Runtime),
            Self::Share => Some(ShellNavActive::Share),
            // Gate page: keep topbar apps, no false "home/runtime" highlight.
            Self::Starting => None,
        }
    }

    fn document_route_mode(self) -> UiRouteMode {
        match self {
            // Workspace-level pages must not use Access App mode: access.js cold-start
            // would fetch scene-drilldown for the topbar default app → 404 toast on /home.
            Self::Home | Self::Share | Self::Runtime | Self::Starting => UiRouteMode::Runtime,
        }
    }

    fn status_path(self) -> &'static str {
        match self {
            Self::Home => "/home",
            Self::Runtime => "/runtime",
            Self::Share => "/share",
            Self::Starting => "/host/starting",
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
    // Target app while on `/host/starting` (avoid flashing the first enabled app).
    pending_app_id: Option<&str>,
    pending_scene: Option<&str>,
) -> AnyView {
    let active_app = pending_app_id.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("");
    let access_scene = pending_scene.map(str::trim).filter(|s| !s.is_empty());
    let topbar = topbar_view(
        apps,
        active_app,
        topbar_menu,
        UiRouteMode::App,
        access_scene,
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
        shell_nav.shell_nav_active(),
        &[],
        None,
    );
    let statusbar = statusbar_view(
        active_app,
        "workspace",
        shell_nav.status_path(),
        None,
    );
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
    let page_attrs = if active_app.is_empty() {
        view! {
            <div class=page_class inner_html=main_inner_html.to_string()></div>
        }
        .into_any()
    } else {
        let app_id = active_app.to_string();
        let scene_id = access_scene.unwrap_or("home").to_string();
        view! {
            <div
                class=page_class
                data-app-id=app_id
                data-scene-id=scene_id
                inner_html=main_inner_html.to_string()
            ></div>
        }
        .into_any()
    };
    view! {
        <div class="shell shell-surface workspace-view-shell mei-text-primary min-h-0 flex flex-1 flex-col">
            <div id="mei-host-topbar-slot" data-mei-host-chrome="top">{topbar}</div>
            <main class=main_class>
                {page_attrs}
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
        shell_nav.shell_nav_active(),
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
    render_workspace_page_with_pending_app(
        page_title,
        shell_nav,
        apps,
        topbar_menu,
        main_inner_html,
        auth_enabled,
        auth_account,
        shell_body_theme_style,
        None,
        None,
    )
}

pub fn render_workspace_page_with_pending_app(
    page_title: &str,
    shell_nav: WorkspaceShellNav,
    apps: &[WorkspaceAppMeta],
    topbar_menu: Option<&TopbarMenuContext>,
    main_inner_html: &str,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    shell_body_theme_style: &str,
    pending_app_id: Option<&str>,
    pending_scene: Option<&str>,
) -> String {
    let route_mode = shell_nav.document_route_mode();
    let shell = workspace_shell(
        apps,
        topbar_menu,
        shell_nav,
        main_inner_html,
        auth_enabled,
        auth_account,
        pending_app_id,
        pending_scene,
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
