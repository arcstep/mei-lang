use std::collections::BTreeMap;

use leptos::prelude::*;
use mei_lang_kernel::{BlockDecl, CompiledApp, DatasetView, WorkspaceAppMeta, WorkspaceNode};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiRouteMode {
    Manage,
    Access,
}

impl UiRouteMode {
    pub fn from_slug(value: &str) -> Self {
        match value {
            "access" | "run" => Self::Access,
            _ => Self::Manage,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Manage => "manage",
            Self::Access => "access",
        }
    }
}

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
    let title = compiled.title.clone();
    let source_panel = source.unwrap_or("").to_string();
    let preview = preview_view(compiled);
    let source_tree = source_tree_view(
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
                <title>{format!("{} - MeiLang", title)}</title>
                <style>{STYLE}</style>
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
                                <p>"集成预留"</p>
                            </div>
                            <div class="opencode-placeholder">
                                <p>"右侧面板已保留，后续阶段接入真正的会话、权限与上下文能力。"</p>
                                <ul>
                                    <li>{format!("当前应用：{}", compiled.app_id)}</li>
                                    <li>{format!("入口脚本：{}", compiled.entry_target)}</li>
                                    <li>{format!("模式：{}", route_mode.slug())}</li>
                                </ul>
                            </div>
                        </aside>
                    </div>
                </div>
                {component_scripts(compiled)}
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

fn preview_view(compiled: &CompiledApp) -> AnyView {
    let dataset_map = compiled
        .datasets
        .iter()
        .map(|dataset| (dataset.id.clone(), dataset.clone()))
        .collect::<BTreeMap<_, _>>();

    if !compiled.blocks.is_empty() {
        let blocks = compiled
            .blocks
            .iter()
            .map(|block| block_view(block, compiled, &dataset_map))
            .collect_view();
        return view! {
            <div class="preview-stack">
                {blocks}
            </div>
        }
        .into_any();
    }

    if let Some(scene_contract) = &compiled.scene_contract {
        let entities = scene_contract
            .world
            .as_ref()
            .map(|world| world.entities.len())
            .unwrap_or_default();
        let panels = scene_contract.panels.len();
        return view! {
            <section class="scene-placeholder">
                <h3>{scene_contract.scene.id.clone()}</h3>
                <p>{scene_contract.scene.summary.clone().unwrap_or_else(|| "已生成 scene contract，运行态将在后续阶段接入。".to_string())}</p>
                <ul>
                    <li>{format!("实体数量：{}", entities)}</li>
                    <li>{format!("观察面区块：{}", panels)}</li>
                    <li>{format!("目标：{}", scene_contract.scene.goal.clone().unwrap_or_else(|| "未声明".to_string()))}</li>
                </ul>
            </section>
        }
        .into_any();
    }

    view! { <div class="empty-preview">"当前入口还没有可渲染的 frame 或 scene。"</div> }.into_any()
}

fn block_view(
    block: &BlockDecl,
    compiled: &CompiledApp,
    datasets: &BTreeMap<String, DatasetView>,
) -> AnyView {
    let title = block
        .title
        .clone()
        .unwrap_or_else(|| block.use_key.clone());
    let area = block.area.clone().unwrap_or_else(|| "auto".to_string());
    let mut props = block.props.clone();
    if let Some(data_ref) = &block.data_ref {
        if let Some(dataset) = datasets.get(data_ref) {
            props["dataset"] = json!({
                "id": dataset.id,
                "title": dataset.title,
                "columns": dataset.columns,
                "rows": dataset.rows,
            });
        }
    }
    let tag = compiled
        .component_assets
        .iter()
        .find(|asset| asset.key == block.use_key)
        .map(|asset| asset.tag.clone())
        .unwrap_or_else(|| "mei-missing-component".to_string());
    let html = component_html(tag.as_str(), &props);
    view! {
        <section class="preview-card" data-area=area>
            <div class="panel-heading">
                <h3>{title}</h3>
                <p>{block.use_key.clone()}</p>
            </div>
            <div class="component-host" inner_html=html></div>
        </section>
    }
    .into_any()
}

fn component_html(tag: &str, props: &Value) -> String {
    let props = escape_html_attr(&serde_json::to_string(props).unwrap_or_else(|_| "{}".to_string()));
    format!("<{tag} data-props=\"{props}\"></{tag}>")
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn source_tree_view(
    nodes: &[WorkspaceNode],
    route_mode: UiRouteMode,
    app_id: &str,
    selected_target: &str,
) -> AnyView {
    let items = nodes
        .iter()
        .map(|node| {
            if node.kind == "dir" {
                let open = selected_target.starts_with(&format!("{}/", node.path));
                let children = source_tree_view(&node.children, route_mode, app_id, selected_target);
                view! {
                    <li class="tree-node">
                        <details open=open>
                            <summary>{node.name.clone()}</summary>
                            {children}
                        </details>
                    </li>
                }
                .into_any()
            } else {
                let href = format!("/apps/{}/{}?target={}", route_mode.slug(), app_id, node.path);
                let class = if node.path == selected_target {
                    "tree-link active"
                } else {
                    "tree-link"
                };
                view! {
                    <li class="tree-node">
                        <a class=class href=href>{node.name.clone()}</a>
                    </li>
                }
                .into_any()
            }
        })
        .collect_view();
    view! { <ul class="tree">{items}</ul> }.into_any()
}

const STYLE: &str = r#"
* { box-sizing: border-box; }
body { margin: 0; font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #0f172a; color: #e2e8f0; }
a { color: inherit; text-decoration: none; }
.shell { min-height: 100vh; display: grid; grid-template-rows: auto 1fr; }
.topbar { display: grid; grid-template-columns: 220px 1fr auto; gap: 16px; align-items: center; padding: 14px 20px; border-bottom: 1px solid rgba(148,163,184,.16); background: rgba(15,23,42,.92); position: sticky; top: 0; z-index: 10; }
.brand { display: grid; gap: 2px; }
.brand strong { font-size: 16px; }
.brand span { color: #94a3b8; font-size: 12px; }
.app-tabs, .mode-tabs { display: flex; flex-wrap: wrap; gap: 8px; }
.app-tab, .mode-tab { padding: 8px 12px; border: 1px solid rgba(96,165,250,.24); border-radius: 999px; background: rgba(30,41,59,.8); color: #cbd5e1; font-size: 13px; }
.app-tab.active, .mode-tab.active { background: rgba(37,99,235,.32); color: #eff6ff; border-color: rgba(96,165,250,.54); }
.workspace { min-height: 0; display: grid; grid-template-columns: 260px minmax(0, 1fr) 320px; gap: 16px; padding: 16px; }
.sidebar, .panel { border: 1px solid rgba(148,163,184,.14); border-radius: 16px; background: rgba(15,23,42,.78); }
.sidebar { padding: 14px; min-height: calc(100vh - 94px); overflow: auto; }
.main { min-width: 0; display: grid; gap: 16px; }
.panel { padding: 14px; }
.panel-heading { display: grid; gap: 4px; margin-bottom: 12px; }
.panel-heading h2, .panel-heading h3 { margin: 0; font-size: 15px; color: #f8fafc; }
.panel-heading p { margin: 0; color: #94a3b8; font-size: 12px; }
.preview-panel { min-height: 360px; }
.preview-stack { display: grid; gap: 14px; }
.preview-card { display: grid; gap: 10px; padding: 12px; border: 1px solid rgba(59,130,246,.18); border-radius: 14px; background: rgba(2,6,23,.32); }
.component-host { min-height: 80px; }
.source-block { margin: 0; padding: 12px; border-radius: 12px; background: #020617; color: #cbd5e1; font-size: 12px; white-space: pre-wrap; overflow: auto; }
.tree { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
.tree-node details { padding-left: 4px; }
.tree-node summary { cursor: pointer; color: #cbd5e1; font-size: 13px; }
.tree-link { display: block; padding: 8px 10px; border-radius: 10px; color: #cbd5e1; font-size: 13px; background: rgba(30,41,59,.58); }
.tree-link.active { background: rgba(37,99,235,.28); color: #eff6ff; }
.opencode-placeholder { display: grid; gap: 10px; color: #cbd5e1; font-size: 13px; }
.opencode-placeholder ul { margin: 0; padding-left: 18px; color: #94a3b8; }
.diag { display: grid; gap: 4px; padding: 10px 12px; border-radius: 12px; margin-top: 8px; }
.diag-error { background: rgba(127,29,29,.25); border: 1px solid rgba(248,113,113,.28); }
.diag-warning { background: rgba(120,53,15,.22); border: 1px solid rgba(251,191,36,.28); }
.diag-info { background: rgba(30,64,175,.22); border: 1px solid rgba(96,165,250,.28); }
.scene-placeholder, .empty-preview { padding: 16px; border-radius: 14px; background: rgba(2,6,23,.36); border: 1px solid rgba(59,130,246,.18); }
.scene-placeholder h3 { margin: 0 0 8px; }
.scene-placeholder p, .empty-preview { color: #cbd5e1; }
.scene-placeholder ul { margin: 12px 0 0; padding-left: 18px; color: #94a3b8; }
@media (max-width: 1200px) {
  .workspace { grid-template-columns: 1fr; }
  .sidebar { min-height: 0; }
}
"#;
