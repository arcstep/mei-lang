use leptos::prelude::*;
use mei_lang_kernel::{
    is_stock_catalog_facet_root, BuildNodeId, BuildViewTab, ReachabilityTreeNode,
    ReachabilityTreeRoot,
};

use super::manage_routing::{build_node_href, BuildReviewAxes};

pub(crate) fn reachability_tree_view(
    roots: &[ReachabilityTreeRoot],
    app_path: &str,
    active_node: &BuildNodeId,
    active_tab: BuildViewTab,
    catalog: Option<&str>,
    stock_pack: Option<&str>,
    review_axes: BuildReviewAxes<'_>,
) -> AnyView {
    let flatten_facet = stock_pack.is_some();
    let mut items = Vec::new();
    for root in roots {
        if flatten_facet && is_stock_catalog_facet_root(root.group.as_str()) {
            for node in &root.children {
                items.push(tree_node(
                    node,
                    app_path,
                    active_node,
                    active_tab,
                    catalog,
                    stock_pack,
                    review_axes,
                ));
            }
        } else {
            items.push(root_branch(
                root,
                app_path,
                active_node,
                active_tab,
                catalog,
                stock_pack,
                review_axes,
            ));
        }
    }
    let items = items.into_view();
    view! {
        <div class="build-tree-shell" data-build-tree-shell="true">
            <div class="build-tree-mode-toggle mb-2 flex gap-1" role="tablist" aria-label="资源树视图">
                <button type="button" class="build-tree-mode-btn is-active" data-build-tree-mode="structure">"结构"</button>
                <button type="button" class="build-tree-mode-btn" data-build-tree-mode="compile">"编译"</button>
            </div>
            <div class="build-reachability-tree" data-build-tree-mode-active="structure">
                <ul class="build-tree-list">{items}</ul>
            </div>
        </div>
    }
    .into_any()
}

fn root_branch(
    root: &ReachabilityTreeRoot,
    app_path: &str,
    active_node: &BuildNodeId,
    active_tab: BuildViewTab,
    catalog: Option<&str>,
    stock_pack: Option<&str>,
    review_axes: BuildReviewAxes<'_>,
) -> AnyView {
    let child_count = root.children.len();
    let children = root
        .children
        .iter()
        .map(|node| tree_node(node, app_path, active_node, active_tab, catalog, stock_pack, review_axes))
        .collect_view();
    view! {
        <li class="build-tree-node build-tree-node--branch">
            <details
                class="build-tree-details"
                data-build-tree-branch=format!("root:{}", root.group.clone())
                data-build-tree-root-group=root.group.clone()
                data-build-tree-children-count=child_count.to_string()
            >
                <summary class="build-tree-summary build-tree-summary--root">
                    <span class="build-tree-kind build-tree-kind--root" aria-hidden="true">"▦"</span>
                    <span class="build-tree-label">
                        {branch_label(reachability_root_label(root), None, child_count)}
                    </span>
                </summary>
                <ul class="build-tree-list build-tree-list--nested">{children}</ul>
            </details>
        </li>
    }
    .into_any()
}

fn reachability_root_label(root: &ReachabilityTreeRoot) -> String {
    match root.group.as_str() {
        "templates" => "组件".to_string(),
        "template_files" => "模板".to_string(),
        "mcg" => "Compile · MCG".to_string(),
        _ => root.label.clone(),
    }
}

