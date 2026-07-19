use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};

use super::access_ai_entry::access_ai_floating_entry;
use super::compile_status::is_static_workspace_asset_target;
use super::manage_routing::WorldSemanticQuery;
use super::preview;
use super::preview_chrome::asset_preview_body;
use super::route::UiRouteMode;
use super::scene_drilldown_context::host_ssr_bootstrap_scripts;

/// Preview surface HTML for revision-first thin shells (custom elements + scopes).
pub fn render_access_preview_surface_html(
    compiled: &CompiledApp,
    app_path: &str,
    file_target: Option<&str>,
    route_mode: UiRouteMode,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
) -> String {
    let current_target = file_target
        .filter(|t| !t.trim().is_empty())
        .unwrap_or(compiled.active_target_file.as_str());
    let static_asset = is_static_workspace_asset_target(current_target);
    let preview = if static_asset {
        asset_preview_body(app_path, current_target, "")
    } else {
        preview::preview_view(
            compiled,
            app_path,
            current_target,
            route_mode,
            WorldSemanticQuery::default(),
            None,
            None,
            data_mode,
            review_projection,
        )
    };
    preview.into_any().to_html()
}

/// Scene drilldown + host runtime capability scripts for thin-shell SSR.
pub fn render_host_ssr_bootstrap_html(
    compiled: &CompiledApp,
    app_path: &str,
    preview_scene_id: Option<&str>,
    data_mode: Option<&str>,
) -> String {
    host_ssr_bootstrap_scripts(compiled, app_path, preview_scene_id, data_mode).to_html()
}

/// Revision-only thin head: drilldown fetched via API; runtime capabilities inline.
pub fn render_host_ssr_bootstrap_head_revision_only(
    compiled: &CompiledApp,
    app_path: &str,
    app_id: &str,
    preview_scene_id: Option<&str>,
    data_mode: Option<&str>,
) -> String {
    super::scene_drilldown_context::render_host_ssr_bootstrap_head_revision_only(
        compiled,
        app_path,
        app_id,
        preview_scene_id,
        data_mode,
    )
}
use super::shell_preview_layout::{
    access_main_preview_class, access_preview_panel_class, access_shell_grid_class,
};
use super::statusbar::statusbar_view;
use super::topbar::{access_scene_for_topbar, topbar_view};
use super::{HostAccountView, TopbarMenuContext};

pub(crate) fn access_shell(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    selected_scene: Option<&str>,
    file_target: Option<&str>,
    source: Option<&str>,
    active_tab: Option<&str>,
    chrome_hidden: bool,
    upload_enabled: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
    admin_nav_items: &[super::topbar::AdminNavItem],
) -> AnyView {
    let current_target = file_target
        .filter(|t| !t.trim().is_empty())
        .unwrap_or(compiled.active_target_file.as_str());
    let static_asset = is_static_workspace_asset_target(current_target);
    let preview = if static_asset {
        asset_preview_body(app_path, current_target, source.unwrap_or(""))
    } else {
        preview::preview_view(
            compiled,
            app_path,
            current_target,
            UiRouteMode::App,
            WorldSemanticQuery::default(),
            None,
            None,
            data_mode,
            review_projection,
        )
    };
    let topbar_preview_target = if static_asset { None } else { file_target };
    let panel_tab = active_tab.unwrap_or("preview");
    let topbar_access_scene = access_scene_for_topbar(
        UiRouteMode::App,
        compiled,
        selected_scene,
        topbar_preview_target,
    );
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let topbar = topbar_view(
        apps,
        app_path,
        topbar_menu,
        UiRouteMode::App,
        topbar_access_scene,
        Some(current_target),
        active_tab,
        None,
        None,
        upload_enabled,
        stage_enabled,
        auth_enabled,
        auth_account,
        data_mode,
        review_projection,
        None,
        Some(compiled.scene_routes.as_slice()),
        None,
        admin_nav_items,
        None,
    );
    let statusbar = statusbar_view(app_path, UiRouteMode::App.slug(), current_target, None);
    let shell_class = access_shell_grid_class(chrome_hidden, stage_enabled);
    let main_class = access_main_preview_class(chrome_hidden, stage_enabled);
    let preview_panel_class = access_preview_panel_class(chrome_hidden, stage_enabled);
    let floating_entry = || access_ai_floating_entry(compiled, app_path, current_target, panel_tab);
    view! {
        <div class=shell_class>
            {host_ssr_bootstrap_scripts(
                compiled,
                app_path,
                selected_scene.or(compiled.active_scene.as_deref()),
                data_mode,
            )}
            {if chrome_hidden {
                view! { <></> }.into_any()
            } else {
                topbar
            }}
            <main class=main_class>
                {if chrome_hidden {
                    view! {
                        <>
                            <section class=preview_panel_class>
                                {preview}
                            </section>
                            {floating_entry()}
                        </>
                    }
                        .into_any()
                } else {
                    view! {
                        <>
                            <section class=preview_panel_class>
                                {preview}
                            </section>
                            {floating_entry()}
                        </>
                    }
                        .into_any()
                }}
            </main>
            {statusbar}
        </div>
    }
    .into_any()
}

/// Topbar + statusbar HTML for revision-first thin shells (host frame stability).
pub fn render_access_shell_chrome_html(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    route_mode: UiRouteMode,
    selected_scene: Option<&str>,
    file_target: Option<&str>,
    active_tab: Option<&str>,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
    chrome_hidden: bool,
    admin_nav_items: &[super::topbar::AdminNavItem],
    admin_active_id: Option<&str>,
) -> (String, String) {
    let current_target = file_target
        .filter(|t| !t.trim().is_empty())
        .unwrap_or(compiled.active_target_file.as_str());
    let topbar_access_scene =
        access_scene_for_topbar(route_mode, compiled, selected_scene, file_target);
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let topbar = (!chrome_hidden).then(|| {
        topbar_view(
            apps,
            app_path,
            topbar_menu,
            route_mode,
            topbar_access_scene,
            Some(current_target),
            active_tab,
            None,
            None,
            false,
            stage_enabled,
            auth_enabled,
            auth_account,
            data_mode,
            review_projection,
            None,
            Some(compiled.scene_routes.as_slice()),
            None,
            admin_nav_items,
            admin_active_id,
        )
    });
    let statusbar = statusbar_view(app_path, route_mode.slug(), current_target, None);
    (
        topbar.map(|view| view.to_html()).unwrap_or_default(),
        statusbar.to_html(),
    )
}
