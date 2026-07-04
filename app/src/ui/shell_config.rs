use leptos::prelude::*;
use mei_lang_kernel::WorkspaceAppMeta;

use super::route::UiRouteMode;
use super::statusbar::statusbar_view;
use super::topbar::topbar_view;
use super::view_routing::upload_href;
use super::{HostAccountView, SourcePanelMeta, TopbarMenuContext};

fn ops_editor_main_view(app_path: &str) -> impl IntoView {
    view! {
        <section
            id="manage-ops-editor-root"
            class="manage-ops-editor-shell flex min-h-0 flex-1 flex-col overflow-hidden"
            data-app-id=app_path.to_string()
        ></section>
    }
}

pub(crate) fn config_shell(
    apps: &[WorkspaceAppMeta],
    _app_title: &str,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    upload_enabled: bool,
    access_scene: Option<&str>,
    _source_meta: Option<&SourcePanelMeta>,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> AnyView {
    let topbar = topbar_view(
        apps,
        app_path,
        topbar_menu,
        UiRouteMode::Config,
        access_scene,
        None,
        None,
        None,
        None,
        upload_enabled,
        false,
        auth_enabled,
        auth_account,
        None,
        None,
    );
    let statusbar = statusbar_view(
        app_path,
        UiRouteMode::Config.slug(),
        ".mei-config.json",
        None,
    );
    let data_link = upload_href(app_path, None);
    view! {
        <div class="shell shell-surface config-view-shell mei-text-primary">
            {topbar}
            <main class="config-view-main chrome-inset min-h-0 flex flex-1 flex-col overflow-hidden px-4 py-3">
                <div class="manage-readonly-note mb-3 rounded-lg border mei-border-default mei-surface-panel-muted px-3 py-2 mei-font-1 leading-5 mei-text-body">
                    <strong class="mr-2 mei-text-inverse">"应用配置"</strong>
                    <span>"编辑当前应用根目录 `.mei-config.json`；运维写回仅允许 `ops.*` 白名单字段。"</span>
                    <span class="mx-2 mei-text-muted">"|"</span>
                    <a class="mei-text-link" href=data_link>"上传物料"</a>
                </div>
                {ops_editor_main_view(app_path)}
            </main>
            {statusbar}
        </div>
    }
    .into_any()
}