fn tree_node(
    node: &ReachabilityTreeNode,
    app_path: &str,
    active_node: &BuildNodeId,
    active_tab: BuildViewTab,
    catalog: Option<&str>,
    stock_pack: Option<&str>,
    review_axes: BuildReviewAxes<'_>,
) -> AnyView {
    let child_count = node.children.len();
    if node.node_id.trim().is_empty() && !node.children.is_empty() {
        if node.kind == "template_group" || node.kind == "component_pack" {
            return template_category_section(
                node,
                app_path,
                active_node,
                active_tab,
                catalog,
                stock_pack,
                review_axes,
            );
        }
        let children = node
            .children
            .iter()
            .map(|child| {
                tree_node(
                    child,
                    app_path,
                    active_node,
                    active_tab,
                    catalog,
                    stock_pack,
                    review_axes,
                )
            })
            .collect_view();
        let branch_id = format!("group:{}", node.id);
        return view! {
            <li class="build-tree-node build-tree-node--branch">
                <details
                    class="build-tree-details"
                    data-build-tree-branch=branch_id
                    data-build-tree-children-count=child_count.to_string()
                >
                    <summary class="build-tree-summary build-tree-summary--group">
                        <span class="build-tree-label">
                            {branch_label(node.label.clone(), None, child_count)}
                        </span>
                    </summary>
                    <ul class="build-tree-list build-tree-list--nested">{children}</ul>
                </details>
            </li>
        }
        .into_any();
    }

    let parsed = BuildNodeId::parse(&node.node_id);
    let is_active = parsed.as_ref() == Some(active_node);
    let href = parsed
        .as_ref()
        .map(|id| {
            build_node_href(
                app_path,
                id,
                tab_for_node_link(id, active_tab),
                Default::default(),
                catalog,
                stock_pack,
                review_axes,
            )
        })
        .unwrap_or_else(|| "#".to_string());
    let kind_glyph = kind_glyph(&node.kind, node);
    let badge = node.badges.first().cloned();

    if node.children.is_empty() {
        let class = if is_active {
            "build-tree-link build-tree-link--active"
        } else {
            "build-tree-link"
        };
        view! {
            <li class="build-tree-node">
                <a
                    class=class
                    href=href
                    title=node.label.clone()
                    data-build-node=node.node_id.clone()
                    data-compile-scene=node.compile_scene.clone()
                    data-compile-target=node.compile_target.clone()
                    data-board-layout-zone=node.board_layout_zone.clone()
                    data-source-file=node.source_file.clone()
                    data-source-symbol=node.source_symbol.clone()
                >
                    <span class="build-tree-spacer" aria-hidden="true"></span>
                    <span class="build-tree-kind" aria-hidden="true">{kind_glyph}</span>
                    <span class="build-tree-label">{leaf_label(node.label.clone(), badge)}</span>
                </a>
            </li>
        }
        .into_any()
    } else {
        let summary_class = if is_active {
            "build-tree-summary build-tree-summary--active"
        } else {
            "build-tree-summary"
        };
        let branch_id = if node.id.trim().is_empty() {
            node.node_id.clone()
        } else {
            node.id.clone()
        };
        let default_open = ui_scope_branch_default_open(node);
        view! {
            <li class="build-tree-node build-tree-node--branch">
                <details
                    class="build-tree-details"
                    data-build-tree-branch=branch_id
                    data-build-tree-children-count=child_count.to_string()
                    open=default_open
                >
                    <summary class=summary_class>
                        <span class="build-tree-kind" aria-hidden="true">{kind_glyph}</span>
                        <a
                            class="build-tree-label build-tree-label--link"
                            href=href
                            data-build-node=node.node_id.clone()
                            data-compile-scene=node.compile_scene.clone()
                            data-compile-target=node.compile_target.clone()
                            data-board-layout-zone=node.board_layout_zone.clone()
                            data-source-file=node.source_file.clone()
                            data-source-symbol=node.source_symbol.clone()
                        >
                            {branch_label(node.label.clone(), badge, child_count)}
                        </a>
                    </summary>
                    <ul class="build-tree-list build-tree-list--nested">
                        {node
                            .children
                            .iter()
                            .map(|child| {
                                tree_node(
                                    child,
                                    app_path,
                                    active_node,
                                    active_tab,
                                    catalog,
                                    stock_pack,
                                    review_axes,
                                )
                            })
                            .collect_view()}
                    </ul>
                </details>
            </li>
        }
        .into_any()
    }
}

fn template_category_section(
    node: &ReachabilityTreeNode,
    app_path: &str,
    active_node: &BuildNodeId,
    active_tab: BuildViewTab,
    catalog: Option<&str>,
    stock_pack: Option<&str>,
    review_axes: BuildReviewAxes<'_>,
) -> AnyView {
    let child_count = node.children.len();
    let children = node
        .children
        .iter()
        .map(|child| {
            tree_node(
                child,
                app_path,
                active_node,
                active_tab,
                catalog,
                stock_pack,
                review_axes,
            )
        })
        .collect_view();
    view! {
        <li class="build-tree-node build-tree-node--branch">
            <details
                class="build-tree-details"
                data-build-tree-branch=node.id.clone()
                data-build-tree-children-count=child_count.to_string()
            >
                <summary class="build-tree-summary build-tree-summary--group">
                    <span class="build-tree-kind" aria-hidden="true">{kind_glyph(&node.kind, node)}</span>
                    <span class="build-tree-label">
                        {branch_label(node.label.clone(), None, child_count)}
                    </span>
                </summary>
                <ul class="build-tree-list build-tree-list--nested">{children}</ul>
            </details>
        </li>
    }
    .into_any()
}

