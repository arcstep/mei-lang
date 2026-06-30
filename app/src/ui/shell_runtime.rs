use leptos::prelude::*;
use mei_lang_kernel::{
    ReachabilityTreeNode, ReachabilityTreeRoot, WorkspaceAppMeta,
};

use super::manage_routing::runtime_node_href;
use super::route::UiRouteMode;
use super::runtime_panels::{runtime_json_panel, runtime_overview_panel};
use super::runtime_snapshot_view::parse_runtime_snapshot;
use super::runtime_tree::runtime_observability_tree_view;
use super::statusbar::statusbar_view;
use super::topbar::{access_scene_for_topbar, topbar_view};
use super::{HostAccountView, TopbarMenuContext};

const RUNTIME_TAB_DETAIL: &str = "overview";
const RUNTIME_TAB_NODE_JSON: &str = "json";
const RUNTIME_TAB_SNAPSHOT_JSON: &str = "snapshot-json";

pub(crate) fn runtime_shell(
    apps: &[WorkspaceAppMeta],
    compiled: &mei_lang_kernel::CompiledApp,
    app_path: &str,
    topbar_menu: Option<&TopbarMenuContext>,
    runtime_roots: &[ReachabilityTreeRoot],
    active_node: Option<&str>,
    active_tab: Option<&str>,
    runtime_snapshot_json: Option<&str>,
    upload_enabled: bool,
    auth_enabled: bool,
    auth_account: Option<&HostAccountView>,
) -> AnyView {
    let active_node_id = active_node
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("ops:overview");
    let active_tab_slug = active_tab
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(RUNTIME_TAB_DETAIL);
    let detail_active = active_tab_slug == RUNTIME_TAB_DETAIL;
    let node_json_active = active_tab_slug == RUNTIME_TAB_NODE_JSON;
    let snapshot_json_active = active_tab_slug == RUNTIME_TAB_SNAPSHOT_JSON;
    let snapshot = runtime_snapshot_json.and_then(parse_runtime_snapshot);
    let selected = find_runtime_node(active_node_id, runtime_roots);
    let tree = runtime_observability_tree_view(runtime_roots, app_path, Some(active_node_id));
    let overview_panel = runtime_overview_panel(
        snapshot.as_ref(),
        selected,
        active_node_id,
        app_path,
    );
    let node_json_panel = runtime_json_panel(
        "当前节点 JSON",
        "host-runtime-node-json",
        "{}",
        Some("此面板会随左侧节点切换，展示 route / scope / slot / summary 的上下文切片。"),
    );
    let snapshot_json_panel = runtime_json_panel(
        "完整快照 JSON · /api/runtime/snapshot",
        "host-runtime-snapshot-json",
        runtime_snapshot_json.unwrap_or("{}"),
        Some("整份 host-shell runtime snapshot，适合导出或在无法定位节点时做兜底排查。"),
    );
    let detail_href = runtime_node_href(app_path, active_node_id, Some(RUNTIME_TAB_DETAIL));
    let node_json_href = runtime_node_href(app_path, active_node_id, Some(RUNTIME_TAB_NODE_JSON));
    let snapshot_json_href =
        runtime_node_href(app_path, active_node_id, Some(RUNTIME_TAB_SNAPSHOT_JSON));
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
        Some(active_tab_slug),
        None,
        None,
        upload_enabled,
        false,
        auth_enabled,
        auth_account,
    );
    let statusbar = statusbar_view(app_path, UiRouteMode::Runtime.slug(), "", None);
    let snapshot_json =
        runtime_snapshot_json.unwrap_or("{}").to_string();
    let roots_json =
        serde_json::to_string(runtime_roots).unwrap_or_else(|_| "[]".to_string());
    view! {
        <div
            class="shell shell-surface mei-text-primary"
            data-app-path=app_path.to_string()
            data-runtime-node=active_node_id.to_string()
            data-runtime-tab=active_tab_slug.to_string()
        >
            <script
                id="mei-runtime-observability-tree"
                type="application/json"
                inner_html=roots_json
            ></script>
            <script
                id="mei-runtime-observability-snapshot"
                type="application/json"
                inner_html=snapshot_json
            ></script>
            {topbar}
            <div class="workspace manage-workspace runtime-workspace host-runtime-console-workspace chrome-inset min-h-0 h-full overflow-hidden px-0 py-0 grid gap-0" id="workspace-root">
                <aside class="sidebar left workspace-panel workspace-panel-side workspace-panel-nav h-full min-h-0 min-w-0 overflow-hidden flex flex-col px-4 py-2.5" id="host-runtime-nav-aside">
                    <div class="sidebar-scroll flex-1 min-h-0 overflow-auto" id="host-runtime-nav-scroll">
                        <div class="runtime-tree-toolbar mb-2 flex items-center justify-between gap-2">
                            <span class="mei-font-1 mei-text-muted">"Host 管理"</span>
                            <button type="button" id="runtime-refresh-btn" class="build-toolbar-btn" data-app-path=app_path.to_string()>"刷新"</button>
                        </div>
                        <div id="host-runtime-nav-mount" class="host-runtime-nav-mount"></div>
                        <div class="host-runtime-legacy-tree" data-host-runtime-legacy-tree="1">{tree}</div>
                    </div>
                </aside>
                <div class="splitter splitter-left" data-workspace-splitter="left" role="separator" aria-orientation="vertical" aria-label="调整左侧资源栏宽度"></div>
                <main class="main h-full min-w-0 min-h-0 overflow-hidden px-0">
                    <section class="main-pane workspace-panel workspace-panel-main min-w-0 min-h-0 flex h-full flex-col overflow-hidden px-2 py-3.5">
                        <div class="manage-workspace-head mb-3 flex min-w-0 flex-wrap items-center justify-between gap-2 pb-2.5">
                            <nav
                                class="manage-view-tabs workspace-tabs-strip flex min-w-0 flex-1 flex-wrap items-center gap-2"
                                role="tablist"
                                aria-label="运行态主视图"
                            >
                                <div class="manage-view-tabs-cluster">
                                    <div class="manage-view-tabs-group" role="presentation">
                                        <a
                                            class=if detail_active { "manage-view-tab is-active" } else { "manage-view-tab" }
                                            href=detail_href
                                            role="tab"
                                            aria-selected=if detail_active { "true" } else { "false" }
                                            data-runtime-tab=RUNTIME_TAB_DETAIL
                                        >
                                            <span class="manage-view-tab-label">"详情"</span>
                                        </a>
                                        <a
                                            class=if node_json_active { "manage-view-tab is-active" } else { "manage-view-tab" }
                                            href=node_json_href
                                            role="tab"
                                            aria-selected=if node_json_active { "true" } else { "false" }
                                            data-runtime-tab=RUNTIME_TAB_NODE_JSON
                                        >
                                            <span class="manage-view-tab-label">"当前节点 JSON"</span>
                                        </a>
                                        <a
                                            class=if snapshot_json_active { "manage-view-tab is-active" } else { "manage-view-tab" }
                                            href=snapshot_json_href
                                            role="tab"
                                            aria-selected=if snapshot_json_active { "true" } else { "false" }
                                            data-runtime-tab=RUNTIME_TAB_SNAPSHOT_JSON
                                        >
                                            <span class="manage-view-tab-label">"完整快照 JSON"</span>
                                        </a>
                                    </div>
                                </div>
                            </nav>
                        </div>
                        <div class="manage-tab-stage min-h-0 min-w-0 flex flex-1 flex-col overflow-hidden">
                            <section
                                class="manage-tab-panel min-h-0 min-w-0 overflow-auto"
                                data-runtime-tab-panel=RUNTIME_TAB_DETAIL
                                hidden=!detail_active
                            >
                                <div id="host-runtime-detail-mount" class="host-runtime-detail-mount"></div>
                                <div class="host-runtime-legacy-overview" data-host-runtime-legacy-overview="1">{overview_panel}</div>
                            </section>
                            <section
                                class="manage-tab-panel min-h-0 min-w-0 overflow-auto"
                                data-runtime-tab-panel=RUNTIME_TAB_NODE_JSON
                                hidden=!node_json_active
                            >
                                {node_json_panel}
                            </section>
                            <section
                                class="manage-tab-panel min-h-0 min-w-0 overflow-auto"
                                data-runtime-tab-panel=RUNTIME_TAB_SNAPSHOT_JSON
                                hidden=!snapshot_json_active
                            >
                                {snapshot_json_panel}
                            </section>
                        </div>
                    </section>
                </main>
            </div>
            {statusbar}
        </div>
    }
    .into_any()
}

fn find_runtime_node<'a>(
    active_node_id: &str,
    roots: &'a [ReachabilityTreeRoot],
) -> Option<&'a ReachabilityTreeNode> {
    for root in roots {
        for node in &root.children {
            if let Some(found) = find_runtime_node_recursive(active_node_id, node) {
                return Some(found);
            }
        }
    }
    None
}

fn find_runtime_node_recursive<'a>(
    active_node_id: &str,
    node: &'a ReachabilityTreeNode,
) -> Option<&'a ReachabilityTreeNode> {
    if node.node_id == active_node_id {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_runtime_node_recursive(active_node_id, child) {
            return Some(found);
        }
    }
    None
}
