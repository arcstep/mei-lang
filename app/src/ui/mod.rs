use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod opencode;
mod preview;
mod route;
mod source_tree;

pub use route::UiRouteMode;

#[derive(Debug, Clone)]
pub struct SourcePanelMeta {
    pub line_count: usize,
    pub char_count: usize,
    pub last_modified_label: Option<String>,
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
    topbar_menu_config: Option<&TopbarMenuConfig>,
    route_mode: UiRouteMode,
    target: Option<&str>,
    source: Option<&str>,
    source_meta: Option<&SourcePanelMeta>,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
    chrome_hidden: bool,
) -> String {
    let shell_mode_class = if route_mode == UiRouteMode::Access && chrome_hidden {
        "access-mode chrome-none"
    } else if route_mode == UiRouteMode::Access {
        "access-mode"
    } else {
        "manage-mode"
    };
    let body_class = format!("{shell_mode_class} sl-theme-dark");
    let shell = match route_mode {
        UiRouteMode::Access => access_shell(
            apps,
            compiled,
            app_path,
            topbar_menu_config,
            selected_entry,
            preview_target,
            active_tab,
            chrome_hidden,
        ),
        UiRouteMode::Manage => manage_shell(
            apps,
            compiled,
            app_path,
            topbar_menu_config,
            target,
            source,
            source_meta,
            selected_entry,
            preview_target,
            active_tab,
        ),
    };
    let chrome_scripts = chrome_scripts_view(route_mode);

    let page = view! {
        <html lang="zh-CN">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <title>{format!("{} - MeiLang", compiled.title)}</title>
                <link rel="icon" href="/app-assets/favicon.svg" type="image/svg+xml"/>
                <link rel="stylesheet" href="/app-assets/app-shell.css"/>
                <link rel="stylesheet" href="/app-assets/tailwind.css"/>
                <link rel="stylesheet" href="/app-assets/vendor/codemirror.css"/>
                <link rel="stylesheet" href="/app-assets/vendor/codemirror-merge.css"/>
                <link
                    rel="stylesheet"
                    href="https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/themes/dark.css"
                />
                <script
                    type="module"
                    src="https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/shoelace-autoloader.js"
                ></script>
            </head>
            <body class=body_class>
                {shell}
                {component_scripts(compiled)}
                {chrome_scripts}
                <script src="/app-assets/spa-navigation.js"></script>
            </body>
        </html>
    };
    page.to_html()
}

fn access_shell(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    topbar_menu_config: Option<&TopbarMenuConfig>,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
    chrome_hidden: bool,
) -> AnyView {
    let preview = preview::preview_view(compiled, app_path);
    let topbar = topbar_view(
        apps,
        compiled,
        app_path,
        topbar_menu_config,
        UiRouteMode::Access,
        selected_entry,
        preview_target,
        active_tab,
    );
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let shell_class = if chrome_hidden {
        "shell block min-h-screen h-screen overflow-hidden bg-slate-900 max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else if stage_enabled {
        "shell grid min-h-screen h-screen overflow-hidden bg-slate-900 [grid-template-rows:auto_minmax(0,1fr)] max-[1200px]:block max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else {
        "shell block min-h-screen h-auto overflow-visible bg-slate-900"
    };
    let main_class = if chrome_hidden {
        "min-h-0 min-w-0 h-full overflow-hidden p-0 max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else if stage_enabled {
        "min-h-0 min-w-0 h-full overflow-hidden p-4 self-stretch max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else {
        "min-w-0 h-auto overflow-visible p-4 self-start"
    };
    let preview_panel_class = if chrome_hidden {
        "min-h-0 min-w-0 h-full overflow-hidden [&_.preview-surface]:h-full [&_.preview-surface]:min-h-full [&_.preview-viewport]:h-full [&_.preview-viewport]:min-h-full max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else if stage_enabled {
        "min-h-0 min-w-0 h-full overflow-hidden [&_.preview-surface]:min-h-auto max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else {
        "min-h-0 min-w-0 overflow-visible [&_.preview-surface]:min-h-auto"
    };
    view! {
        <div class=shell_class>
            {if chrome_hidden {
                view! { <></> }.into_any()
            } else {
                topbar
            }}
            <main class=main_class>
                <section class=preview_panel_class>
                    {preview}
                </section>
            </main>
        </div>
    }
    .into_any()
}