fn leaf_label(label: String, meta_badge: Option<String>) -> AnyView {
    match meta_badge {
        Some(value) if !value.trim().is_empty() => view! {
            <>
                {label}
                <span class="build-tree-badge build-tree-badge--meta">{value}</span>
            </>
        }
        .into_any(),
        _ => view! { {label} }.into_any(),
    }
}

fn branch_label(label: String, meta_badge: Option<String>, child_count: usize) -> AnyView {
    let meta = meta_badge.filter(|value| !value.trim().is_empty());
    match (meta, child_count > 0) {
        (Some(meta_value), true) => view! {
            <>
                {label}
                <span class="build-tree-badge build-tree-badge--meta">{meta_value}</span>
                <span
                    class="build-tree-badge build-tree-badge--count"
                    title=format!("{child_count} 个子节点")
                >
                    {child_count}
                </span>
            </>
        }
        .into_any(),
        (Some(meta_value), false) => view! {
            <>
                {label}
                <span class="build-tree-badge build-tree-badge--meta">{meta_value}</span>
            </>
        }
        .into_any(),
        (None, true) => view! {
            <>
                {label}
                <span
                    class="build-tree-badge build-tree-badge--count"
                    title=format!("{child_count} 个子节点")
                >
                    {child_count}
                </span>
            </>
        }
        .into_any(),
        (None, false) => view! { {label} }.into_any(),
    }
}

fn kind_glyph(kind: &str, node: &ReachabilityTreeNode) -> &'static str {
    match kind {
        "route" => "R",
        "scene" => "S",
        "scene_panel" => "P",
        "scene_block" => "B",
        "projection" => "O",
        "world_dataset" | "world_metric" | "world_file" => "W",
        "explain_block" => "E",
        "dataset" => "D",
        "component" => "C",
        "board_file" => "B",
        "board_slot" => "S",
        "template" => "T",
        "template_file" => "F",
        "template_group" | "component_pack" => "G",
        "artifact" => "A",
        "mcg_node" => "M",
        "ui_scope" => ui_scope_glyph(node),
        _ => "·",
    }
}

fn ui_scope_glyph(node: &ReachabilityTreeNode) -> &'static str {
    match node.badges.first().map(String::as_str) {
        Some("plane") => "P",
        Some("region") => "R",
        Some("section") => "§",
        Some("micro_layout") => "M",
        Some("slot") => "L",
        Some("content") => "C",
        Some("budget") => "B",
        Some("scene") => "S",
        _ => "U",
    }
}

fn ui_scope_branch_default_open(node: &ReachabilityTreeNode) -> bool {
    if node.kind != "ui_scope" {
        return false;
    }
    matches!(
        node.badges.first().map(String::as_str),
        Some("scene") | Some("plane") | Some("region") | Some("section")
    )
}

fn tab_for_node_link(node: &BuildNodeId, current: BuildViewTab) -> BuildViewTab {
    use mei_lang_kernel::{tab_visible_for_node, BuildNodeKind, BuildViewTab};
    if node.kind == BuildNodeKind::UiScope {
        return BuildViewTab::Preview;
    }
    if tab_visible_for_node(node, current) {
        return current;
    }
    match node.kind {
        BuildNodeKind::Scene
        | BuildNodeKind::ScenePanel
        | BuildNodeKind::SceneBlock
        | BuildNodeKind::UiScope
        | BuildNodeKind::Route
        | BuildNodeKind::Projection
        | BuildNodeKind::Template
        | BuildNodeKind::Component
        | BuildNodeKind::BoardFile
        | BuildNodeKind::BoardSlot
        | BuildNodeKind::WorldMetric
        | BuildNodeKind::WorldDataset
        | BuildNodeKind::WorldExplain
        | BuildNodeKind::Dataset => {
            if tab_visible_for_node(node, BuildViewTab::Preview) {
                return BuildViewTab::Preview;
            }
        }
        _ => {}
    }
    node.default_tab()
}
