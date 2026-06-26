use leptos::prelude::*;
use mei_lang_kernel::{
    BuildExecScope, BuildNodeId, BuildNodeKind, BuildViewTab, ReachabilityTreeNode,
    ReachabilityTreeRoot, WorkspaceAppMeta,
};

use super::manage_routing::build_node_href;
use super::route::UiRouteMode;
use super::runtime_tree::runtime_observability_tree_view;
use super::statusbar::statusbar_view;
use super::topbar::{access_scene_for_topbar, topbar_view};
use super::{HostAccountView, TopbarMenuContext};

pub(crate) fn runtime_shell(
    apps: &[WorkspaceAppMeta],
    compiled: &mei_lang_kernel::CompiledApp,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    runtime_roots: &[ReachabilityTreeRoot],
    active_node: Option<&str>,
    upload_enabled: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> AnyView {
    let active_node_id = active_node
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("mrg-slot-summary");
    let tree = runtime_observability_tree_view(runtime_roots, app_path, Some(active_node_id));
    let topbar = topbar_view(
        apps,
        app_path,
        topbar_menu,
        UiRouteMode::Runtime,
        access_scene_for_topbar(
            UiRouteMode::Runtime,
            compiled,
            compiled.active_scene.as_deref(),
            None,
        ),
        None,
        Some("overview"),
        None,
        None,
        upload_enabled,
        false,
        auth_enabled,
        auth_account,
    );
    let statusbar = statusbar_view(app_path, UiRouteMode::Runtime.slug(), "", None);
    let snapshot_json =
        serde_json::to_string(runtime_roots).unwrap_or_else(|_| "[]".to_string());
    let detail = runtime_detail_panel(active_node_id, app_path, runtime_roots);
    view! {
        <div class="shell shell-surface mei-text-primary" data-app-path=app_path.to_string() data-runtime-node=active_node_id.to_string()>
            <script
                id="mei-runtime-observability-tree"
                type="application/json"
                inner_html=snapshot_json
            ></script>
            {topbar}
            <div class="workspace runtime-workspace chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0" id="workspace-root">
                <aside class="sidebar left workspace-panel workspace-panel-side workspace-panel-nav h-full min-h-0 min-w-0 overflow-hidden flex flex-col px-4 py-2.5">
                    <div class="sidebar-scroll flex-1 min-h-0 overflow-auto">
                        <div class="runtime-tree-toolbar mb-2 flex items-center justify-between gap-2">
                            <span class="mei-font-1 mei-text-muted">"运行态观测"</span>
                            <button type="button" id="runtime-refresh-btn" class="build-toolbar-btn" data-app-path=app_path.to_string()>"刷新"</button>
                        </div>
                        {tree}
                    </div>
                </aside>
                <div class="splitter splitter-left" data-workspace-splitter="left" role="separator" aria-orientation="vertical" aria-label="调整左侧观测树宽度"></div>
                <main class="main h-full min-w-0 min-h-0 overflow-hidden px-0">
                    <section class="main-pane workspace-panel workspace-panel-main min-w-0 min-h-0 flex h-full flex-col overflow-hidden px-4 py-3.5">
                        {detail}
                    </section>
                </main>
            </div>
            {statusbar}
        </div>
    }
    .into_any()
}

fn runtime_detail_panel(active_node_id: &str, app_path: &str, roots: &[ReachabilityTreeRoot]) -> AnyView {
    let selected = find_runtime_node(active_node_id, roots);
    let title = selected
        .map(|node| node.label.clone())
        .unwrap_or_else(|| "运行态概览".to_string());
    let badges: Vec<String> = selected
        .map(|node| node.badges.clone())
        .unwrap_or_default();
    let build_cross_link = mcg_build_href_for_runtime_node(app_path, active_node_id);
    view! {
        <article class="runtime-detail-panel min-h-0 flex-1 overflow-auto">
            <header class="mb-3 border-b border-white/10 pb-2">
                <h2 class="mei-font-3 mei-text-primary">{title}</h2>
                <div class="mt-1 flex flex-wrap gap-2">
                    {badges.into_iter().map(|badge| view! {
                        <span class="build-tree-badge build-tree-badge--meta">{badge}</span>
                    }).collect_view()}
                </div>
            </header>
            <p class="mei-font-2 mei-text-body mb-3">
                "此视图展示 MRG materialization、预热进度、L1 策略与 diagnostics 告警的当前态；左侧树每 5 秒自动刷新。"
            </p>
            {build_cross_link.map(|href| view! {
                <div class="mb-3 flex flex-col gap-1 rounded-lg border border-white/10 bg-black/10 p-3">
                    <span class="mei-font-1 mei-text-muted">"对应 MCG 编译检查点在构建视图的 Compile · MCG 分组。"</span>
                    <a class="build-toolbar-btn inline-flex w-fit" href=href>"在构建视图查看 MCG 检查点"</a>
                </div>
            })}
            <p class="mei-font-1 mei-text-muted mb-3">
                "无特定 slot 选中时，请从左侧 MRG · Materialization 树选择节点。"
            </p>
            <pre class="runtime-detail-json mt-4 overflow-auto rounded bg-black/20 p-3 mei-font-1 mei-text-body" id="runtime-detail-json">{"{}"}</pre>
        </article>
    }
    .into_any()
}

fn mcg_build_href_for_runtime_node(app_path: &str, runtime_node_id: &str) -> Option<String> {
    let rest = runtime_node_id.strip_prefix("mrg-slot:")?;
    let slot_key = rest.split('@').next()?.trim();
    if slot_key.is_empty() {
        return None;
    }
    for prefix in ["metric_def_bundle", "scene_payload", "assembly_view"] {
        let node = BuildNodeId::new(
            BuildNodeKind::McgNode,
            format!("{prefix}:{slot_key}"),
        );
        return Some(build_node_href(
            app_path,
            &node,
            BuildViewTab::Overview,
            BuildExecScope::Warmup,
            None,
            None,
        ));
    }
    None
}

fn find_runtime_node<'a>(
    active_node_id: &str,
    roots: &'a [ReachabilityTreeRoot],
) -> Option<&'a ReachabilityTreeNode> {
    for root in roots {
        for node in &root.children {
            if node.node_id == active_node_id {
                return Some(node);
            }
        }
    }
    None
}