fn manage_shell(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    topbar_menu_config: Option<&TopbarMenuConfig>,
    target: Option<&str>,
    source: Option<&str>,
    source_meta: Option<&SourcePanelMeta>,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
) -> AnyView {
    let selected_target = target.unwrap_or(&compiled.entry_target).to_string();
    let source_panel = source.unwrap_or("").to_string();
    let source_lang = source_language(selected_target.as_str());
    let source_meta_text = source_meta_summary(source_meta);
    let preview = preview::preview_view(compiled, app_path);
    let active_entry = compiled.active_entry.as_deref();
    let source_entries = source_tree::entry_list_view(
        &compiled.entries,
        UiRouteMode::Manage,
        app_path,
        active_entry,
        preview_target,
        active_tab,
    );
    let source_tree = source_tree::source_tree_view(
        &compiled.file_tree,
        UiRouteMode::Manage,
        app_path,
        selected_target.as_str(),
        selected_entry.or(active_entry),
        preview_target,
        active_tab,
    );
    let diagnostics = diagnostics_view(compiled);
    let diagnostics_total = compiled.diagnostics.len();
    let diagnostics_errors = compiled
        .diagnostics
        .iter()
        .filter(|item| matches!(item.severity, mei_lang_kernel::Severity::Error))
        .count();
    let diagnostics_warnings = compiled
        .diagnostics
        .iter()
        .filter(|item| matches!(item.severity, mei_lang_kernel::Severity::Warning))
        .count();
    let diagnostics_infos = compiled
        .diagnostics
        .iter()
        .filter(|item| matches!(item.severity, mei_lang_kernel::Severity::Info))
        .count();
    let script_target = is_mei_script_target(selected_target.as_str());
    let active_manage_tab = manage_view_tab_from_query(active_tab, script_target);
    let topbar = topbar_view(
        apps,
        compiled,
        app_path,
        topbar_menu_config,
        UiRouteMode::Manage,
        selected_entry.or(active_entry),
        preview_target,
        active_tab,
    );
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let shell_class = if stage_enabled {
        "shell frame-stage-enabled bg-slate-900 text-slate-200"
    } else {
        "shell bg-slate-900 text-slate-200"
    };
    let preview_scroll_class = if stage_enabled {
        "main-pane-scroll preview-pane-scroll frame-stage-enabled flex-1 min-h-0 overflow-hidden p-0"
    } else {
        "main-pane-scroll preview-pane-scroll flex-1 min-h-0 overflow-auto p-0"
    };
    let tabs = if script_target {
        vec![
            (ManageViewTab::Preview, "应用预览".to_string(), None),
            (ManageViewTab::Source, "源代码".to_string(), None),
            (ManageViewTab::Diff, "差异代码".to_string(), None),
            (
                ManageViewTab::Diagnostics,
                "错误诊断".to_string(),
                Some(diagnostics_total.to_string()),
            ),
        ]
    } else {
        vec![(ManageViewTab::Preview, "预览".to_string(), None)]
    };
    let tab_links = tabs
        .into_iter()
        .map(|(tab, label, badge)| {
            let href = manage_tab_href(
                app_path,
                selected_target.as_str(),
                selected_entry.or(active_entry),
                preview_target,
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
            view! {
                <a
                    class=class
                    href=href
                    role="tab"
                    aria-selected=if tab == active_manage_tab { "true" } else { "false" }
                    data-manage-tab=tab.slug()
                    aria-current=aria_current
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
                <strong class="text-slate-200">"错误诊断"</strong>
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
    let source_status_text = if source_mode == "diff" {
        "差异视图仅支持最后一轮 Build。"
    } else {
        "仅支持最后一轮 Build。"
    };

    view! {
        <div class=shell_class>
            {topbar}
            <div
                class="workspace min-h-0 h-full overflow-hidden p-4 grid gap-0 [grid-template-columns:var(--workspace-left-aside)_8px_minmax(0,1fr)_8px_var(--workspace-right-aside)]"
                id="workspace-root"
            >
                <aside class="sidebar left h-full min-h-0 min-w-0 overflow-hidden flex flex-col rounded-2xl border border-slate-400/15 bg-slate-900/80 p-3.5">
                    <div class="sidebar-header sticky top-0 z-[2] grid gap-2.5 pb-2.5 bg-slate-900/80">
                        <div class="mb-3 grid gap-1">
                            <h2 class="m-0 text-[15px] font-semibold text-slate-50">"资源树"</h2>
                            <p class="m-0 text-xs text-slate-400">{app_path.to_string()}</p>
                        </div>
                        {source_entries}
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
                ></div>
                <main class="main min-w-0 min-h-0 overflow-hidden px-4">
                    <section class="main-pane min-w-0 min-h-0 flex h-full flex-col overflow-hidden rounded-2xl border border-slate-400/15 bg-slate-900/80 p-3.5">
                        <nav class="manage-view-tabs mb-3 flex min-w-0 flex-wrap items-center gap-2 border-b border-slate-600/35 pb-2.5" role="tablist" aria-label="管理主视图">
                            {tab_links}
                            {if script_target {
                                view! {
                                    <div class="ml-auto flex items-center gap-2 text-[11px] text-slate-400">
                                        <span>{format!("Error {}", diagnostics_errors)}</span>
                                        <span>{format!("Warning {}", diagnostics_warnings)}</span>
                                        <span>{format!("Info {}", diagnostics_infos)}</span>
                                    </div>
                                }
                                    .into_any()
                            } else {
                                view! { <></> }.into_any()
                            }}
                        </nav>
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
                                            <div class="source-view-switcher mb-2.5 flex min-w-0 flex-nowrap items-center gap-2" role="group" aria-label="源码视图">
                                                <sl-button
                                                    class="source-view-btn hidden"
                                                    id="source-view-source-btn"
                                                    data-view-mode="source"
                                                    size="small"
                                                    pill=true
                                                    hidden=true
                                                >
                                                    "当前源码"
                                                </sl-button>
                                                <sl-tag class="source-view-status min-w-0" id="source-view-status" size="small" variant="primary" pill=true>
                                                    {source_status_text}
                                                </sl-tag>
                                                <span class="ml-auto shrink-0 text-right text-xs leading-5 text-slate-400">{source_meta_text.clone()}</span>
                                            </div>
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
                                    <section
                                        class="min-w-0 min-h-0 flex-1 overflow-auto"
                                        data-manage-tab-panel="diagnostics"
                                        hidden=!diagnostics_tab_active
                                    >
                                        {diagnostics_panel}
                                    </section>
                                </>
                            }
                                .into_any()
                        } else {
                            asset_preview_view(
                                app_path,
                                selected_target.as_str(),
                                source_panel.as_str(),
                                source_meta_text.as_str(),
                            )
                        }}
                    </section>
                </main>
                <div
                    class="splitter splitter-right"
                    data-workspace-splitter="right"
                    title="拖拽调整右侧 OpenCode 栏宽度"
                ></div>
                <aside class="sidebar right h-full min-h-0 min-w-0 overflow-hidden flex flex-col rounded-2xl border border-slate-400/15 bg-slate-900/80 p-3.5">
                    <div class="sidebar-scroll flex-1 min-h-0 overflow-auto">
                        {opencode::panel_view(
                            compiled,
                            app_path,
                            UiRouteMode::Manage,
                            selected_target.as_str(),
                            script_target,
                            active_manage_tab.slug(),
                        )}
                    </div>
                </aside>
            </div>
        </div>
    }
    .into_any()
}

fn topbar_view(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    active_app_path: &str,
    topbar_menu_config: Option<&TopbarMenuConfig>,
    route_mode: UiRouteMode,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
) -> AnyView {
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let route_query = route_query(selected_entry, preview_target, active_tab);
    let menu_groups = build_topbar_menu_groups(apps, topbar_menu_config);
    let active_menu_context = menu_groups.iter().find_map(|group| {
        group
            .items
            .iter()
            .find(|item| item.app_id.as_str() == active_app_path)
            .map(|item| (group.label.clone(), item.label.clone()))
    });
    let app_tabs = menu_groups
        .into_iter()
        .map(|group| {
            let group_id = group.id.clone();
            let group_label = group.label.clone();
            let group_has_active = group
                .items
                .iter()
                .any(|item| item.app_id.as_str() == active_app_path);
            let trigger_class = if group_has_active {
                "app-group-trigger is-active"
            } else {
                "app-group-trigger"
            };
            let mut direct_items = Vec::new();
            let mut subgroup_items: BTreeMap<String, Vec<_>> = BTreeMap::new();
            for item in &group.items {
                if let Some(subgroup) = &item.subgroup {
                    subgroup_items
                        .entry(subgroup.clone())
                        .or_default()
                        .push(item.clone());
                } else {
                    direct_items.push(item.clone());
                }
            }
            let direct_links = direct_items
                .iter()
                .map(|item| {
                    let class = if item.app_id.as_str() == active_app_path {
                        "app-tab app-tab-sub active"
                    } else {
                        "app-tab app-tab-sub"
                    };
                    let href = format!("/apps/{}/{}{}", route_mode.slug(), item.app_id, route_query);
                    view! { <a class=class href=href>{item.label.clone()}</a> }
                })
                .collect_view();
            let subgroup_blocks = subgroup_items
                .into_iter()
                .map(|(subgroup, items)| {
                    let links = items
                        .iter()
                        .map(|item| {
                            let class = if item.app_id.as_str() == active_app_path {
                                "app-tab app-tab-sub active"
                            } else {
                                "app-tab app-tab-sub"
                            };
                            let href = format!(
                                "/apps/{}/{}{}",
                                route_mode.slug(),
                                item.app_id,
                                route_query
                            );
                            view! { <a class=class href=href>{item.label.clone()}</a> }
                        })
                        .collect_view();
                    view! {
                        <section class="app-subgroup">
                            <h4 class="app-subgroup-title">{subgroup}</h4>
                            <div class="app-subgroup-items">{links}</div>
                        </section>
                    }
                })
                .collect_view();
            view! {
                <sl-dropdown
                    class="app-group-dropdown"
                    data-topbar-menu-group=group_id.clone()
                    placement="bottom-start"
                    distance="4"
                    hoist=true
                >
                    <sl-button
                        slot="trigger"
                        class=trigger_class
                        size="small"
                        caret=true
                    >
                        {group_label}
                    </sl-button>
                    <div class="app-group-menu">
                        {direct_links}
                        {subgroup_blocks}
                    </div>
                </sl-dropdown>
            }
        })
        .collect_view();
    let active_item_breadcrumb = active_menu_context
        .map(|(group_label, item_label)| {
            let aria_label = format!("当前位置：{group_label} / {item_label}");
            view! {
                <div class="app-current-path ml-auto inline-flex min-w-0 max-w-[min(320px,40vw)] items-center gap-2 border-l border-slate-400/15 pl-3.5 text-xs text-slate-400" aria-label=aria_label>
                    <span class="app-current-path-label shrink-0 text-slate-500">"当前"</span>
                    <span class="app-current-path-trail inline-flex min-w-0 items-center gap-1.5 whitespace-nowrap">
                        <span class="app-current-path-group shrink-0 text-slate-400">{group_label}</span>
                        <span class="app-current-path-separator shrink-0 text-slate-400/70" aria-hidden="true">"/"</span>
                        <span class="app-current-path-item min-w-0 overflow-hidden text-ellipsis text-slate-200">{item_label}</span>
                    </span>
                </div>
            }
            .into_any()
        })
        .unwrap_or_else(|| view! { <></> }.into_any());
    let manage_href = format!("/apps/manage/{}{}", active_app_path, route_query);
    let access_href = format!("/apps/access/{}{}", active_app_path, route_query);
    let presentation_href = if route_query.is_empty() {
        format!("/apps/access/{}?chrome=none", active_app_path)
    } else {
        format!(
            "/apps/access/{}{}&chrome=none",
            active_app_path, route_query
        )
    };
    let mode_tabs = view! {
        <div class="mode-tabs inline-flex items-center">
            <sl-button-group class="mode-tab-group" label="模式切换">
                <sl-button
                    class=if route_mode == UiRouteMode::Manage { "mode-tab-btn is-active" } else { "mode-tab-btn" }
                    size="small"
                    href=manage_href
                    title="编辑态"
                    aria-label="编辑态"
                >
                    <span class="mode-btn-content">
                        <span class="mode-icon" aria-hidden="true">
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M12 20h9"/>
                                <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4 12.5-12.5z"/>
                            </svg>
                        </span>
                        <span class="mode-label">"管理"</span>
                    </span>
                </sl-button>
                <sl-button
                    class=if route_mode == UiRouteMode::Access { "mode-tab-btn is-active" } else { "mode-tab-btn" }
                    size="small"
                    href=access_href
                    title="访问态"
                    aria-label="访问态"
                >
                    <span class="mode-btn-content">
                        <span class="mode-icon" aria-hidden="true">
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <rect x="3" y="4" width="18" height="14" rx="2"/>
                                <path d="M8 20h8"/>
                                <path d="M12 18v2"/>
                            </svg>
                        </span>
                        <span class="mode-label">"访问"</span>
                    </span>
                </sl-button>
            </sl-button-group>
        </div>
    };
    let launch_title = if stage_enabled {
        "在新标签页打开大屏"
    } else {
        "在新标签页打开无 Chrome 应用"
    };
    view! {
        <header class="topbar sticky top-0 z-10 grid grid-cols-[220px_minmax(0,1fr)_auto] items-center gap-4 border-b border-slate-400/15 bg-slate-900/90 px-5 py-3.5">
            <div class="brand grid gap-0.5">
                <div class="brand-title-row flex min-w-0 items-center gap-2">
                    <img
                        class="brand-mark block h-[22px] w-[22px] shrink-0"
                        src="/app-assets/favicon.svg"
                        width="22"
                        height="22"
                        alt=""
                        aria-hidden="true"
                    />
                    <strong class="text-base font-semibold text-slate-100">"MeiLang"</strong>
                </div>
                <span class="text-xs text-slate-400">"AI-Native"</span>
            </div>
            <nav class="app-tabs flex min-w-0 items-center justify-between gap-4">
                <div class="app-tabs-groups flex min-w-0 flex-wrap items-start gap-2">{app_tabs}</div>
                {active_item_breadcrumb}
            </nav>
            <div class="topbar-actions flex flex-wrap items-center justify-end gap-2.5">
                {mode_tabs}
                <sl-tooltip content=launch_title placement="bottom">
                    <sl-button
                        class="topbar-launch-btn"
                        size="small"
                        href=presentation_href
                        target="_blank"
                        rel="noopener noreferrer"
                        aria-label=launch_title
                    >
                        <span class="mode-icon" aria-hidden="true">
                            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M14 3h7v7"/>
                                <path d="M10 14L21 3"/>
                                <path d="M21 14v4a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3V6a3 3 0 0 1 3-3h4"/>
                            </svg>
                        </span>
                    </sl-button>
                </sl-tooltip>
            </div>
        </header>
    }
    .into_any()
}

#[derive(Debug, Clone)]
struct TopbarMenuItem {
    app_id: String,
    subgroup: Option<String>,
    label: String,
    order: i32,
}

#[derive(Debug, Clone)]
struct TopbarMenuGroup {
    id: String,
    label: String,
    order: i32,
    items: Vec<TopbarMenuItem>,
}

fn build_topbar_menu_groups(
    apps: &[WorkspaceAppMeta],
    config: Option<&TopbarMenuConfig>,
) -> Vec<TopbarMenuGroup> {
    let mut groups: BTreeMap<String, TopbarMenuGroup> = BTreeMap::new();
    let mut group_overrides: BTreeMap<String, (Option<String>, i32)> = BTreeMap::new();
    if let Some(config) = config {
        for group in &config.groups {
            group_overrides.insert(
                group.id.clone(),
                (group.label.clone(), group.order.unwrap_or(i32::MAX / 2)),
            );
        }
    }
    let item_overrides = config
        .map(|cfg| {
            cfg.items
                .iter()
                .map(|item| (item.app_id.clone(), item.clone()))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let skip_prefixes = normalized_skip_prefixes(config);
    for app in apps {
        let mut segments = app.id.split('/').filter(|value| !value.is_empty()).collect::<Vec<_>>();
        while segments.len() > 1 {
            let head = segments.first().map(|value| value.to_ascii_lowercase());
            if head
                .as_deref()
                .is_some_and(|value| skip_prefixes.iter().any(|prefix| prefix == value))
            {
                segments.remove(0);
                continue;
            }
            break;
        }
        if segments.is_empty() {
            continue;
        }
        let (mut group, mut subgroup, mut label) = menu_placement_from_segments(&segments);
        let mut item_order = infer_order_from_label(&label).unwrap_or(i32::MAX / 2);
        if let Some(override_item) = item_overrides.get(&app.id) {
            if let Some(override_group) = &override_item.group {
                group = override_group.clone();
            }
            if override_item.subgroup.is_some() {
                subgroup = override_item.subgroup.clone();
            }
            if let Some(override_label) = &override_item.label {
                label = override_label.clone();
            }
            if let Some(order) = override_item.order {
                item_order = order;
            }
        }
        let (group_label, group_order) = if let Some((label_override, order_override)) =
            group_overrides.get(&group)
        {
            (
                label_override
                    .clone()
                    .unwrap_or_else(|| menu_group_display_label(&group)),
                *order_override,
            )
        } else {
            (menu_group_display_label(&group), i32::MAX / 2)
        };
        groups
            .entry(group.clone())
            .or_insert_with(|| TopbarMenuGroup {
                id: group.clone(),
                label: group_label,
                order: group_order,
                items: Vec::new(),
            })
            .items
            .push(TopbarMenuItem {
                app_id: app.id.clone(),
                subgroup,
                label,
                order: item_order,
            });
    }
    let mut ordered = groups.into_values().collect::<Vec<_>>();
    for group in &mut ordered {
        group.items.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then(left.subgroup.cmp(&right.subgroup))
                .then(left.label.cmp(&right.label))
                .then(left.app_id.cmp(&right.app_id))
        });
    }
    ordered.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then(left.label.cmp(&right.label))
            .then(left.id.cmp(&right.id))
    });
    ordered
}

