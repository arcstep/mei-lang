use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta, WorkspaceNode};
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

mod agent_panel;
mod capabilities;
mod compile_status;
mod document;
mod manage_routing;
mod preview;
mod preview_chrome;
mod route;
mod shell_access;
mod shell_config;
mod shell_manage;
mod shell_presentation;
mod shell_upload;
mod source_tree;
mod statusbar;
mod topbar;
mod view_routing;

pub use capabilities::HostCapabilities;
pub use route::UiRouteMode;
pub use shell_upload::UploadFileEntry;

use preview_chrome::component_scripts;
use shell_access::access_shell;
use shell_config::config_shell;
use shell_manage::{manage_shell, manage_source_shell};
use shell_presentation::presentation_shell;
use shell_upload::upload_shell;

use document::render_document;

#[derive(Debug, Clone, Serialize)]
pub struct SourcePanelMeta {
    pub line_count: usize,
    pub char_count: usize,
    pub last_modified_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostAccountView {
    #[serde(default)]
    pub logged_in: bool,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub capabilities: HostCapabilities,
}

impl Default for HostAccountView {
    fn default() -> Self {
        Self {
            logged_in: false,
            username: String::new(),
            profile: String::new(),
            role: String::new(),
            capabilities: HostCapabilities::auth_disabled(),
        }
    }
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
    /// `.mei-workspace.json#workspace.label`（无则回退 id /「工作区」），用于顶栏应用面包屑。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_label: Option<String>,
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
        UiRouteMode::Presentation => presentation_shell(
            apps,
            compiled,
            app_path,
            topbar_menu,
            selected_scene,
            target,
            source,
            active_tab,
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
            build_href("zhifa", Some("main.mei"), Some("preview")),
            "/apps/build/zhifa?file=main.mei&tab=preview"
        );
        assert_eq!(config_href("zhifa"), "/apps/config/zhifa");
    }

    #[test]
    fn manage_ops_config_href_forces_preview_tab() {
        assert_eq!(
            manage_tab_href(
                "zhifa",
                Some(OPS_CONFIG_TARGET),
                OPS_CONFIG_TARGET,
                false,
                ManageViewTab::Diagnostics,
                Some("all")
            ),
            "/apps/build/zhifa?file=.mei-config.json&tab=preview"
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
