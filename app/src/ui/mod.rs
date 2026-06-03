use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

mod agent_panel;
mod compile_status;
mod manage_routing;
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

/// 顶栏菜单：优先按 app id 首段匹配 `<source_root>/<segment>/.mei-config.json` 的 `menu`；若无则回退 `<segment>/_menu.json`；再回退 `<source_root>/.mei-config.json` / `<source_root>/_menu.json`（用于 `source_root` 直接指向 `examples` 等子树时的整段配置）。
#[derive(Debug, Clone, Default)]
pub struct TopbarMenuContext {
    /// `source_root/.mei-config.json`（`menu`）或 `source_root/_menu.json`，对该 `source_root` 下所有应用生效（在首段无专用配置时作为回退）。
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
            topbar_menu,
            selected_scene,
            preview_target,
            active_tab,
            chrome_hidden,
        ),
        UiRouteMode::Manage => manage_shell(
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
        ),
    };
    let chrome_scripts = chrome_scripts_view(route_mode);

    let manage_timing_meta = match route_mode {
        UiRouteMode::Manage => view! {
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
                <title>{format!("{} - MeiLang", compiled.title)}</title>
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
                data-mei-handler-html-ready-ms="__MEI_HANDLER_HTML_READY_MS__"
                data-mei-ssr-http-response-body-ms="__MEI_SSR_HTTP_BODY_MS__"
            >
                {shell}
                {component_scripts(compiled)}
                {chrome_scripts}
            </body>
        </html>
    };
    page.to_html()
}

#[cfg(test)]
mod tests {
    use super::manage_routing::{
        access_scene_query, encode_query_value, manage_tab_href, manage_view_tab_from_query,
        route_query, ManageViewTab,
    };
    use super::UiRouteMode;

    #[test]
    fn manage_defaults_to_diagnostics_when_errors_exist() {
        assert!(matches!(
            manage_view_tab_from_query(None, true, true, 1, "main.mei"),
            ManageViewTab::Diagnostics
        ));
    }

    #[test]
    fn manage_respects_explicit_preview_tab_even_when_errors_exist() {
        assert!(matches!(
            manage_view_tab_from_query(Some("preview"), true, true, 1, "main.mei"),
            ManageViewTab::Preview
        ));
    }

    #[test]
    fn manage_asset_dual_allows_source_tab() {
        assert!(matches!(
            manage_view_tab_from_query(Some("source"), false, false, 0, "readme.md"),
            ManageViewTab::Source
        ));
    }

    #[test]
    fn manage_world_capsule_defaults_to_source_tab() {
        assert!(matches!(
            manage_view_tab_from_query(None, true, true, 2, "scenes/foo.world.mei"),
            ManageViewTab::Source
        ));
    }

    #[test]
    fn route_query_omits_tab_for_cross_app_navigation() {
        assert_eq!(
            route_query(UiRouteMode::Manage, None, None, Some("source")),
            ""
        );
        assert_eq!(
            route_query(
                UiRouteMode::Manage,
                None,
                Some("main.mei"),
                Some("diagnostics")
            ),
            ""
        );
    }

    #[test]
    fn access_scene_query_available_while_manage_route_query_empty() {
        assert_eq!(
            route_query(UiRouteMode::Manage, Some("dataset-foo"), None, None),
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
            route_query(UiRouteMode::Access, Some("中文 场景"), None, None),
            "/scene/%E4%B8%AD%E6%96%87%20%E5%9C%BA%E6%99%AF"
        );
        assert_eq!(
            access_scene_query(Some("中文 场景")),
            "/scene/%E4%B8%AD%E6%96%87%20%E5%9C%BA%E6%99%AF"
        );
        assert_eq!(encode_query_value("README #1.md"), "README%20%231.md");
    }

    #[test]
    fn route_query_access_includes_tab_in_query() {
        assert_eq!(
            route_query(UiRouteMode::Access, Some("home"), None, Some("source")),
            "/scene/home?tab=source"
        );
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
            "/apps/manage/examples/demo?file=docs%2FREADME%20%231.md&tab=source"
        );
    }
}