fn menu_placement_from_segments(segments: &[&str]) -> (String, Option<String>, String) {
    if segments.len() == 1 {
        let only = segments[0];
        if let Some((prefix, rest)) = only.split_once('-') {
            if !prefix.is_empty() && !rest.is_empty() {
                return (
                    prefix.to_string(),
                    None,
                    rest.trim_start_matches('-').to_string(),
                );
            }
        }
        return ("misc".to_string(), None, only.to_string());
    }
    if segments.len() == 2 {
        return (
            segments[0].to_string(),
            None,
            segments[1].to_string(),
        );
    }
    (
        segments[0].to_string(),
        Some(segments[1].to_string()),
        segments[2..].join("/"),
    )
}

fn menu_group_display_label(group: &str) -> String {
    if group == "misc" {
        "其他".to_string()
    } else {
        group.to_string()
    }
}

fn normalized_skip_prefixes(config: Option<&TopbarMenuConfig>) -> Vec<String> {
    if let Some(config) = config {
        if !config.skip_prefixes.is_empty() {
            return config
                .skip_prefixes
                .iter()
                .map(|value| value.to_ascii_lowercase())
                .collect();
        }
    }
    vec!["examples".to_string(), "workspaces".to_string()]
}

fn infer_order_from_label(label: &str) -> Option<i32> {
    let mut digits = String::new();
    for ch in label.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    digits.parse::<i32>().ok()
}

