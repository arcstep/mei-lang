use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};

mod opencode;
mod preview;
mod route;
mod source_tree;
mod workspace;

pub use route::UiRouteMode;

pub fn render_page(
    apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    route_mode: UiRouteMode,
    target: Option<&str>,
    source: Option<&str>,
) -> String {
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

    let manage_href = format!("/apps/manage/{}", compiled.app_id);
    let access_href = format!("/apps/access/{}", compiled.app_id);
    let selected_target = target.unwrap_or(&compiled.entry_target).to_string();
    let source_panel = source.unwrap_or("").to_string();
    let preview = preview::preview_view(compiled);
    let source_tree = source_tree::source_tree_view(
        &compiled.file_tree,
        route_mode,
        &compiled.app_id,
        selected_target.as_str(),
    );
    let mode_tabs = view! {
        <div class="mode-tabs">
            <a class=if route_mode == UiRouteMode::Manage { "mode-tab active" } else { "mode-tab" } href=manage_href>"编辑态"</a>
            <a class=if route_mode == UiRouteMode::Access { "mode-tab active" } else { "mode-tab" } href=access_href>"访问态"</a>
        </div>
    };
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

    let page = view! {
        <html lang="zh-CN">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <title>{format!("🌸 {} - MeiLang", compiled.title)}</title>
                <link rel="stylesheet" href="/app-assets/app-shell.css"/>
            </head>
            <body>
                <div class="shell">
                    <header class="topbar">
                        <div class="brand">
                            <strong>"🌸 MeiLang"</strong>
                            <span>"AI-Native"</span>
                        </div>
                        <nav class="app-tabs">{app_tabs}</nav>
                        {mode_tabs}
                    </header>
                    <div class="workspace" id="workspace-root">
                        <aside class="sidebar left">
                            <div class="sidebar-header">
                                <div class="panel-heading">
                                    <h2>"资源树"</h2>
                                    <p>{compiled.app_id.clone()}</p>
                                </div>
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
                                    <div class="main-pane-scroll preview-pane-scroll">
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
                                        <div class="panel-heading">
                                            <h3>"源码"</h3>
                                            <p>{selected_target.clone()}</p>
                                        </div>
                                        <pre class="source-block">{source_panel}</pre>
                                        {if compiled.diagnostics.is_empty() {
                                            view! { <></> }.into_any()
                                        } else {
                                            view! {
                                                <section class="source-diagnostics">
                                                    <div class="panel-heading">
                                                        <h3>"编译提示"</h3>
                                                        <p>"最小内核 diagnostics"</p>
                                                    </div>
                                                    {diagnostics}
                                                </section>
                                            }.into_any()
                                        }}
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
                            <div class="sidebar-header">
                                <div class="panel-heading">
                                    <h2>"OpenCode"</h2>
                                    <p>{format!("{} · {}", compiled.app_id, route_mode.slug())}</p>
                                </div>
                            </div>
                            <div class="sidebar-scroll">
                                {opencode::panel_view(compiled, route_mode, selected_target.as_str())}
                            </div>
                        </aside>
                    </div>
                </div>
                {component_scripts(compiled)}
                <script src="/app-assets/opencode-panel.js"></script>
                <script>{workspace::SPLITTER_SCRIPT}</script>
                <script>{source_tree::TREE_SCRIPT}</script>
            </body>
        </html>
    };
    page.to_html()
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
