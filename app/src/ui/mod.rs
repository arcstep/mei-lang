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
    );
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let shell_class = if chrome_hidden && stage_enabled {
        "shell access-shell access-shell-chromeless frame-stage-enabled"
    } else if chrome_hidden {
        "shell access-shell access-shell-chromeless"
    } else if stage_enabled {
        "shell access-shell frame-stage-enabled"
    } else {
        "shell access-shell"
    };
    let main_class = if chrome_hidden && stage_enabled {
        "access-main frame-stage-enabled access-main-chromeless"
    } else if chrome_hidden {
        "access-main access-main-chromeless"
    } else if stage_enabled {
        "access-main frame-stage-enabled"
    } else {
        "access-main"
    };
    let preview_panel_class = if chrome_hidden && stage_enabled {
        "access-preview-panel frame-stage-enabled access-preview-panel-chromeless"
    } else if chrome_hidden {
        "access-preview-panel access-preview-panel-chromeless"
    } else if stage_enabled {
        "access-preview-panel frame-stage-enabled"
    } else {
        "access-preview-panel"
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
    );
    let source_tree = source_tree::source_tree_view(
        &compiled.file_tree,
        UiRouteMode::Manage,
        app_path,
        selected_target.as_str(),
        selected_entry.or(active_entry),
        preview_target,
    );
    let diagnostics = diagnostics_view(compiled);
    let topbar = topbar_view(
        apps,
        compiled,
        app_path,
        topbar_menu_config,
        UiRouteMode::Manage,
        selected_entry.or(active_entry),
        preview_target,
    );
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let shell_class = if stage_enabled {
        "shell frame-stage-enabled"
    } else {
        "shell"
    };
    let preview_scroll_class = if stage_enabled {
        "main-pane-scroll preview-pane-scroll frame-stage-enabled"
    } else {
        "main-pane-scroll preview-pane-scroll"
    };

    view! {
        <div class=shell_class>
            {topbar}
            <div class="workspace" id="workspace-root">
                <aside class="sidebar left">
                    <div class="sidebar-header">
                        <div class="panel-heading">
                            <h2>"资源树"</h2>
                            <p>{app_path.to_string()}</p>
                        </div>
                        {source_entries}
                        {source_tree::controls_view()}
                    </div>
                    <div class="sidebar-scroll">
                        {source_tree}
                    </div>
                </aside>
                <div
                    class="splitter"
                    data-workspace-splitter="left"
                    title="拖拽调整左侧资源栏宽度"
                ></div>
                <main class="main">
                    <div class="main-stack" id="main-stack-root">
                        <section class="panel preview-panel main-pane preview-pane">
                            <div class=preview_scroll_class>
                                {preview}
                            </div>
                        </section>
                        <div
                            class="splitter splitter-horizontal"
                            data-workspace-splitter="preview"
                            title="拖拽调整应用预览与源码区域高度"
                        ></div>
                        <section class="panel source-panel main-pane source-pane">
                            <div class="main-pane-scroll source-pane-scroll">
                                <div class="source-view-switcher" role="group" aria-label="源码视图">
                                    <sl-button
                                        class="source-view-btn is-active"
                                        id="source-view-source-btn"
                                        data-view-mode="source"
                                        size="small"
                                        pill=true
                                    >
                                        "当前源码"
                                    </sl-button>
                                    <sl-tag class="source-view-status" id="source-view-status" size="small" variant="primary" pill=true>
                                        "仅支持最后一轮 Build"
                                    </sl-tag>
                                    <span class="source-panel-meta source-panel-meta-inline">{source_meta_text}</span>
                                </div>
                                <div class="source-view-host" id="source-view-host">
                                    <div
                                        class="source-editor-host"
                                        id="source-view-source-panel"
                                        data-source-target=selected_target.clone()
                                        data-source-lang=source_lang
                                    ></div>
                                    <div
                                        id="source-view-source-raw"
                                        hidden
                                        data-source-target=selected_target.clone()
                                        data-source-lang=source_lang
                                    >{source_panel}</div>
                                    <div class="source-diff-host" id="source-view-diff-panel" hidden></div>
                                </div>
                                {diagnostics}
                            </div>
                        </section>
                    </div>
                </main>
                <div
                    class="splitter splitter-right"
                    data-workspace-splitter="right"
                    title="拖拽调整右侧 OpenCode 栏宽度"
                ></div>
                <aside class="sidebar right">
                    <div class="sidebar-scroll">
                        {opencode::panel_view(compiled, app_path, UiRouteMode::Manage, selected_target.as_str())}
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
) -> AnyView {
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let app_tabs = build_topbar_menu_groups(apps, topbar_menu_config)
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
                    let href = format!("/apps/{}/{}", route_mode.slug(), item.app_id);
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
                            let href = format!("/apps/{}/{}", route_mode.slug(), item.app_id);
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
    let route_query = route_query(selected_entry, preview_target);
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
        <div class="mode-tabs">
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
        <header class="topbar">
            <div class="brand">
                <div class="brand-title-row">
                    <img
                        class="brand-mark"
                        src="/app-assets/favicon.svg"
                        width="22"
                        height="22"
                        alt=""
                        aria-hidden="true"
                    />
                    <strong>"MeiLang"</strong>
                </div>
                <span>"AI-Native"</span>
            </div>
            <nav class="app-tabs">{app_tabs}</nav>
            <div class="topbar-actions">
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

fn route_query(selected_entry: Option<&str>, preview_target: Option<&str>) -> String {
    if let Some(preview_target) = preview_target {
        return format!("?preview_target={preview_target}");
    }
    if let Some(entry) = selected_entry {
        return format!("?entry={entry}");
    }
    String::new()
}

fn source_language(target: &str) -> &'static str {
    if target.ends_with(".mei") || target.ends_with(".star") {
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

fn diagnostics_view(compiled: &CompiledApp) -> AnyView {
    if compiled.diagnostics.is_empty() {
        return view! { <></> }.into_any();
    }
    let diagnostics = compiled
        .diagnostics
        .iter()
        .map(|diag| {
            let class = match diag.severity {
                mei_lang_kernel::Severity::Error => "diag diag-error",
                mei_lang_kernel::Severity::Warning => "diag diag-warning",
                mei_lang_kernel::Severity::Info => "diag diag-info",
            };
            view! {
                <div class=class>
                    <strong>{diag.code.clone()}</strong>
                    <span>{diag.message.clone()}</span>
                </div>
            }
        })
        .collect_view();
    view! {
        <section class="source-diagnostics">
            <div class="panel-heading">
                <h3>"编译提示"</h3>
                <p>"最小内核 diagnostics"</p>
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