fn route_query(
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    active_tab: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    if let Some(preview_target) = preview_target {
        parts.push(format!("preview_target={preview_target}"));
    } else if let Some(entry) = selected_entry {
        parts.push(format!("entry={entry}"));
    }
    if let Some(tab) = manage_tab_from_slug(active_tab) {
        parts.push(format!("tab={}", tab.slug()));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

fn source_language(target: &str) -> &'static str {
    if is_mei_script_target(target) {
        "mei"
    } else {
        "plain"
    }
}

fn source_meta_summary(meta: Option<&SourcePanelMeta>) -> String {
    let Some(meta) = meta else {
        return "0 行 · 0 字 · 最后编辑时间未知".to_string();
    };
    let last_modified = meta
        .last_modified_label
        .as_deref()
        .map(|value| format!("最后编辑 {value}"))
        .unwrap_or_else(|| "最后编辑时间未知".to_string());
    format!(
        "{} 行 · {} 字 · {}",
        meta.line_count, meta.char_count, last_modified
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManageViewTab {
    Preview,
    Source,
    Diff,
    Diagnostics,
}

impl ManageViewTab {
    fn slug(self) -> &'static str {
        match self {
            ManageViewTab::Preview => "preview",
            ManageViewTab::Source => "source",
            ManageViewTab::Diff => "diff",
            ManageViewTab::Diagnostics => "diagnostics",
        }
    }
}

fn manage_tab_from_slug(value: Option<&str>) -> Option<ManageViewTab> {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "preview" => Some(ManageViewTab::Preview),
        "source" => Some(ManageViewTab::Source),
        "diff" => Some(ManageViewTab::Diff),
        "diagnostics" => Some(ManageViewTab::Diagnostics),
        _ => None,
    }
}

fn manage_view_tab_from_query(active_tab: Option<&str>, script_target: bool) -> ManageViewTab {
    let next = manage_tab_from_slug(active_tab).unwrap_or(ManageViewTab::Preview);
    if script_target {
        next
    } else {
        ManageViewTab::Preview
    }
}

fn is_mei_script_target(target: &str) -> bool {
    target.ends_with(".mei") || target.ends_with(".star")
}

fn manage_tab_href(
    app_path: &str,
    target: &str,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    script_target: bool,
    tab: ManageViewTab,
) -> String {
    let mut query = vec![format!("target={target}")];
    if let Some(preview_target) = preview_target {
        query.push(format!("preview_target={preview_target}"));
    } else if let Some(entry) = selected_entry {
        query.push(format!("entry={entry}"));
    }
    let route_tab = if script_target { tab } else { ManageViewTab::Preview };
    query.push(format!("tab={}", route_tab.slug()));
    format!("/apps/manage/{app_path}?{}", query.join("&"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssetPreviewKind {
    Markdown,
    Image,
    Pdf,
    Csv,
    Text,
    Unsupported,
}

fn asset_preview_view(
    app_path: &str,
    target: &str,
    source: &str,
    source_meta_text: &str,
) -> AnyView {
    let kind = asset_preview_kind(target);
    let asset_src = workspace_asset_href(app_path, target);
    let extension = target
        .rsplit('.')
        .next()
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let content = match kind {
        AssetPreviewKind::Markdown => {
            let html = markdown_preview_html(source);
            view! { <article class="asset-markdown-preview min-h-0 overflow-auto rounded-xl border border-slate-700/55 bg-slate-950/40 p-4" inner_html=html></article> }
                .into_any()
        }
        AssetPreviewKind::Image => {
            view! {
                <div class="asset-image-preview flex min-h-0 flex-1 items-center justify-center overflow-auto rounded-xl border border-slate-700/55 bg-slate-950/40 p-4">
                    <img class="max-h-full max-w-full rounded-lg object-contain" src=asset_src alt=target.to_string() loading="lazy"/>
                </div>
            }
            .into_any()
        }
        AssetPreviewKind::Pdf => {
            view! {
                <div class="asset-pdf-preview min-h-0 flex-1 overflow-hidden rounded-xl border border-slate-700/55 bg-slate-950/40">
                    <iframe class="h-full w-full border-0" src=asset_src title=target.to_string()></iframe>
                </div>
            }
            .into_any()
        }
        AssetPreviewKind::Csv => {
            let (headers, rows, truncated, shown_rows, shown_cols) = csv_preview_table(source, 120, 24);
            view! {
                <div class="asset-csv-preview grid min-h-0 flex-1 gap-2 overflow-hidden">
                    <div class="flex items-center justify-between gap-2 text-[11px] text-slate-400">
                        <span>{format!("CSV 预览：{} 行 · {} 列", shown_rows, shown_cols)}</span>
                        {if truncated {
                            view! { <span class="text-amber-300">"已截断显示"</span> }.into_any()
                        } else {
                            view! { <></> }.into_any()
                        }}
                    </div>
                    <div class="overflow-auto rounded-xl border border-slate-700/55 bg-slate-950/40">
                        <table class="min-w-full border-collapse text-left text-xs text-slate-200">
                            <thead class="sticky top-0 z-[1] bg-slate-900/95">
                                <tr>
                                    <th class="sticky left-0 z-[2] whitespace-nowrap border-b border-slate-700 bg-slate-900/95 px-3 py-2 font-semibold text-slate-400">"#"</th>
                                    {headers
                                        .iter()
                                        .enumerate()
                                        .map(|(idx, value)| {
                                            let title = if value.is_empty() {
                                                format!("列 {}", idx + 1)
                                            } else {
                                                value.clone()
                                            };
                                            view! {
                                                <th class="whitespace-nowrap border-b border-slate-700 px-3 py-2 font-semibold text-slate-100">{title}</th>
                                            }
                                        })
                                        .collect_view()}
                                </tr>
                            </thead>
                            <tbody>
                                {rows
                                    .iter()
                                    .enumerate()
                                    .map(|row| {
                                        let row_index = row.0 + 1;
                                        let row = row.1;
                                        view! {
                                            <tr>
                                                <td class="sticky left-0 z-[1] border-b border-slate-800/80 bg-slate-900/80 px-3 py-2 align-top text-slate-400">{row_index}</td>
                                                {row
                                                    .iter()
                                                    .map(|cell| {
                                                        view! { <td class="border-b border-slate-800/80 px-3 py-2 align-top leading-5 text-slate-300">{cell.clone()}</td> }
                                                    })
                                                    .collect_view()}
                                            </tr>
                                        }
                                    })
                                    .collect_view()}
                            </tbody>
                        </table>
                    </div>
                    {if truncated {
                        view! {
                            <div class="text-[11px] text-slate-400">
                                "CSV 预览已截断，仅展示前 120 行与前 24 列（含索引列）。"
                            </div>
                        }
                            .into_any()
                    } else {
                        view! { <></> }.into_any()
                    }}
                </div>
            }
            .into_any()
        }
        AssetPreviewKind::Text => {
            view! {
                <pre class="asset-text-preview min-h-0 flex-1 overflow-auto rounded-xl border border-slate-700/55 bg-slate-950/40 p-4 text-xs leading-6 text-slate-200">{source.to_string()}</pre>
            }
            .into_any()
        }
        AssetPreviewKind::Unsupported => {
            view! {
                <section class="grid min-h-0 flex-1 place-content-center gap-2 rounded-xl border border-dashed border-slate-600/55 bg-slate-950/35 p-6 text-center text-sm leading-6 text-slate-400">
                    <strong class="text-slate-100">"暂不支持该资源类型预览"</strong>
                    <span>{format!("目标：{}{}", target, if extension.is_empty() { "".to_string() } else { format!("（.{}）", extension) })}</span>
                </section>
            }
            .into_any()
        }
    };
    view! {
        <section class="asset-preview-pane grid h-full min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-2.5" data-manage-tab-panel="preview">
            <div class="inline-flex min-w-0 items-center justify-between gap-2 rounded-xl border border-slate-700/55 bg-slate-900/55 px-3 py-2 text-xs">
                <div class="min-w-0 truncate text-slate-200">{target.to_string()}</div>
                <div class="shrink-0 text-slate-400">{source_meta_text.to_string()}</div>
            </div>
            {content}
        </section>
    }
    .into_any()
}

fn asset_preview_kind(target: &str) -> AssetPreviewKind {
    let ext = target
        .rsplit('.')
        .next()
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" => AssetPreviewKind::Markdown,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "avif" => {
            AssetPreviewKind::Image
        }
        "pdf" => AssetPreviewKind::Pdf,
        "csv" => AssetPreviewKind::Csv,
        "txt" | "json" | "yaml" | "yml" | "toml" | "xml" | "log" | "rs" | "js" | "ts"
        | "tsx" | "jsx" | "css" | "html" | "htm" | "sh" | "zsh" | "bash" | "mei"
        | "star" => AssetPreviewKind::Text,
        _ => {
            if ext.is_empty() {
                AssetPreviewKind::Text
            } else {
                AssetPreviewKind::Unsupported
            }
        }
    }
}

fn workspace_asset_href(app_path: &str, target: &str) -> String {
    format!(
        "/workspace-app-assets/{}/{}",
        percent_encode_path(app_path),
        percent_encode_path(target)
    )
}

fn percent_encode_path(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        let is_allowed = byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/');
        if is_allowed {
            output.push(char::from(byte));
        } else {
            output.push('%');
            output.push_str(&format!("{:02X}", byte));
        }
    }
    output
}

fn csv_preview_table(
    source: &str,
    max_rows: usize,
    max_cols: usize,
) -> (Vec<String>, Vec<Vec<String>>, bool, usize, usize) {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(source.as_bytes());
    let mut rows = Vec::new();
    let mut max_width = 0usize;
    let mut truncated = false;
    for record in reader.records() {
        match record {
            Ok(record) => {
                if rows.len() >= max_rows {
                    truncated = true;
                    break;
                }
                let mut row = record.iter().take(max_cols).map(|value| value.to_string()).collect::<Vec<_>>();
                if record.len() > max_cols {
                    truncated = true;
                }
                max_width = max_width.max(row.len());
                if row.is_empty() {
                    row.push(String::new());
                }
                rows.push(row);
            }
            Err(_) => {
                return (
                    vec!["内容".to_string()],
                    source
                        .lines()
                        .take(max_rows)
                        .map(|line| vec![line.to_string()])
                        .collect::<Vec<_>>(),
                    source.lines().count() > max_rows,
                    source.lines().take(max_rows).count(),
                    1,
                );
            }
        }
    }
    if rows.is_empty() {
        return (vec!["内容".to_string()], vec![vec!["".to_string()]], false, 1, 1);
    }
    let width = max_width.max(1);
    for row in &mut rows {
        while row.len() < width {
            row.push(String::new());
        }
    }
    let headers = (0..width)
        .map(|idx| rows[0].get(idx).cloned().unwrap_or_default())
        .collect::<Vec<_>>();
    let body = if rows.len() > 1 { rows[1..].to_vec() } else { Vec::new() };
    let shown_rows = body.len().max(1);
    (headers, body, truncated, shown_rows, width)
}

fn markdown_preview_html(source: &str) -> String {
    let mut html = String::new();
    let mut in_list = false;
    let mut in_code = false;
    for raw_line in source.lines().take(800) {
        let line = raw_line.trim_end();
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_code {
                html.push_str("</code></pre>");
                in_code = false;
            } else {
                if in_list {
                    html.push_str("</ul>");
                    in_list = false;
                }
                html.push_str("<pre><code>");
                in_code = true;
            }
            continue;
        }
        if in_code {
            html.push_str(&escape_html(line));
            html.push('\n');
            continue;
        }
        if trimmed.is_empty() {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str("<h1>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</h1>");
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str("<h2>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</h2>");
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str("<h3>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</h3>");
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if !in_list {
                html.push_str("<ul>");
                in_list = true;
            }
            html.push_str("<li>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</li>");
            continue;
        }
        if in_list {
            html.push_str("</ul>");
            in_list = false;
        }
        html.push_str("<p>");
        html.push_str(&markdown_inline_html(trimmed));
        html.push_str("</p>");
    }
    if in_code {
        html.push_str("</code></pre>");
    }
    if in_list {
        html.push_str("</ul>");
    }
    if html.is_empty() {
        html.push_str("<p class=\"is-empty\">空文档</p>");
    }
    html
}

