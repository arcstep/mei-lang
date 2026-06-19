use leptos::prelude::*;
use mei_lang_kernel::{WorkspaceAppMeta, WorkspaceNode};

use super::super::compile_status::{
    classify_asset_shell, codemirror_dataset_lang, is_mei_script_target, AssetShellKind,
};
use super::super::manage_routing::{manage_tab_href, manage_view_tab_from_query, ManageViewTab, WorldSemanticQuery};
use super::super::preview_chrome::asset_preview_body;
use super::super::route::UiRouteMode;
use super::super::source_tree;
use super::super::statusbar::statusbar_view;
use super::super::topbar::topbar_view;
use super::super::{HostAccountView, SourcePanelMeta, TopbarMenuContext};

fn asset_codemirror_stack(
    app_path: &str,
    target: &str,
    source: &str,
    cm_lang: &'static str,
) -> impl IntoView {
    view! {
        <div class="main-pane-scroll source-pane-scroll flex min-h-0 flex-1 flex-col overflow-auto">
            <div
                id="asset-source-editor-host"
                class="source-editor-host asset-source-editor-host min-h-[12rem] flex-1"
                data-app-path=app_path.to_string()
                data-source-target=target.to_string()
            ></div>
            <pre
                id="asset-source-raw"
                hidden
                data-source-target=target.to_string()
                data-source-lang=cm_lang
            >{source.to_string()}</pre>
        </div>
    }
}

fn readonly_source_notice() -> impl IntoView {
    view! {
        <div class="manage-readonly-note mb-2 rounded-lg border mei-border-default mei-surface-panel-muted px-3 py-2 text-[11px] leading-5 mei-text-body">
            <strong class="mr-2 mei-text-inverse">"只读查看"</strong>
            <span>"构建视图中的 `.mei` 与资源文件仅用于预览和只读源码查看；应用配置请切换到「配置」视图。"</span>
        </div>
    }
}
pub(crate) fn manage_source_shell(
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
) -> AnyView {
    let selected_target = target.to_string();
    let source_panel = source.to_string();
    let script_target = is_mei_script_target(selected_target.as_str());
    let asset_shell = classify_asset_shell(selected_target.as_str());
    let asset_cm_lang = codemirror_dataset_lang(selected_target.as_str());
    let source_tree = source_tree::source_tree_view(
        file_tree,
        UiRouteMode::Build,
        app_path,
        selected_target.as_str(),
        selected_scene,
        &[],
        selected_target.as_str(),
        active_tab,
        WorldSemanticQuery::default(),
    );
    let active_manage_tab = manage_view_tab_from_query(
        active_tab,
        script_target,
        false,
        0,
        selected_target.as_str(),
        WorldSemanticQuery::default(),
    );
    let topbar = topbar_view(
        apps,
        app_path,
        topbar_menu,
        UiRouteMode::Build,
        selected_scene,
        Some(selected_target.as_str()),
        active_tab,
        upload_enabled,
        false,
        auth_enabled,
        auth_account,
    );
    let statusbar = statusbar_view(
        app_path,
        app_title,
        UiRouteMode::Build.slug(),
        selected_target.as_str(),
        source_meta,
        None,
        false,
        false,
        selected_scene,
    );
    let tab_specs: Vec<ManageViewTab> = if script_target || asset_shell == AssetShellKind::Dual {
        vec![ManageViewTab::Preview, ManageViewTab::Source]
    } else {
        vec![]
    };
    let tab_links = tab_specs
        .into_iter()
        .map(|tab| {
            let href = manage_tab_href(
                app_path,
                Some(selected_target.as_str()),
                selected_target.as_str(),
                script_target,
                tab,
                None,
                selected_scene,
                WorldSemanticQuery::default(),
            );
            let class = if tab == active_manage_tab {
                "manage-view-tab is-active"
            } else {
                "manage-view-tab"
            };
            let label = match tab {
                ManageViewTab::Preview => "预览",
                ManageViewTab::Source => "只读源码",
                ManageViewTab::Diagnostics => "调试",
            };
            view! {
                <a
                    class=class
                    href=href
                    role="tab"
                    aria-selected=if tab == active_manage_tab { "true" } else { "false" }
                    data-manage-tab=tab.slug()
                >
                    <span class="manage-view-tab-label">{label}</span>
                </a>
            }
        })
        .collect_view();
    let main_tabs_nav = if script_target || asset_shell == AssetShellKind::Dual {
        view! {
            <nav
                class="manage-view-tabs workspace-tabs-strip mb-3 flex min-w-0 flex-wrap items-center gap-2 pb-2.5"
                role="tablist"
                aria-label="构建主视图"
            >
                <div class="manage-view-tabs-cluster">
                    <div class="manage-view-tabs-group" role="presentation">
                        {tab_links}
                    </div>
                </div>
            </nav>
        }
        .into_any()
    } else {
        view! { <></> }.into_any()
    };
    let main_panel = match asset_shell {
        AssetShellKind::PreviewOnly | AssetShellKind::Unsupported => view! {
            <section class="asset-preview-pane flex min-h-0 flex-1 flex-col overflow-hidden">
                {asset_preview_body(
                    app_path,
                    selected_target.as_str(),
                    source_panel.as_str(),
                )}
            </section>
        }
        .into_any(),
        _ => view! {
            <section class="source-panel source-pane min-w-0 min-h-0 flex flex-1 flex-col overflow-hidden">
                {readonly_source_notice()}
                {asset_codemirror_stack(
                    app_path,
                    selected_target.as_str(),
                    source_panel.as_str(),
                    asset_cm_lang,
                )}
            </section>
        }
        .into_any(),
    };
    view! {
        <div class="shell shell-surface mei-text-primary">
            <div
                id="tree-icons-sprite-root"
                class="pointer-events-none absolute left-0 top-0 -z-10 h-0 w-0 overflow-hidden opacity-0"
                aria-hidden="true"
                inner_html=source_tree::TREE_ICONS_SPRITE_SVG
            ></div>
            {topbar}
            <div
                class="workspace manage-workspace chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0"
                id="workspace-root"
            >
                <aside class="sidebar left workspace-panel workspace-panel-side workspace-panel-nav h-full min-h-0 min-w-0 overflow-hidden flex flex-col px-4 py-2.5">
                    <div class="sidebar-scroll flex-1 min-h-0 overflow-auto">
                        {source_tree}
                    </div>
                </aside>
                <div
                    class="splitter splitter-left"
                    data-workspace-splitter="left"
                    role="separator"
                    aria-orientation="vertical"
                    aria-label="调整左侧资源栏宽度"
                >
                    <button
                        class="splitter-toggle"
                        type="button"
                        data-workspace-toggle="left"
                        aria-label="折叠左侧资源栏"
                        title="折叠左侧资源栏"
                    >
                        <span class="splitter-toggle-icon" aria-hidden="true">
                            <svg
                                viewBox="0 0 20 20"
                                width="12"
                                height="12"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1.8"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                            >
                                <path d="M12.5 4.5L7.5 10l5 5.5"></path>
                            </svg>
                        </span>
                    </button>
                </div>
                <main class="main min-w-0 min-h-0 overflow-hidden px-0">
                    <section class="main-pane workspace-panel workspace-panel-main min-w-0 min-h-0 flex h-full flex-col overflow-hidden px-2 py-3.5">
                        {main_tabs_nav}
                        {main_panel}
                    </section>
                </main>
            </div>
            {statusbar}
        </div>
    }
    .into_any()
}
