use leptos::prelude::*;
use mei_lang_kernel::{CompiledSceneRoute, WorkspaceAppMeta};

use super::route::UiRouteMode;
use super::shell_upload::{upload_workbench_view, UploadFileEntry};
use super::statusbar::statusbar_view;
use super::topbar::{topbar_view, AdminNavItem};
use super::view_routing::{app_access_href, config_href, host_upload_href};
use super::{HostAccountView, TopbarMenuContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminMainSurface {
    FormCard,
    OpsEmbed,
    UploadEmbed,
    AssetSlotCollection,
}

pub struct AdminUploadEmbed<'a> {
    pub upload_root_label: &'a str,
    pub files: &'a [UploadFileEntry],
    pub selected_file: Option<&'a str>,
}

pub fn admin_shell(
    apps: &[WorkspaceAppMeta],
    app_title: &str,
    app_path: &str,
    resource_id: &str,
    resource_title: Option<&str>,
    topbar_menu: Option<&TopbarMenuContext>,
    admin_nav_items: &[AdminNavItem],
    admin_active_id: Option<&str>,
    resource_json: &str,
    surface: AdminMainSurface,
    upload_embed: Option<AdminUploadEmbed<'_>>,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
    // Stage Registry candidates for topbar Stage strip (0544); hrefs always `/apps/...`.
    access_stage_routes: &[CompiledSceneRoute],
    // Default Access stage — standalone-open only; Admin does not mark Stage chips active.
    access_scene: Option<&str>,
    source_anchor: &str,
    projection_digest: &str,
) -> AnyView {
    let stage_routes = if access_stage_routes.is_empty() {
        None
    } else {
        Some(access_stage_routes)
    };
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
        stage_routes,
        None,
        admin_nav_items,
        admin_active_id,
    );
    let title = resource_title.unwrap_or(resource_id);
    let back_href = app_access_href(app_path);
    let crumb = format!("{app_title} / {title}");
    let statusbar = statusbar_view(app_path, UiRouteMode::Admin.slug(), title, None);
    let resource_attr = resource_json.to_string();
    let app_attr = app_path.to_string();
    let rid_attr = resource_id.to_string();
    let source_anchor_attr = source_anchor.to_string();
    let projection_digest_attr = projection_digest.to_string();
    let legacy_config = config_href(app_path);
    let legacy_upload = host_upload_href(Some(app_path), None);
    let admin_upload = format!("/admin/apps/{app_path}/upload_files");

    let kit_compose = |kit: AnyView| {
        view! {
            <div
                id="mei-admin-compose-root"
                class="mei-admin-compose-root min-h-0 flex flex-1 flex-col overflow-hidden"
                data-mei-admin-compose="document"
                data-mei-admin-surface="document"
                data-mei-resource-id=rid_attr.clone()
                data-mei-source-anchor=source_anchor_attr.clone()
                data-mei-projection-digest=projection_digest_attr.clone()
            >
                {kit}
            </div>
        }
        .into_any()
    };

    let main = match surface {
        AdminMainSurface::FormCard => kit_compose(
            view! {
                <section
                    id="admin-form-root"
                    class="admin-form-shell admin-kit-detail admin-kit-detail--scroll"
                    data-app-id=app_attr.clone()
                    data-resource-id=rid_attr.clone()
                    data-admin-resource=resource_attr.clone()
                ></section>
            }
            .into_any(),
        ),
        AdminMainSurface::OpsEmbed => view! {
            <div class="admin-kit-detail">
                <div class="admin-kit-banner">
                    <strong>"运维配置"</strong>
                    <span>"编辑当前应用根目录 `.mei-config.json`；运维写回仅允许 `ops.*` 白名单字段。"</span>
                    <span class="mei-text-muted">"|"</span>
                    <a class="mei-text-link" href=admin_upload.clone()>"上传物料"</a>
                    <span class="mei-text-muted">"|"</span>
                    <a class="mei-text-link" href=legacy_config.clone()>"旧入口 /config"</a>
                </div>
                <div class="admin-kit-embed">
                    <section
                        id="manage-ops-editor-root"
                        class="manage-ops-editor-shell flex min-h-0 flex-1 flex-col overflow-hidden"
                        data-app-id=app_attr.clone()
                    ></section>
                </div>
            </div>
        }
        .into_any(),
        AdminMainSurface::UploadEmbed => {
            let embed = upload_embed.expect("upload embed context");
            let ops_href = format!("/admin/apps/{app_path}/ops_config");
            let workbench = upload_workbench_view(
                app_path,
                embed.upload_root_label,
                embed.files,
                embed.selected_file,
                ops_href.as_str(),
            );
            view! {
                <div class="admin-kit-detail">
                    <div class="admin-kit-banner">
                        <strong>"上传物料"</strong>
                        <span>"管理应用 upload 目录；写路径仍为 `/api/upload/*`。"</span>
                        <span class="mei-text-muted">"|"</span>
                        <a class="mei-text-link" href=legacy_upload.clone()>"旧入口 /upload"</a>
                    </div>
                    <div
                        id="tree-icons-sprite-root"
                        class="pointer-events-none absolute left-0 top-0 -z-10 h-0 w-0 overflow-hidden opacity-0"
                        aria-hidden="true"
                        inner_html=super::source_tree::TREE_ICONS_SPRITE_SVG
                    ></div>
                    <div class="admin-kit-embed">
                        <div class="admin-upload-embed flex min-h-0 flex-1 flex-col overflow-hidden">
                            {workbench}
                        </div>
                    </div>
                </div>
            }
            .into_any()
        }
        AdminMainSurface::AssetSlotCollection => kit_compose(
            view! {
                <section
                    id="admin-asset-slot-root"
                    class="admin-asset-slot-shell admin-kit-detail"
                    data-app-id=app_attr.clone()
                    data-resource-id=rid_attr.clone()
                    data-admin-resource=resource_attr.clone()
                ></section>
            }
            .into_any(),
        ),
    };

    view! {
        <div class="shell shell-surface admin-view-shell mei-text-primary">
            <div id="mei-host-topbar-slot" class="mei-host-chrome-slot" data-mei-host-chrome="top">{topbar}</div>
            <main
                class="admin-view-main chrome-inset min-h-0 flex flex-1 flex-col overflow-hidden px-4 py-3"
                data-mei-stage-surface="document"
                data-mei-page-program=rid_attr.clone()
                data-mei-page-surface="document"
                data-mei-resource-id=rid_attr.clone()
                data-mei-source-anchor=source_anchor_attr.clone()
                data-mei-projection-digest=projection_digest_attr.clone()
            >
                <div class="admin-kit-banner flex flex-wrap items-center gap-3">
                    <nav class="admin-breadcrumb min-w-0 flex-1" aria-label="管理面包屑">
                        <strong class="mei-text-inverse">{crumb}</strong>
                    </nav>
                    <a class="mei-text-link shrink-0" href=back_href>"返回应用"</a>
                </div>
                {main}
            </main>
            <div id="mei-host-statusbar-slot" class="mei-host-chrome-slot" data-mei-host-chrome="bottom">{statusbar}</div>
        </div>
    }
    .into_any()
}
