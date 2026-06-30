use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

mod access_ai_entry;
mod agent_panel;
mod build_tree;
mod capabilities;
mod compile_status;
mod document;
mod manage_routing;
mod preview;
mod preview_chrome;
mod route;
mod scene_drilldown_context;
mod shell_access;
mod shell_config;
mod shell_manage;
mod shell_presentation;
mod shell_speaker;
mod shell_preview_layout;
mod shell_runtime;
mod shell_upload;
mod runtime_panels;
mod runtime_snapshot_view;
mod runtime_tree;
mod source_tree;
mod statusbar;
mod topbar;
mod view_routing;

pub use capabilities::HostCapabilities;
pub use route::UiRouteMode;
pub use shell_upload::UploadFileEntry;

use preview_chrome::{component_script_preloads, component_scripts};
use shell_access::access_shell;
use shell_config::config_shell;
use shell_manage::manage_shell;
use shell_presentation::presentation_shell;
use shell_speaker::speaker_shell;
use shell_upload::upload_shell;

pub use preview::{
    default_shell_body_theme_style, page_body_theme_style, scene_theme_style_for_theme_id,
    scene_viewport_theme_style,
    shell_body_theme_style,
};
pub use shell_manage::{render_build_preview_fragment, BuildPreviewFragment};
pub use topbar::load_topbar_menu_context;
use shell_runtime::runtime_shell;

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stock_component_packs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stock_template_packs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock_catalog_app_id: Option<String>,
    /// `stock.catalogApp.title`（如「组件库」），组件库浏览时顶栏面包屑首段。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock_catalog_app_title: Option<String>,
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
    /// When set, the item appears only in listed route modes (e.g. `["build"]`).
    #[serde(default)]
    pub modes: Vec<String>,
    /// Stock catalog split entry: `components` | `templates` (same app_id, distinct topbar tabs).
    #[serde(default)]
    pub catalog: Option<String>,
    /// Stock pack within catalog facet (component `pack_path` or template top folder).
    #[serde(default)]
    pub pack: Option<String>,
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
    world_metric: Option<&str>,
    world_dataset: Option<&str>,
    explain: Option<&str>,
    node: Option<&str>,
    scope: Option<&str>,
    focus: Option<&str>,
    catalog: Option<&str>,
    stock_pack: Option<&str>,
    chrome_hidden: bool,
    upload_enabled: bool,
    upload_root_label: Option<&str>,
    upload_files: &[UploadFileEntry],
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    scene_component_bundle_url: Option<&str>,
    shell_body_theme_style: &str,
    runtime_roots: Option<&[mei_lang_kernel::ReachabilityTreeRoot]>,
    runtime_snapshot_json: Option<&str>,
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
        UiRouteMode::Run => presentation_shell(
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
        UiRouteMode::Speaker => speaker_shell(
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
            world_metric,
            world_dataset,
            explain,
            node,
            scope,
            focus,
            catalog,
            stock_pack,
            upload_enabled,
            auth_enabled,
            auth_account,
        ),
        UiRouteMode::Runtime => runtime_shell(
            apps,
            compiled,
            app_path,
            topbar_menu,
            runtime_roots.unwrap_or(&[]),
            node,
            active_tab,
            runtime_snapshot_json,
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
        component_script_preloads(compiled, scene_component_bundle_url),
        component_scripts(compiled, scene_component_bundle_url).into_any(),
        auth_enabled,
        auth_account,
        shell_body_theme_style,
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
    shell_body_theme_style: &str,
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
        view! { <></> }.into_any(),
        auth_enabled,
        auth_account,
        shell_body_theme_style,
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
    shell_body_theme_style: &str,
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
        view! { <></> }.into_any(),
        auth_enabled,
        auth_account,
        shell_body_theme_style,
    )
}

#[cfg(test)]
mod tests {
    use super::manage_routing::{
        access_scene_query, build_preview_href, encode_query_value, manage_tab_href,
        resolve_build_query, route_query, WorldSemanticQuery, OPS_CONFIG_TARGET,
    };
    use super::view_routing::{build_href_with_catalog, config_href};
    use super::UiRouteMode;
    use mei_lang_kernel::BuildViewTab;

    #[test]
    fn build_view_defaults_to_overview_for_scene_node() {
        assert!(matches!(
            resolve_build_query(Some("scene:home"), None, None, None, None, None, None, None,)
                .map(|resolved| resolved.tab),
            Some(BuildViewTab::Overview)
        ));
    }

    #[test]
    fn build_href_uses_build_route() {
        assert_eq!(
            build_href_with_catalog("zhifa", Some("main.mei"), Some("preview"), None, None),
            "/apps/build/zhifa?file=main.mei&tab=preview"
        );
        assert_eq!(config_href("zhifa"), "/apps/config/zhifa");
    }

    #[test]
    fn manage_ops_config_href_forces_preview_tab() {
        let href = manage_tab_href(
            "zhifa",
            Some(OPS_CONFIG_TARGET),
            OPS_CONFIG_TARGET,
            false,
            BuildViewTab::Overview,
            Some("all"),
            None,
            WorldSemanticQuery::default(),
        );
        assert!(href.contains("/apps/build/zhifa"));
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
    fn build_preview_href_includes_scene_export_selector() {
        let href = build_preview_href(
            "zhifa",
            Some("scenes/05-监督预警.board.mei"),
            Some("warnings_analytics_board"),
            Some("preview"),
            None,
            WorldSemanticQuery::default(),
        );
        assert!(href.contains("node=scene%3Awarnings_analytics_board"));
        assert!(href.contains("tab=preview"));
    }

    #[test]
    fn manage_tab_href_encodes_file_value() {
        let href = manage_tab_href(
            "examples/demo",
            Some("docs/README #1.md"),
            "docs/README #1.md",
            false,
            BuildViewTab::Overview,
            None,
            None,
            WorldSemanticQuery::default(),
        );
        assert!(href.contains("/apps/build/examples/demo"));
        assert!(href.contains("node="));
    }

    #[test]
    fn build_preview_href_includes_world_semantic_query() {
        let href = build_preview_href(
            "zhifa",
            Some("scenes/07-问题办理.world.mei"),
            None,
            Some("preview"),
            None,
            WorldSemanticQuery {
                world_metric: Some("warnings_pending_count"),
                world_dataset: None,
                explain: Some("composition_by_category"),
            },
        );
        assert!(href.contains("node=world-explain"));
        assert!(href.contains("tab=preview"));
    }
}
