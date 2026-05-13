use std::path::Path;

use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};

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

pub fn render_page(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    route_mode: UiRouteMode,
    target: Option<&str>,
    source: Option<&str>,
    source_meta: Option<&SourcePanelMeta>,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    chrome_hidden: bool,
) -> String {
    let body_class = if route_mode == UiRouteMode::Access && chrome_hidden {
        "access-mode chrome-none"
    } else if route_mode == UiRouteMode::Access {
        "access-mode"
    } else {
        "manage-mode"
    };
    let shell = match route_mode {
        UiRouteMode::Access => access_shell(
            apps,
            compiled,
            selected_entry,
            preview_target,
            chrome_hidden,
        ),
        UiRouteMode::Manage => manage_shell(
            apps,
            compiled,
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
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
    chrome_hidden: bool,
) -> AnyView {
    let preview = preview::preview_view(compiled);
    let topbar = topbar_view(
        apps,
        compiled,
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
    target: Option<&str>,
    source: Option<&str>,
    source_meta: Option<&SourcePanelMeta>,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
) -> AnyView {
    let selected_target = target.unwrap_or(&compiled.entry_target).to_string();
    let source_panel = source.unwrap_or("").to_string();
    let source_lang = source_language(selected_target.as_str());
    let source_title = source_display_name(selected_target.as_str());
    let source_meta_text = source_meta_summary(source_meta);
    let preview = preview::preview_view(compiled);
    let active_entry = compiled.active_entry.as_deref();
    let source_entries = source_tree::entry_list_view(
        &compiled.entries,
        UiRouteMode::Manage,
        &compiled.app_id,
        active_entry,
    );
    let source_tree = source_tree::source_tree_view(
        &compiled.file_tree,
        UiRouteMode::Manage,
        &compiled.app_id,
        selected_target.as_str(),
        selected_entry.or(active_entry),
        preview_target,
    );
    let diagnostics = diagnostics_view(compiled);
    let topbar = topbar_view(
        apps,
        compiled,
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
                            <p>{compiled.app_id.clone()}</p>
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
                                <div class="panel-heading source-panel-heading">
                                    <h3>{source_title}</h3>
                                    <p class="source-panel-meta">{source_meta_text}</p>
                                </div>
                                <pre class="source-block"><code
                                    class="source-code"
                                    data-source-viewer="1"
                                    data-source-target=selected_target.clone()
                                    data-source-lang=source_lang
                                >{source_panel}</code></pre>
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
                        {opencode::panel_view(compiled, UiRouteMode::Manage, selected_target.as_str())}
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
    route_mode: UiRouteMode,
    selected_entry: Option<&str>,
    preview_target: Option<&str>,
) -> AnyView {
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let app_tabs = apps
        .iter()
        .map(|app| {
            let class = if app.id == compiled.app_id {
                "app-tab active"
            } else {
                "app-tab"
            };
            let href = format!("/apps/{}/{}", route_mode.slug(), app.id);
            view! { <a class=class href=href>{app.id.clone()}</a> }
        })
        .collect_view();
    let route_query = route_query(selected_entry, preview_target);
    let manage_href = format!("/apps/manage/{}{}", compiled.app_id, route_query);
    let access_href = format!("/apps/access/{}{}", compiled.app_id, route_query);
    let presentation_href = if route_query.is_empty() {
        format!("/apps/access/{}?chrome=none", compiled.app_id)
    } else {
        format!(
            "/apps/access/{}{}&chrome=none",
            compiled.app_id, route_query
        )
    };
    let mode_tabs = view! {
        <div class="mode-tabs">
            <a
                class=if route_mode == UiRouteMode::Manage { "mode-tab active" } else { "mode-tab" }
                href=manage_href
                title="编辑态"
                aria-label="编辑态"
            >
                <span class="mode-icon" aria-hidden="true">
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M12 20h9"/>
                        <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4 12.5-12.5z"/>
                    </svg>
                </span>
            </a>
            <a
                class=if route_mode == UiRouteMode::Access { "mode-tab active" } else { "mode-tab" }
                href=access_href
                title="访问态"
                aria-label="访问态"
            >
                <span class="mode-icon" aria-hidden="true">
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <rect x="3" y="4" width="18" height="14" rx="2"/>
                        <path d="M8 20h8"/>
                        <path d="M12 18v2"/>
                    </svg>
                </span>
            </a>
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
                <a
                    class="topbar-launch-link"
                    href=presentation_href
                    target="_blank"
                    rel="noopener noreferrer"
                    title=launch_title
                    aria-label=launch_title
                >
                    <span class="mode-icon" aria-hidden="true">
                        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M14 3h7v7"/>
                            <path d="M10 14L21 3"/>
                            <path d="M21 14v4a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3V6a3 3 0 0 1 3-3h4"/>
                        </svg>
                    </span>
                </a>
            </div>
        </header>
    }
    .into_any()
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

fn source_display_name(target: &str) -> String {
    Path::new(target)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(target)
        .to_string()
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