fn markdown_inline_html(value: &str) -> String {
    let mut output = String::new();
    let mut index = 0usize;
    while index < value.len() {
        let rest = &value[index..];
        let next_code = rest.find('`');
        let next_link = rest.find('[');
        let next_token = match (next_code, next_link) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some(next) = next_token else {
            output.push_str(&escape_html(rest));
            break;
        };
        if next > 0 {
            output.push_str(&escape_html(&rest[..next]));
            index += next;
            continue;
        }
        if rest.starts_with('`') {
            if let Some(end) = rest[1..].find('`') {
                let code = &rest[1..(1 + end)];
                output.push_str("<code>");
                output.push_str(&escape_html(code));
                output.push_str("</code>");
                index += end + 2;
            } else {
                output.push('`');
                index += 1;
            }
            continue;
        }
        if rest.starts_with('[') {
            if let Some(close) = rest.find(']') {
                let label = &rest[1..close];
                let remain = &rest[(close + 1)..];
                if let Some(link_body) = remain.strip_prefix('(') {
                    if let Some(end) = link_body.find(')') {
                        let raw_href = link_body[..end].trim();
                        if let Some(href) = sanitize_markdown_href(raw_href) {
                            output.push_str("<a href=\"");
                            output.push_str(&escape_html_attr(href));
                            output.push_str("\" target=\"_blank\" rel=\"noopener noreferrer\">");
                            output.push_str(&escape_html(label));
                            output.push_str("</a>");
                            index += close + end + 3;
                            continue;
                        }
                    }
                }
            }
            output.push('[');
            index += 1;
            continue;
        }
    }
    output
}

