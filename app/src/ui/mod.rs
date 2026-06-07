use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta, WorkspaceNode};
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

mod agent_panel;
mod compile_status;
mod manage_routing;
mod preview;
mod preview_chrome;
mod route;
mod shell_access;
mod shell_config;
mod shell_manage;
mod shell_upload;
mod source_tree;
mod statusbar;
mod topbar;
mod view_routing;

pub use route::UiRouteMode;
pub use shell_upload::UploadFileEntry;

use preview_chrome::{chrome_scripts_view, component_scripts};
use shell_access::access_shell;
use shell_config::config_shell;
use shell_manage::{manage_shell, manage_source_shell};
use shell_upload::upload_shell;

#[derive(Debug, Clone, Serialize)]
pub struct SourcePanelMeta {
    pub line_count: usize,
    pub char_count: usize,
    pub last_modified_label: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostAccountView {
    #[serde(default)]
    pub logged_in: bool,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub role: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TopbarMenuConfig {
    #[serde(default)]
    pub skip_prefixes: Vec<String>,
    #[serde(default)]
    pub groups: Vec<TopbarMenuConfigGroup>,
    #[serde(default)]
    pub items: Vec<TopbarMenuConfigItem>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TopbarMenuContext {
    pub root: Option<TopbarMenuConfig>,
    pub by_segment: BTreeMap<String, TopbarMenuConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopbarMenuConfigGroup {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopbarMenuConfigItem {
    pub app_id: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub subgroup: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub order: Option<i32>,
}

fn render_document(
    app_title: &str,
    route_mode: UiRouteMode,
    chrome_hidden: bool,
    shell: AnyView,
    component_scripts_view: AnyView,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> String {
    let shell_mode_class = match route_mode {
        UiRouteMode::App if chrome_hidden => "app-view chrome-none",
        UiRouteMode::App => "app-view",
        UiRouteMode::Build => "build-view",
        UiRouteMode::Config => "config-view",
        UiRouteMode::Upload => "upload-view",
    };
    let body_class = format!("{shell_mode_class} sl-theme-dark");
    let chrome_scripts = chrome_scripts_view(route_mode);
    let auth_user_meta = if auth_enabled {
        auth_account
            .map(|view| view.username.as_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("")
    } else {
        ""
    };
    let auth_role_meta = if auth_enabled {
        auth_account
            .map(|view| view.role.as_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("")
    } else {
        ""
    };
    let auth_logged_in_meta = if auth_enabled {
        auth_account
            .map(|view| if view.logged_in { "1" } else { "0" })
            .unwrap_or("0")
    } else {
        "0"
    };

    let manage_timing_meta = match route_mode {
        UiRouteMode::Build => view! {
            <meta name="mei-handler-html-ready-ms" content="__MEI_HANDLER_HTML_READY_MS__"/>
            <meta name="mei-ssr-http-response-body-ms" content="__MEI_SSR_HTTP_BODY_MS__"/>
        }
        .into_any(),
        _ => view! { <></> }.into_any(),
    };

    let page = view! {
        <html lang="zh-CN">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="mei-tiles-base-url" content="__MEI_TILES_BASE_URL__"/>
                <meta name="mei-tiles-json-path" content="__MEI_TILES_JSON_PATH__"/>
                <meta name="mei-view" content=route_mode.slug()/>
                <meta name="mei-auth-user" content=auth_user_meta/>
                <meta name="mei-auth-role" content=auth_role_meta/>
                <meta name="mei-auth-logged-in" content=auth_logged_in_meta/>
                <title>{format!("{app_title} - MeiLang")}</title>
                <link rel="icon" href="/app-assets/favicon.svg" type="image/svg+xml"/>
                <link rel="stylesheet" href="/app-bundles/styles.css"/>
                <script
                    type="module"
                    src="/app-bundles/shoelace.js"
                ></script>
                {manage_timing_meta}
            </head>
            <body
                class=body_class
                data-mei-view=route_mode.slug()
                data-mei-handler-html-ready-ms="__MEI_HANDLER_HTML_READY_MS__"
                data-mei-ssr-http-response-body-ms="__MEI_SSR_HTTP_BODY_MS__"
                data-mei-auth-user=auth_user_meta
                data-mei-auth-role=auth_role_meta
                data-mei-auth-logged-in=auth_logged_in_meta
            >
                {shell}
                {component_scripts_view}
                {chrome_scripts}
            </body>
        </html>
    };
    page.to_html()
}

pub fn render_page(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    route_mode: UiRouteMode,
    target: Option<&str>,
    source: Option<&str>,
    source_meta: Option<&SourcePanelMeta>,
    selected_scene: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
    diag_filter: Option<&str>,
    chrome_hidden: bool,
    upload_enabled: bool,
    upload_root_label: Option<&str>,
    upload_files: &[UploadFileEntry],
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> String {
    let shell = match route_mode {
        UiRouteMode::App => access_shell(
            apps,
            compiled,
            app_path,
            topbar_menu,
            selected_scene,
            target,
            source,
            active_tab,
            chrome_hidden,
            upload_enabled,
            auth_enabled,
            auth_account,
        ),
        UiRouteMode::Build => manage_shell(
            apps,
            compiled,
            app_path,
            topbar_menu,
            target,
            source,
            source_meta,
            selected_scene,
            preview_target,
            active_tab,
            diag_filter,
            upload_enabled,
            auth_enabled,
            auth_account,
        ),
        UiRouteMode::Config => config_shell(
            apps,
            compiled.title.as_str(),
            app_path,
            topbar_menu,
            upload_enabled,
            selected_scene,
            source_meta,
            auth_enabled,
            auth_account,
        ),
        UiRouteMode::Upload => upload_shell(
            apps,
            compiled.title.as_str(),
            app_path,
            topbar_menu,
            upload_enabled,
            selected_scene,
            upload_root_label.unwrap_or("upload"),
            upload_files,
            target,
            source,
            source_meta,
            auth_enabled,
            auth_account,
        ),
    };
    render_document(
        compiled.title.as_str(),
        route_mode,
        chrome_hidden,
        shell,
        component_scripts(compiled).into_any(),
        auth_enabled,
        auth_account,
    )
}

pub fn render_config_page(
    apps: &[WorkspaceAppMeta],
    app_title: &str,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    source: Option<&str>,
    source_meta: Option<&SourcePanelMeta>,
    selected_scene: Option<&str>,
    upload_enabled: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> String {
    let _ = source;
    let shell = config_shell(
        apps,
        app_title,
        app_path,
        topbar_menu,
        upload_enabled,
        selected_scene,
        source_meta,
        auth_enabled,
        auth_account,
    );
    render_document(
        app_title,
        UiRouteMode::Config,
        false,
        shell,
        view! { <></> }.into_any(),
        auth_enabled,
        auth_account,
    )
}

pub fn render_upload_page(
    apps: &[WorkspaceAppMeta],
    app_title: &str,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    target: Option<&str>,
    source: Option<&str>,
    source_meta: Option<&SourcePanelMeta>,
    selected_scene: Option<&str>,
    upload_enabled: bool,
    upload_root_label: Option<&str>,
    upload_files: &[UploadFileEntry],
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> String {
    let shell = upload_shell(
        apps,
        app_title,
        app_path,
        topbar_menu,
        upload_enabled,
        selected_scene,
        upload_root_label.unwrap_or("upload"),
        upload_files,
        target,
        source,
        source_meta,
        auth_enabled,
        auth_account,
    );
    render_document(
        app_title,
        UiRouteMode::Upload,
        false,
        shell,
        view! { <></> }.into_any(),
        auth_enabled,
        auth_account,
    )
}

pub fn render_build_source_page(
    apps: &[WorkspaceAppMeta],
    app_title: &str,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    file_tree: &[WorkspaceNode],
    target: &str,
    source: &str,
    source_meta: Option<&SourcePanelMeta>,
    selected_scene: Option<&str>,
    active_tab: Option<&str>,
    upload_enabled: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> String {
    let shell = manage_source_shell(
        apps,
        app_title,
        app_path,
        topbar_menu,
        file_tree,
        target,
        source,
        source_meta,
        selected_scene,
        active_tab,
        upload_enabled,
        auth_enabled,
        auth_account,
    );
    render_document(
        app_title,
        UiRouteMode::Build,
        false,
        shell,
        view! { <></> }.into_any(),
        auth_enabled,
        auth_account,
    )
}

#[cfg(test)]
mod tests {
    use super::manage_routing::{
        access_scene_query, encode_query_value, manage_tab_href, manage_view_tab_from_query,
        route_query, ManageViewTab, OPS_CONFIG_TARGET,
    };
    use super::view_routing::{build_href, config_href};
    use super::UiRouteMode;

    #[test]
    fn manage_defaults_to_diagnostics_when_errors_exist() {
        assert!(matches!(
            manage_view_tab_from_query(None, true, true, 1, "main.mei"),
            ManageViewTab::Diagnostics
        ));
    }

    #[test]
    fn build_href_uses_build_route() {
        assert_eq!(
            build_href("spbjw", Some("main.mei"), Some("preview")),
            "/apps/build/spbjw?file=main.mei&tab=preview"
        );
        assert_eq!(config_href("spbjw"), "/apps/config/spbjw");
    }

    #[test]
    fn manage_ops_config_href_forces_preview_tab() {
        assert_eq!(
            manage_tab_href(
                "spbjw",
                Some(OPS_CONFIG_TARGET),
                OPS_CONFIG_TARGET,
                false,
                ManageViewTab::Diagnostics,
                Some("all")
            ),
            "/apps/build/spbjw?file=.mei-config.json&tab=preview"
        );
    }

    #[test]
    fn route_query_omits_tab_for_cross_app_navigation() {
        assert_eq!(
            route_query(UiRouteMode::Build, None, None, Some("source")),
            ""
        );
    }

    #[test]
    fn access_scene_query_available_while_build_route_query_empty() {
        assert_eq!(
            route_query(UiRouteMode::Build, Some("dataset-foo"), None, None),
            ""
        );
        assert_eq!(
            access_scene_query(Some("dataset-foo")),
            "/scene/dataset-foo"
        );
    }

    #[test]
    fn route_query_encodes_scene_value() {
        assert_eq!(
            route_query(UiRouteMode::App, Some("中文 场景"), None, None),
            "/scene/%E4%B8%AD%E6%96%87%20%E5%9C%BA%E6%99%AF"
        );
        assert_eq!(encode_query_value("README #1.md"), "README%20%231.md");
    }

    #[test]
    fn manage_tab_href_encodes_file_value() {
        assert_eq!(
            manage_tab_href(
                "examples/demo",
                Some("docs/README #1.md"),
                "docs/README #1.md",
                false,
                ManageViewTab::Source,
                None,
            ),
            "/apps/build/examples/demo?file=docs%2FREADME%20%231.md&tab=source"
        );
    }
}
