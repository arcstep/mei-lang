use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};

use super::super::compile_status::{
    classify_asset_shell, codemirror_dataset_lang, compiled_has_error_diagnostics,
    is_mei_script_target, visible_diagnostics_count, AssetShellKind, DiagnosticsFilterMode,
};
use super::super::manage_routing::{manage_tab_href, manage_view_tab_from_query, ManageViewTab, WorldSemanticQuery};
use super::super::preview;
use super::super::preview_chrome::{asset_preview_body, diagnostics_view};
use super::super::route::UiRouteMode;
use super::super::scene_drilldown_context::host_ssr_bootstrap_scripts;
use super::super::source_tree;
use super::super::statusbar::statusbar_view;
use super::super::topbar::{access_scene_for_topbar, topbar_view};
use super::super::{HostAccountView, SourcePanelMeta, TopbarMenuContext};
use super::world_semantic_inspector::{
    should_show_world_semantic_inspector, world_semantic_inspector_view,
};

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
        <div class="manage-readonly-note mb-2 rounded-lg border border-slate-700/55 bg-slate-900/45 px-3 py-2 text-[11px] leading-5 text-slate-300">
            <strong class="mr-2 text-slate-100">"只读查看"</strong>
            <span>"构建视图中的 `.mei` 与资源文件仅用于预览和只读源码查看；应用配置请切换到「配置」视图。"</span>
        </div>
    }
}
pub(crate) fn manage_shell(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
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
    upload_enabled: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> AnyView {
    let selected_target = target.unwrap_or(&compiled.active_target_file).to_string();
    let semantic = WorldSemanticQuery {
        world_metric,
        world_dataset,
        explain,
    };
    let show_inspector = should_show_world_semantic_inspector(selected_target.as_str(), semantic);
    let diag_filter_mode = DiagnosticsFilterMode::from_query(diag_filter);
    let source_panel = source.unwrap_or("").to_string();
    let preview = preview::preview_view(
        compiled,
        app_path,
        selected_target.as_str(),
        UiRouteMode::Build,
        semantic,
    );
    let active_scene = compiled.active_scene.as_deref();
    let scene_for_links = selected_scene.or(active_scene);
    let scene_target_pairs = compiled
        .scene_routes
        .iter()
        .map(|route| (route.target_file.clone(), route.scene_id.clone()))
        .collect::<Vec<_>>();
    let source_tree = source_tree::source_tree_view(
        &compiled.file_tree,
        UiRouteMode::Build,
        app_path,
        selected_target.as_str(),
        scene_for_links,
        scene_target_pairs.as_slice(),
        compiled.active_target_file.as_str(),
        active_tab,
        semantic,
    );
    let diagnostics = diagnostics_view(
        compiled,
        app_path,
        selected_target.as_str(),
        scene_for_links,
        diag_filter_mode,
    );
    let diagnostics_total = visible_diagnostics_count(compiled, selected_target.as_str());
    let script_target = is_mei_script_target(selected_target.as_str());
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let active_manage_tab = manage_view_tab_from_query(
        active_tab,
        script_target,
        compiled_has_error_diagnostics(compiled, selected_target.as_str()),
        diagnostics_total,
        selected_target.as_str(),
        semantic,
    );
    let topbar = topbar_view(
        apps,
        app_path,
        topbar_menu,
        UiRouteMode::Build,
        access_scene_for_topbar(
            UiRouteMode::Build,
            compiled,
            selected_scene.or(active_scene),
            preview_target,
        ),
        Some(selected_target.as_str()),
        active_tab,
        upload_enabled,
        stage_enabled,
        auth_enabled,
        auth_account,
    );
    let statusbar = statusbar_view(
        app_path,
        compiled.title.as_str(),
        UiRouteMode::Build.slug(),
        selected_target.as_str(),
        source_meta,
        Some(compiled),
        true,
        true,
        scene_for_links,
    );
    let workspace_class = if show_inspector {
        "workspace manage-workspace manage-workspace--with-inspector chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0"
    } else {
        "workspace manage-workspace chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0"
    };
    let inspector = if show_inspector {
        world_semantic_inspector_view(compiled, app_path, selected_target.as_str(), semantic)
    } else {
        view! { <></> }.into_any()
    };
    let shell_class = if stage_enabled {
        "shell shell-surface frame-stage-enabled text-slate-200"
    } else {
        "shell shell-surface text-slate-200"
    };
    let preview_scroll_class = if stage_enabled {
        "main-pane-scroll preview-pane-scroll frame-stage-enabled flex-1 min-h-0 overflow-auto p-0"
    } else {
        "main-pane-scroll preview-pane-scroll flex-1 min-h-0 overflow-auto p-0"
    };
    let asset_shell = classify_asset_shell(selected_target.as_str());
    let asset_cm_lang = codemirror_dataset_lang(selected_target.as_str());

    let tab_specs: Vec<(ManageViewTab, String, Option<String>, bool)> = if script_target {
        let mut v = vec![
            (ManageViewTab::Preview, "预览".to_string(), None, false),
            (ManageViewTab::Source, "只读源码".to_string(), None, false),
        ];
        if diagnostics_total > 0 {
            v.push((
                ManageViewTab::Diagnostics,
                "调试".to_string(),
                Some(diagnostics_total.to_string()),
                false,
            ));
        }
        v
    } else if asset_shell == AssetShellKind::Dual {
        vec![
            (ManageViewTab::Preview, "预览".to_string(), None, false),
            (ManageViewTab::Source, "只读源码".to_string(), None, false),
        ]
    } else {
        vec![]
    };

    let tab_links = tab_specs
        .into_iter()
        .map(|(tab, label, badge, start_hidden)| {
            let href = manage_tab_href(
                app_path,
                Some(selected_target.as_str()),
                selected_target.as_str(),
                script_target,
                tab,
                if tab == ManageViewTab::Diagnostics {
                    Some(diag_filter_mode.slug())
                } else {
                    None
                },
                scene_for_links,
                semantic,
            );
            let class = if tab == active_manage_tab {
                "manage-view-tab is-active"
            } else {
                "manage-view-tab"
            };
            let aria_current = if tab == active_manage_tab {
                Some("page")
            } else {
                None
            };
            let tab_id = format!("manage-tab-{}", tab.slug());
            view! {
                <a
                    id=tab_id
                    class=class
                    href=href
                    role="tab"
                    aria-selected=if tab == active_manage_tab { "true" } else { "false" }
                    data-manage-tab=tab.slug()
                    aria-current=aria_current
                    hidden=start_hidden
                >
                    <span class="manage-view-tab-label">{label}</span>
                    {badge
                        .map(|value| {
                            view! { <span class="manage-view-tab-badge">{value}</span> }.into_any()
                        })
                        .unwrap_or_else(|| view! { <></> }.into_any())}
                </a>
            }
        })
        .collect_view();

    let diagnostics_panel = if compiled.diagnostics.is_empty() {
        view! {
            <section class="grid gap-2 rounded-xl border border-dashed border-slate-600/55 bg-slate-900/45 p-4 text-xs leading-6 text-slate-400">
                <strong class="text-slate-200">"调试"</strong>
                <span>"当前编译没有 diagnostics。"</span>
                <span class="text-slate-500">"出现错误后，此页签会自动展示 Error / Warning / Info 列表。"</span>
            </section>
        }
        .into_any()
    } else {
        diagnostics
    };
    let preview_tab_active = active_manage_tab == ManageViewTab::Preview;
    let source_tab_active = active_manage_tab == ManageViewTab::Source;
    let diagnostics_tab_active = active_manage_tab == ManageViewTab::Diagnostics;
    let asset_source_tab_active = active_manage_tab == ManageViewTab::Source;
    let preview_scene_id = scene_for_links.or(compiled.active_scene.as_deref());
    let host_ssr_bootstrap = if script_target || stage_enabled {
        Some(host_ssr_bootstrap_scripts(
            compiled,
            app_path,
            preview_scene_id,
        ))
    } else {
        None
    };

    let non_script_main = match asset_shell {
        AssetShellKind::Dual => view! {
            <>
                <section
                    class="preview-pane min-w-0 min-h-0 flex flex-1 flex-col overflow-hidden"
                    data-manage-tab-panel="preview"
                    hidden=!preview_tab_active
                >
                    <div class="main-pane-scroll flex-1 min-h-0 overflow-auto p-0">
                        {asset_preview_body(
                            app_path,
                            selected_target.as_str(),
                            source_panel.as_str(),
                        )}
                    </div>
                </section>
                <section
                    class="source-panel source-pane min-w-0 min-h-0 flex flex-1 flex-col overflow-hidden"
                    data-manage-tab-panel="source"
                    hidden=!asset_source_tab_active
                >
                    {readonly_source_notice()}
                    {asset_codemirror_stack(
                        app_path,
                        selected_target.as_str(),
                        source_panel.as_str(),
                        asset_cm_lang,
                    )}
                </section>
            </>
        }
        .into_any(),
        AssetShellKind::SourceCode => view! {
            <section
                class="source-panel source-pane flex min-h-0 flex-1 flex-col overflow-hidden"
                data-asset-cm-only="1"
            >
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
        AssetShellKind::PreviewOnly | AssetShellKind::Unsupported => view! {
            <section
                class="asset-preview-pane flex min-h-0 flex-1 flex-col overflow-hidden"
                data-manage-tab-panel="preview"
            >
                {asset_preview_body(
                    app_path,
                    selected_target.as_str(),
                    source_panel.as_str(),
                )}
            </section>
        }
        .into_any(),
    };

    let main_tabs_nav = if script_target || asset_shell == AssetShellKind::Dual {
        view! {
            <nav
                class="manage-view-tabs workspace-tabs-strip mb-3 flex min-w-0 flex-wrap items-center gap-2 pb-2.5"
                role="tablist"
                aria-label="管理主视图"
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

    view! {
        <div class=shell_class>
            {host_ssr_bootstrap.unwrap_or_else(|| view! { <></> }.into_any())}
            <div
                id="tree-icons-sprite-root"
                class="pointer-events-none absolute left-0 top-0 -z-10 h-0 w-0 overflow-hidden opacity-0"
                aria-hidden="true"
                inner_html=source_tree::TREE_ICONS_SPRITE_SVG
            ></div>
            {topbar}
            <div
                class=workspace_class
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
                        {if script_target {
                            view! {
                                <>
                                    <section
                                        class=if stage_enabled {
                                            "preview-pane min-w-0 min-h-0 flex flex-1 flex-col overflow-auto"
                                        } else {
                                            "preview-pane min-w-0 min-h-0 flex flex-1 flex-col overflow-hidden"
                                        }
                                        data-manage-tab-panel="preview"
                                        hidden=!preview_tab_active
                                    >
                                        <div class=preview_scroll_class>
                                            {preview}
                                        </div>
                                    </section>
                                    <section
                                        class="source-panel source-pane min-w-0 min-h-0 flex flex-1 flex-col overflow-hidden"
                                        data-manage-tab-panel="source"
                                        hidden=!source_tab_active
                                    >
                                        {readonly_source_notice()}
                                        {asset_codemirror_stack(
                                            app_path,
                                            selected_target.as_str(),
                                            source_panel.as_str(),
                                            asset_cm_lang,
                                        )}
                                    </section>
                                    {if diagnostics_total > 0 {
                                        view! {
                                            <section
                                                class="min-w-0 min-h-0 flex-1 overflow-auto"
                                                data-manage-tab-panel="diagnostics"
                                                hidden=!diagnostics_tab_active
                                            >
                                                {diagnostics_panel}
                                            </section>
                                        }
                                            .into_any()
                                    } else {
                                        view! { <></> }.into_any()
                                    }}
                                </>
                            }
                                .into_any()
                        } else {
                            non_script_main
                        }}
                    </section>
                </main>
                {if show_inspector {
                    view! {
                        <>
                            <div
                                class="splitter splitter-right"
                                data-workspace-splitter="right"
                                role="separator"
                                aria-orientation="vertical"
                                aria-label="调整右侧语义检视栏宽度"
                            >
                                <button
                                    class="splitter-toggle"
                                    type="button"
                                    data-workspace-toggle="right"
                                    aria-label="折叠右侧语义检视栏"
                                    title="折叠右侧语义检视栏"
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
                                            <path d="M7.5 4.5L12.5 10l-5 5.5"></path>
                                        </svg>
                                    </span>
                                </button>
                            </div>
                            {inspector}
                        </>
                    }
                        .into_any()
                } else {
                    view! { <></> }.into_any()
                }}
            </div>
            {statusbar}
        </div>
    }
    .into_any()
}