fn sanitize_markdown_href(raw: &str) -> Option<&str> {
    let href = raw.trim();
    if href.is_empty() {
        return None;
    }
    if href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("mailto:")
        || href.starts_with('/')
        || href.starts_with("./")
        || href.starts_with("../")
        || href.starts_with('#')
    {
        Some(href)
    } else {
        None
    }
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn diagnostics_view(compiled: &CompiledApp) -> AnyView {
    if compiled.diagnostics.is_empty() {
        return view! { <></> }.into_any();
    }
    let diagnostics = compiled
        .diagnostics
        .iter()
        .map(|diag| {
            let class = match diag.severity {
                mei_lang_kernel::Severity::Error => {
                    "diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-red-900/25 border-red-400/30"
                }
                mei_lang_kernel::Severity::Warning => {
                    "diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-amber-900/25 border-amber-300/35"
                }
                mei_lang_kernel::Severity::Info => {
                    "diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-blue-900/25 border-blue-300/35"
                }
            };
            view! {
                <div class=class>
                    <strong class="text-xs font-semibold text-slate-50">{diag.code.clone()}</strong>
                    <span class="text-xs leading-5 text-slate-200">{diag.message.clone()}</span>
                </div>
            }
        })
        .collect_view();
    view! {
        <section class="source-diagnostics mt-4 grid gap-2 border-t border-slate-600/40 pt-4">
            <div class="mb-0 grid gap-1">
                <h3 class="m-0 text-[15px] font-semibold text-slate-50">"编译提示"</h3>
                <p class="m-0 text-xs text-slate-400">"最小内核 diagnostics"</p>
            </div>
            {diagnostics}
        </section>
    }
    .into_any()
}

fn chrome_scripts_view(route_mode: UiRouteMode) -> AnyView {
    if route_mode == UiRouteMode::Manage {
        view! {
            <>
                <script src="/app-assets/frame-stage.js"></script>
                <script src="/app-assets/vendor/diff-match-patch.js"></script>
                <script src="/app-assets/vendor/codemirror.js"></script>
                <script src="/app-assets/source-codemirror-mode.js"></script>
                <script src="/app-assets/vendor/codemirror-merge.js"></script>
                <script src="/app-assets/manage-tabs.js"></script>
                <script src="/app-assets/opencode-panel.js"></script>
                <script src="/app-assets/workspace-splitters.js"></script>
                <script src="/app-assets/source-tree-controls.js"></script>
                <script src="/app-assets/source-highlight.js"></script>
            </>
        }
        .into_any()
    } else {
        view! { <script src="/app-assets/frame-stage.js"></script> }.into_any()
    }
}

fn component_scripts(compiled: &CompiledApp) -> impl IntoView {
    let scripts = compiled
        .component_assets
        .iter()
        .map(|asset| {
            let src = format!("/workspace-components/{}", asset.script);
            view! { <script type="module" src=src></script> }
        })
        .collect_view();
    view! { <>{scripts}</> }
}
