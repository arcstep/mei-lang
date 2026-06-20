use leptos::prelude::*;
use mei_lang_kernel::{BuildNodeId, BuildViewTab, ReachabilityTreeRoot};

use super::manage_routing::build_node_href;

pub(crate) fn reachability_tree_view(
    roots: &[ReachabilityTreeRoot],
    app_path: &str,
    active_node: &BuildNodeId,
    active_tab: BuildViewTab,
) -> AnyView {
    let items = roots
        .iter()
        .map(|root| root_branch(root, app_path, active_node, active_tab))
        .collect_view();
    view! {
        <div class="build-reachability-tree">
            <ul class="build-tree-list">{items}</ul>
        </div>
    }
    .into_any()
}

fn root_branch(
    root: &ReachabilityTreeRoot,
    app_path: &str,
    active_node: &BuildNodeId,
    active_tab: BuildViewTab,
) -> AnyView {
    let children = root
        .children
        .iter()
        .map(|node| tree_node(node, app_path, active_node, active_tab))
        .collect_view();
    view! {
        <li class="build-tree-node build-tree-node--branch">
            <details class="build-tree-details" open=root.default_open>
                <summary class="build-tree-summary build-tree-summary--root">
                    <span class="build-tree-kind build-tree-kind--group" aria-hidden="true">"▦"</span>
                    <span class="build-tree-label">{root.label.clone()}</span>
                </summary>
                <ul class="build-tree-list build-tree-list--nested">{children}</ul>
            </details>
        </li>
    }
    .into_any()
}

fn tree_node(
    node: &mei_lang_kernel::ReachabilityTreeNode,
    app_path: &str,
    active_node: &BuildNodeId,
    active_tab: BuildViewTab,
) -> AnyView {
    if node.node_id.trim().is_empty() && !node.children.is_empty() {
        let children = node
            .children
            .iter()
            .map(|child| tree_node(child, app_path, active_node, active_tab))
            .collect_view();
        return view! {
            <li class="build-tree-node build-tree-node--branch">
                <details class="build-tree-details" open=true>
                    <summary class="build-tree-summary build-tree-summary--group">
                        <span class="build-tree-kind build-tree-kind--group" aria-hidden="true">"▸"</span>
                        <span class="build-tree-label">{node.label.clone()}</span>
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
        .map(|id| build_node_href(app_path, id, tab_for_node_link(id, active_tab), Default::default()))
        .unwrap_or_else(|| "#".to_string());
    let kind_glyph = kind_glyph(&node.kind);
    let badge = node.badges.first().cloned();

    if node.children.is_empty() {
        let class = if is_active {
            "build-tree-link build-tree-link--active"
        } else {
            "build-tree-link"
        };
        view! {
            <li class="build-tree-node">
                <a class=class href=href title=node.label.clone() data-build-node=node.node_id.clone()>
                    <span class="build-tree-spacer" aria-hidden="true"></span>
                    <span class="build-tree-kind" aria-hidden="true">{kind_glyph}</span>
                    <span class="build-tree-label">{label_with_badge(node.label.clone(), badge)}</span>
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
        view! {
            <li class="build-tree-node build-tree-node--branch">
                <details class="build-tree-details" open=is_active>
                    <summary class=summary_class>
                        <span class="build-tree-kind" aria-hidden="true">{kind_glyph}</span>
                        <a class="build-tree-label build-tree-label--link" href=href data-build-node=node.node_id.clone()>
                            {label_with_badge(node.label.clone(), badge)}
                        </a>
                    </summary>
                    <ul class="build-tree-list build-tree-list--nested">
                        {node
                            .children
                            .iter()
                            .map(|child| tree_node(child, app_path, active_node, active_tab))
                            .collect_view()}
                    </ul>
                </details>
            </li>
        }
        .into_any()
    }
}

fn label_with_badge(label: String, badge: Option<String>) -> AnyView {
    match badge {
        Some(value) if !value.trim().is_empty() => view! {
            <>
                {label}
                <span class="build-tree-badge">{value}</span>
            </>
        }
        .into_any(),
        _ => view! { {label} }.into_any(),
    }
}

fn kind_glyph(kind: &str) -> &'static str {
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
        "artifact" => "A",
        _ => "·",
    }
}

fn tab_for_node_link(node: &BuildNodeId, current: BuildViewTab) -> BuildViewTab {
    use mei_lang_kernel::{tab_visible_for_node, BuildNodeKind, BuildViewTab};
    if tab_visible_for_node(node, current) {
        return current;
    }
    match node.kind {
        BuildNodeKind::Scene
        | BuildNodeKind::ScenePanel
        | BuildNodeKind::SceneBlock
        | BuildNodeKind::Route
        | BuildNodeKind::Projection
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
