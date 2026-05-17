use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};

use super::compile_status::{
    classify_asset_shell, codemirror_dataset_lang, compiled_has_error_diagnostics,
    is_mei_script_target, source_language, AssetShellKind,
};
use super::manage_routing::{manage_tab_href, manage_view_tab_from_query, ManageViewTab};
use super::agent_panel;
use super::preview;
use super::preview_chrome::{asset_preview_body, diagnostics_view};
use super::route::UiRouteMode;
use super::source_tree;
use super::statusbar::statusbar_view;
use super::topbar::topbar_view;
use super::{SourcePanelMeta, TopbarMenuContext};

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

pub(super) fn manage_shell(
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
) -> AnyView {
    let selected_target = target.unwrap_or(&compiled.active_target_file).to_string();
    let source_panel = source.unwrap_or("").to_string();
    let source_lang = source_language(selected_target.as_str());
    let preview = preview::preview_view(compiled, app_path);
    let active_scene = compiled.active_scene.as_deref();
    let current_scene = selected_scene.or(active_scene);
    let default_file_for_scene = current_scene
        .and_then(|sid| {
            compiled
                .scene_routes
                .iter()
                .find(|r| r.scene_id == sid)
                .map(|r| r.target_file.as_str())
        })
        .unwrap_or(compiled.active_target_file.as_str());
    let file_for_url = if selected_target.as_str() == default_file_for_scene {
        None
    } else {
        Some(selected_target.as_str())
    };
    let source_tree = source_tree::source_tree_view(
        &compiled.file_tree,
        UiRouteMode::Manage,
        app_path,
        selected_target.as_str(),
        selected_scene.or(active_scene),
        default_file_for_scene,
        active_tab,
    );
    let diagnostics = diagnostics_view(compiled);
    let diagnostics_total = compiled.diagnostics.len();
    let script_target = is_mei_script_target(selected_target.as_str());
    let active_manage_tab = manage_view_tab_from_query(
        active_tab,
        script_target,
        compiled_has_error_diagnostics(compiled),
        diagnostics_total,
        selected_target.as_str(),
    );
    let topbar = topbar_view(
        apps,
        compiled,
        app_path,
        topbar_menu,
        UiRouteMode::Manage,
        selected_scene.or(active_scene),
        preview_target,
        active_tab,
    );
    let statusbar = statusbar_view(
        app_path,
        UiRouteMode::Manage.slug(),
        selected_target.as_str(),
        source_meta,
        compiled,
        true,
        true,
    );
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let shell_class = if stage_enabled {
        "shell shell-surface frame-stage-enabled text-slate-200"
    } else {
        "shell shell-surface text-slate-200"
    };
    let preview_scroll_class = if stage_enabled {
        "main-pane-scroll preview-pane-scroll frame-stage-enabled flex-1 min-h-0 overflow-hidden p-0"
    } else {
        "main-pane-scroll preview-pane-scroll flex-1 min-h-0 overflow-auto p-0"
    };
    let asset_shell = classify_asset_shell(selected_target.as_str());
    let asset_cm_lang = codemirror_dataset_lang(selected_target.as_str());

    let tab_specs: Vec<(ManageViewTab, String, Option<String>, bool)> = if script_target {
        let mut v = vec![
            (ManageViewTab::Preview, "预览".to_string(), None, false),
            (ManageViewTab::Source, "源码".to_string(), None, false),
            (ManageViewTab::Diff, "修改".to_string(), None, true),
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
            (ManageViewTab::Source, "源码".to_string(), None, false),
        ]
    } else {
        vec![]
    };

    let tab_links = tab_specs
        .into_iter()
        .map(|(tab, label, badge, start_hidden)| {
            let href = manage_tab_href(
                app_path,
                selected_scene.or(active_scene),
                file_for_url,
                selected_target.as_str(),
                script_target,
                tab,
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
    let source_mode = if active_manage_tab == ManageViewTab::Diff {
        "diff"
    } else {
        "source"
    };
    let preview_tab_active = active_manage_tab == ManageViewTab::Preview;
    let source_tab_active =
        active_manage_tab == ManageViewTab::Source || active_manage_tab == ManageViewTab::Diff;
    let diagnostics_tab_active = active_manage_tab == ManageViewTab::Diagnostics;
    let asset_source_tab_active = active_manage_tab == ManageViewTab::Source;

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
            <div
                id="tree-icons-sprite-root"
                class="pointer-events-none absolute left-0 top-0 -z-10 h-0 w-0 overflow-hidden opacity-0"
                aria-hidden="true"
                inner_html=source_tree::TREE_ICONS_SPRITE_SVG
            ></div>
            {topbar}
            <div
                class="workspace chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0 [grid-template-columns:var(--workspace-left-aside)_8px_minmax(0,1fr)_8px_var(--workspace-right-aside)]"
                id="workspace-root"
            >
                <aside class="sidebar left workspace-panel workspace-panel-side workspace-panel-nav h-full min-h-0 min-w-0 overflow-hidden flex flex-col px-4 py-2.5">
                    <div class="sidebar-header workspace-panel-header sticky top-0 z-[2] grid gap-2.5 pb-2.5">
                        {source_tree::controls_view()}
                    </div>
                    <div class="sidebar-scroll flex-1 min-h-0 overflow-auto">
                        {source_tree}
                    </div>
                </aside>
                <div
                    class="splitter"
                    data-workspace-splitter="left"
                    title="拖拽调整左侧资源栏宽度"
                >
                    <button
                        class="splitter-toggle"
                        type="button"
                        data-workspace-toggle="left"
                        aria-label="折叠左侧资源栏"
                        title="折叠左侧资源栏"
                    >
                        <span class="splitter-toggle-icon" aria-hidden="true">
                            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M10.5 3.5 6 8l4.5 4.5"/>
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
                                        class="preview-pane min-w-0 min-h-0 flex flex-1 flex-col overflow-hidden"
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
                                        <div class="main-pane-scroll source-pane-scroll flex min-h-0 flex-1 flex-col overflow-auto">
                                            <div class="source-view-host flex flex-1 min-h-0 flex-col gap-2.5" id="source-view-host" data-source-mode=source_mode>
                                                <div
                                                    class="source-editor-host"
                                                    id="source-view-source-panel"
                                                    data-source-target=selected_target.clone()
                                                    data-source-lang=source_lang
                                                    hidden=source_mode != "source"
                                                ></div>
                                                <div
                                                    id="source-view-source-raw"
                                                    hidden
                                                    data-source-target=selected_target.clone()
                                                    data-source-lang=source_lang
                                                >{source_panel.clone()}</div>
                                                <div class="source-diff-host" id="source-view-diff-panel" hidden=source_mode != "diff"></div>
                                            </div>
                                        </div>
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
                <div
                    class="splitter splitter-right"
                    data-workspace-splitter="right"
                    title="拖拽调整右侧 OpenCode 栏宽度"
                >
                    <button
                        class="splitter-toggle"
                        type="button"
                        data-workspace-toggle="right"
                        aria-label="折叠右侧 OpenCode 栏"
                        title="折叠右侧 OpenCode 栏"
                    >
                        <span class="splitter-toggle-icon" aria-hidden="true">
                            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M5.5 3.5 10 8l-4.5 4.5"/>
                            </svg>
                        </span>
                    </button>
                </div>
                <aside class="sidebar right workspace-panel workspace-panel-side workspace-panel-tool h-full min-h-0 min-w-0 overflow-hidden flex flex-col px-0 py-2.5">
                    <div class="sidebar-scroll flex-1 min-h-0 overflow-auto">
                        {agent_panel::panel_view(
                            compiled,
                            app_path,
                            UiRouteMode::Manage,
                            selected_target.as_str(),
                            script_target,
                            active_manage_tab.slug(),
                            true,
                            true,
                            "build",
                            true,
                        )}
                    </div>
                </aside>
            </div>
            {statusbar}
        </div>
    }
    .into_any()
}
