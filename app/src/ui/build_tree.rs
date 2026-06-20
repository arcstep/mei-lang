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
    view! { <ul class="tree m-0 grid list-none gap-0.5 p-0">{items}</ul> }.into_any()
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
        .map(|node| tree_node(node, app_path, active_node, active_tab, 0))
        .collect_view();
    view! {
        <li class="tree-node tree-li-branch">
            <details class="pl-1" open=root.default_open>
                <summary class="tree-folder-summary flex min-w-0 cursor-pointer select-none items-center gap-1 py-1 text-xs font-semibold mei-text-body">
                    <span class="tree-folder-label min-w-0 truncate">{root.label.clone()}</span>
                </summary>
                <ul class="tree m-0 grid list-none gap-0.5 p-0 pl-2">{children}</ul>
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
    depth: usize,
) -> AnyView {
    if node.node_id.trim().is_empty() && !node.children.is_empty() {
        let children = node
            .children
            .iter()
            .map(|child| tree_node(child, app_path, active_node, active_tab, depth + 1))
            .collect_view();
        return view! {
            <li class="tree-node tree-li-branch">
                <details class="pl-1" open=true>
                    <summary class="tree-folder-summary flex min-w-0 cursor-pointer select-none items-center gap-1 py-1 text-xs font-medium mei-text-muted">
                        <span class="tree-folder-label min-w-0 truncate">{node.label.clone()}</span>
                    </summary>
                    <ul class="tree m-0 grid list-none gap-0.5 p-0 pl-2">{children}</ul>
                </details>
            </li>
        }
        .into_any();
    }

    let parsed = BuildNodeId::parse(&node.node_id);
    let is_active = parsed.as_ref() == Some(active_node);
    let href = parsed
        .as_ref()
        .map(|id| build_node_href(app_path, id, active_tab, Default::default()))
        .unwrap_or_else(|| "#".to_string());
    let class = if is_active {
        "tree-link tree-file-row tree-file-row--active font-medium text-sky-100 transition-colors"
    } else {
        "tree-link tree-file-row mei-text-body transition-colors hover:mei-text-inverse"
    };

    if node.children.is_empty() {
        view! {
            <li class="tree-node">
                <div class="tree-file-entry">
                    <a class=class href=href title=node.label.clone() data-build-node=node.node_id.clone()>
                        <span class="tree-file-label min-w-0 flex-1 truncate">{node.label.clone()}</span>
                        {node
                            .badges
                            .first()
                            .map(|badge| {
                                view! {
                                    <span class="ml-1 text-[10px] mei-text-muted">{badge.clone()}</span>
                                }
                                    .into_any()
                            })
                            .unwrap_or_else(|| view! { <></> }.into_any())}
                    </a>
                </div>
            </li>
        }
        .into_any()
    } else {
        let children = node
            .children
            .iter()
            .map(|child| tree_node(child, app_path, active_node, active_tab, depth + 1))
            .collect_view();
        view! {
            <li class="tree-node tree-li-branch">
                <details class="pl-1" open=is_active>
                    <summary class=if is_active {
                        "tree-folder-summary tree-file-row tree-file-row--active cursor-pointer select-none font-medium text-sky-100"
                    } else {
                        "tree-folder-summary tree-file-row cursor-pointer select-none mei-text-body"
                    }>
                        <a class="min-w-0 flex-1 truncate" href=href data-build-node=node.node_id.clone()>
                            {node.label.clone()}
                        </a>
                    </summary>
                    <ul class="tree m-0 grid list-none gap-0.5 p-0 pl-2">{children}</ul>
                </details>
            </li>
        }
        .into_any()
    }
}
