use std::path::Path;

use mei_lang_app::{
    page_body_theme_style, render_workspace_page, render_workspace_page_with_pending_app,
    HostAccountView, TopbarMenuContext, WorkspaceShellNav,
};
use mei_lang_kernel::{load_workspace_config, WorkspaceAppMeta};

use crate::build_info::fill_page_shell_placeholders;

pub(crate) fn render_workspace_shell_page(
    workspace_root: &Path,
    apps: &[WorkspaceAppMeta],
    topbar_menu: &TopbarMenuContext,
    shell_nav: WorkspaceShellNav,
    page_title: &str,
    main_inner_html: &str,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
) -> String {
    render_workspace_shell_page_with_pending_app(
        workspace_root,
        apps,
        topbar_menu,
        shell_nav,
        page_title,
        main_inner_html,
        auth_enabled,
        account_view,
        None,
        None,
    )
}

pub(crate) fn render_workspace_shell_page_with_pending_app(
    workspace_root: &Path,
    apps: &[WorkspaceAppMeta],
    topbar_menu: &TopbarMenuContext,
    shell_nav: WorkspaceShellNav,
    page_title: &str,
    main_inner_html: &str,
    auth_enabled: bool,
    account_view: Option<&HostAccountView>,
    pending_app_id: Option<&str>,
    pending_scene: Option<&str>,
) -> String {
    let workspace = load_workspace_config(workspace_root);
    let theme_style = page_body_theme_style(&workspace, None, None);
    let html = if pending_app_id.is_some() || pending_scene.is_some() {
        render_workspace_page_with_pending_app(
            page_title,
            shell_nav,
            apps,
            Some(topbar_menu),
            main_inner_html,
            auth_enabled,
            account_view,
            theme_style.as_str(),
            pending_app_id,
            pending_scene,
        )
    } else {
        render_workspace_page(
            page_title,
            shell_nav,
            apps,
            Some(topbar_menu),
            main_inner_html,
            auth_enabled,
            account_view,
            theme_style.as_str(),
        )
    };
    fill_page_shell_placeholders(html, workspace_root)
}
