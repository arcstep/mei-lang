use leptos::prelude::*;
use mei_lang_kernel::{CompiledSceneRoute, WorkspaceAppMeta};

use super::route::UiRouteMode;
use super::statusbar::statusbar_view;
use super::topbar::{topbar_view, AdminNavItem};
use super::view_routing::app_access_href;
use super::{HostAccountView, TopbarMenuContext};

#[allow(clippy::too_many_arguments)]
pub fn admin_shell(
    apps: &[WorkspaceAppMeta],
    app_title: &str,
    app_path: &str,
    resource_id: &str,
    module_id: &str,
    resource_title: Option<&str>,
    topbar_menu: Option<&TopbarMenuContext>,
    admin_nav_items: &[AdminNavItem],
    admin_active_id: Option<&str>,
    scene_id: &str,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    access_stage_routes: &[CompiledSceneRoute],
    access_scene: Option<&str>,
    source_anchor: &str,
    registry_digest: &str,
    structure_digest: &str,
) -> AnyView {
    let topbar = topbar_view(
        apps,
        app_path,
        topbar_menu,
        UiRouteMode::Admin,
        access_scene,
        None,
        None,
        None,
        None,
        true,
        false,
        auth_enabled,
        auth_account,
        None,
        None,
        None,
        (!access_stage_routes.is_empty()).then_some(access_stage_routes),
        None,
        admin_nav_items,
        admin_active_id,
    );
    let title = resource_title.unwrap_or(resource_id);
    let crumb = format!("{app_title} / {title}");
    let statusbar = statusbar_view(app_path, UiRouteMode::Admin.slug(), title, None);

    view! {
        <div class="shell shell-surface admin-view-shell mei-text-primary">
            <div id="mei-host-topbar-slot" class="mei-host-chrome-slot" data-mei-host-chrome="top">
                {topbar}
            </div>
            <main
                id="mei-view-host"
                class="admin-view-main chrome-inset min-h-0 flex flex-1 flex-col overflow-hidden"
                data-mei-stage-surface="document"
                data-mei-admin-entry="v2"
                data-app-id=app_path.to_string()
                data-scene-id=scene_id.to_string()
                data-resource-id=resource_id.to_string()
                data-module-id=module_id.to_string()
                data-source-anchor=source_anchor.to_string()
                data-admin-registry-digest=registry_digest.to_string()
                data-page-structure-digest=structure_digest.to_string()
            >
                <div class="admin-breadcrumb flex shrink-0 items-center gap-3 px-4 py-3">
                    <strong class="mei-text-inverse">{crumb}</strong>
                    <a class="mei-text-link ml-auto" href=app_access_href(app_path)>"返回应用"</a>
                </div>
                <div
                    id="mei-compose-root"
                    class="preview-pane-scroll min-h-0 flex-1 overflow-hidden"
                    data-mei-compose-root="admin"
                    data-route-mode="admin"
                    data-app-id=app_path.to_string()
                    data-scene-id=scene_id.to_string()
                ></div>
                <p id="mei-thin-shell-fallback" class="mei-view-loading-overlay" hidden>
                    "正在装配 Admin PagePack…"
                </p>
            </main>
            <div
                id="mei-host-statusbar-slot"
                class="mei-host-chrome-slot"
                data-mei-host-chrome="bottom"
            >
                {statusbar}
            </div>
        </div>
    }
    .into_any()
}
