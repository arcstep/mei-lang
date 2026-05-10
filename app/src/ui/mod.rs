use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, WorkspaceAppMeta};

mod opencode;
mod preview;
mod route;
mod source_tree;
mod style;

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
                <title>{format!("{} - MeiLang", compiled.title)}</title>
                <style>{style::STYLE}</style>
            </head>
            <body>
                <div class="shell">
                    <header class="topbar">
                        <div class="brand">
                            <strong>"MeiLang"</strong>
                            <span>"scene-first 骨架"</span>
                        </div>
                        <nav class="app-tabs">{app_tabs}</nav>
                        {mode_tabs}
                    </header>
                    <div class="workspace">
                        <aside class="sidebar left">
                            <div class="panel-heading">
                                <h2>"资源树"</h2>
                                <p>{compiled.app_id.clone()}</p>
                            </div>
                            {source_tree}
                        </aside>
                        <main class="main">
                            <div class="panel-heading">
                                <h2>{compiled.title.clone()}</h2>
                                <p>{compiled.entry_target.clone()}</p>
                            </div>
                            <section class="panel preview-panel">
                                {preview}
                            </section>
                            <section class="panel source-panel">
                                <div class="panel-heading">
                                    <h3>"源码预览"</h3>
                                    <p>{selected_target.clone()}</p>
                                </div>
                                <pre class="source-block">{source_panel}</pre>
                            </section>
                            {if compiled.diagnostics.is_empty() {
                                view! { <></> }.into_any()
                            } else {
                                view! {
                                    <section class="panel diagnostics-panel">
                                        <div class="panel-heading">
                                            <h3>"编译提示"</h3>
                                            <p>"最小内核 diagnostics"</p>
                                        </div>
                                        {diagnostics}
                                    </section>
                                }.into_any()
                            }}
                        </main>
                        <aside class="sidebar right">
                            <div class="panel-heading">
                                <h2>"OpenCode"</h2>
                                <p>"宿主桥接"</p>
                            </div>
                            {opencode::panel_view(compiled, route_mode)}
                        </aside>
                    </div>
                </div>
                {component_scripts(compiled)}
                <script>{opencode::BOOTSTRAP_SCRIPT}</script>
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
