use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};
use serde::{Deserialize, Serialize};

mod compile_status;
mod manage_routing;
mod opencode;
mod preview;
mod preview_chrome;
mod route;
mod shell_access;
mod shell_manage;
mod source_tree;
mod statusbar;
mod topbar;

pub use route::UiRouteMode;

use preview_chrome::{chrome_scripts_view, component_scripts};
use shell_access::access_shell;
use shell_manage::manage_shell;

#[derive(Debug, Clone)]
pub struct SourcePanelMeta {
    pub line_count: usize,
    pub char_count: usize,
    pub last_modified_label: Option<String>,
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

pub fn render_page(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    topbar_menu_config: Option<&TopbarMenuConfig>,
    route_mode: UiRouteMode,
    target: Option<&str>,
    source: Option<&str>,
    source_meta: Option<&SourcePanelMeta>,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
    chrome_hidden: bool,
) -> String {
    let shell_mode_class = if route_mode == UiRouteMode::Access && chrome_hidden {
        "access-mode chrome-none"
    } else if route_mode == UiRouteMode::Access {
        "access-mode"
    } else {
        "manage-mode"
    };
    let body_class = format!("{shell_mode_class} sl-theme-dark");
    let shell = match route_mode {
        UiRouteMode::Access => access_shell(
            apps,
            compiled,
            app_path,
            topbar_menu_config,
            selected_entry,
            preview_target,
            active_tab,
            chrome_hidden,
        ),
        UiRouteMode::Manage => manage_shell(
            apps,
            compiled,
            app_path,
            topbar_menu_config,
            target,
            source,
            source_meta,
            selected_entry,
            preview_target,
            active_tab,
        ),
    };
    let chrome_scripts = chrome_scripts_view(route_mode);

    let page = view! {
        <html lang="zh-CN">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <title>{format!("{} - MeiLang", compiled.title)}</title>
                <link rel="icon" href="/app-assets/favicon.svg" type="image/svg+xml"/>
                <link rel="stylesheet" href="/app-assets/app-shell.css"/>
                <link rel="stylesheet" href="/app-assets/tailwind.css"/>
                <link rel="stylesheet" href="/app-assets/vendor/codemirror.css"/>
                <link rel="stylesheet" href="/app-assets/vendor/codemirror-merge.css"/>
                <link
                    rel="stylesheet"
                    href="https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/themes/dark.css"
                />
                <script
                    type="module"
                    src="https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/shoelace-autoloader.js"
                ></script>
            </head>
            <body class=body_class>
                {shell}
                {component_scripts(compiled)}
                {chrome_scripts}
                <script src="/app-assets/spa-navigation.js"></script>
            </body>
        </html>
    };
    page.to_html()
}

#[cfg(test)]
mod tests {
    use super::manage_routing::{manage_view_tab_from_query, route_query, ManageViewTab};

    #[test]
    fn manage_defaults_to_diagnostics_when_errors_exist() {
        assert!(matches!(
            manage_view_tab_from_query(None, true, true),
            ManageViewTab::Diagnostics
        ));
    }

    #[test]
    fn manage_respects_explicit_preview_tab_even_when_errors_exist() {
        assert!(matches!(
            manage_view_tab_from_query(Some("preview"), true, true),
            ManageViewTab::Preview
        ));
    }

    #[test]
    fn route_query_omits_tab_for_cross_app_navigation() {
        assert_eq!(route_query(None, None, Some("source")), "");
        assert_eq!(
            route_query(None, Some("main.mei"), Some("diagnostics")),
            "?preview_target=main.mei"
        );
    }
}
