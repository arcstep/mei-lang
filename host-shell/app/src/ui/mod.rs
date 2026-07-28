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
pub mod prototype_preset;
mod route;
mod runtime_panels;
mod runtime_snapshot_view;
mod runtime_tree;
mod scene_drilldown_context;
mod shell_access;
mod shell_admin;
mod shell_copilot;
mod shell_manage;
mod shell_presentation;
mod shell_preview_layout;
mod shell_runtime;
mod shell_workspace;
mod source_tree;
mod statusbar;
mod topbar;
mod view_routing;

pub use capabilities::HostCapabilities;
pub use route::UiRouteMode;
pub use shell_workspace::{
    render_workspace_page, render_workspace_page_with_pending_app,
    render_workspace_shell_chrome_html, WorkspaceShellNav,
};
pub use topbar::AdminNavItem;
pub use view_routing::mcg_href;

use preview_chrome::{component_script_preloads, component_scripts};
pub use scene_drilldown_context::scene_drilldown_context_json_for_host_ssr;
use shell_access::access_shell;
pub use shell_access::{
    render_access_preview_surface_html, render_access_shell_chrome_html,
    render_host_ssr_bootstrap_head_revision_only, render_host_ssr_bootstrap_html,
};
use shell_admin::admin_shell;
use shell_copilot::copilot_shell;
use shell_manage::manage_shell;
use shell_presentation::presentation_shell;

pub use preview::{
    build_preview_runtime_context, default_shell_body_theme_style, page_body_theme_style,
    scene_theme_css_vars_for_theme_id, scene_theme_style_for_theme_id, scene_viewport_theme_style,
    shell_body_theme_style,
    PreviewRuntimeContext,
};
pub use shell_manage::{render_build_preview_fragment, BuildPreviewFragment};

#[derive(Debug, Clone, Serialize)]
pub struct UploadFileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: Option<u64>,
    pub modified_ms: Option<u64>,
    pub modified_label: Option<String>,
}
use shell_runtime::runtime_shell;
pub use topbar::load_topbar_menu_context;

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
    /// 顶栏品牌文案（`workspace.brand.title`；缺省 MeiLang）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand_title: Option<String>,
    /// 顶栏品牌 logo URL（已解析；缺省 `/app-assets/favicon.svg`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand_logo_href: Option<String>,
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
    _upload_root_label: Option<&str>,
    _upload_files: &[UploadFileEntry],
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    scene_component_bundle_url: Option<&str>,
    shell_body_theme_style: &str,
    runtime_roots: Option<&[mei_lang_kernel::ReachabilityTreeRoot]>,
    runtime_snapshot_json: Option<&str>,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
    data_mode_ceiling_notice: Option<&str>,
    tree_max_ui_role: Option<&str>,
    build_tree_mode: Option<&str>,
    admin_nav_items: &[AdminNavItem],
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
            data_mode,
            review_projection,
            admin_nav_items,
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
            data_mode,
            review_projection,
        ),
        UiRouteMode::Copilot => copilot_shell(
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
        UiRouteMode::Layout | UiRouteMode::Prototype => manage_shell(
            apps,
            compiled,
            app_path,
            topbar_menu,
            route_mode,
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
            data_mode,
            review_projection,
            data_mode_ceiling_notice,
            tree_max_ui_role,
            build_tree_mode,
            admin_nav_items,
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
        UiRouteMode::Config | UiRouteMode::Upload => {
            unreachable!("legacy config/upload pages were removed")
        }
        UiRouteMode::Admin => {
            // Admin surfaces use render_admin_page; CompiledApp path is unsupported.
            access_shell(
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
                data_mode,
                review_projection,
                admin_nav_items,
            )
        }
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
        None,
        None,
    )
}

pub fn render_admin_page(
    apps: &[WorkspaceAppMeta],
    app_title: &str,
    app_path: &str,
    resource_id: &str,
    module_id: &str,
    resource_title: Option<&str>,
    visible_body_html: Option<&str>,
    topbar_menu: Option<&TopbarMenuContext>,
    admin_nav_items: &[AdminNavItem],
    admin_active_id: Option<&str>,
    scene_id: &str,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    shell_body_theme_style: &str,
    access_stage_routes: &[mei_lang_kernel::CompiledSceneRoute],
    access_scene: Option<&str>,
    source_anchor: &str,
    projection_digest: &str,
    structure_digest: &str,
) -> String {
    let shell = admin_shell(
        apps,
        app_title,
        app_path,
        resource_id,
        module_id,
        resource_title,
        visible_body_html,
        topbar_menu,
        admin_nav_items,
        admin_active_id,
        scene_id,
        auth_enabled,
        auth_account,
        access_stage_routes,
        access_scene,
        source_anchor,
        projection_digest,
        structure_digest,
    );
    render_document(
        app_title,
        UiRouteMode::Admin,
        false,
        shell,
        view! { <></> }.into_any(),
        view! { <></> }.into_any(),
        auth_enabled,
        auth_account,
        shell_body_theme_style,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::manage_routing::{
        access_scene_query, build_preview_href, encode_query_value, manage_tab_href,
        resolve_build_query, route_query, BuildReviewAxes, WorldSemanticQuery, OPS_CONFIG_TARGET,
    };
    use super::view_routing::build_href_with_catalog;
    use super::UiRouteMode;
    use mei_lang_kernel::BuildViewTab;

    #[test]
    fn build_view_defaults_to_preview_for_scene_node() {
        assert!(matches!(
            resolve_build_query(Some("scene:home"), None, None, None, None, None, None, None,)
                .map(|resolved| resolved.tab),
            Some(BuildViewTab::Preview)
        ));
    }

    #[test]
    fn layout_href_seals_to_access_stage() {
        assert_eq!(
            build_href_with_catalog("zhifa", Some("main.mei"), Some("preview"), None, None),
            "/apps/zhifa/home"
        );
    }

    #[test]
    fn manage_ops_config_href_seals_to_access_home() {
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
        assert_eq!(href, "/apps/zhifa/home");
    }

    #[test]
    fn route_query_omits_tab_for_cross_app_navigation() {
        assert_eq!(
            route_query(UiRouteMode::Layout, None, None, Some("source")),
            ""
        );
    }

    #[test]
    fn access_scene_query_available_while_build_route_query_empty() {
        assert_eq!(
            route_query(UiRouteMode::Layout, Some("dataset-foo"), None, None),
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
    fn build_preview_href_seals_board_scene_to_access_home() {
        let href = build_preview_href(
            "zhifa",
            Some("scenes/05-监督预警.board.mei"),
            Some("warnings_analytics_board"),
            Some("preview"),
            None,
            WorldSemanticQuery::default(),
            BuildReviewAxes::default(),
        );
        assert_eq!(href, "/apps/zhifa/home");
    }

    #[test]
    fn manage_tab_href_seals_file_target_to_access_home() {
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
        assert_eq!(href, "/apps/examples/demo/home");
    }

    #[test]
    fn build_preview_href_seals_world_semantic_to_access_home() {
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
            BuildReviewAxes::default(),
        );
        assert_eq!(href, "/apps/zhifa/home");
        assert!(!href.contains('?'));
    }
}
