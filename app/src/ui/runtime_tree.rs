use leptos::prelude::*;
use mei_lang_kernel::{ReachabilityTreeNode, ReachabilityTreeRoot};

use super::manage_routing::runtime_node_href;

pub(crate) fn runtime_observability_tree_view(
    roots: &[ReachabilityTreeRoot],
    app_path: &str,
    active_node_id: Option<&str>,
) -> AnyView {
    let items = roots
        .iter()
        .map(|root| root_branch(root, app_path, active_node_id))
        .collect_view();
    view! {
        <div class="build-reachability-tree runtime-observability-tree" data-mei-runtime-tree="1">
            <ul class="build-tree-list">{items}</ul>
        </div>
    }
    .into_any()
}

fn root_branch(
    root: &ReachabilityTreeRoot,
    app_path: &str,
    active_node_id: Option<&str>,
) -> AnyView {
    let child_count = root.children.len();
    let children = root
        .children
        .iter()
        .map(|node| tree_node(node, app_path, active_node_id))
        .collect_view();
    view! {
        <li class="build-tree-node build-tree-node--branch">
            <details
                class="build-tree-details"
                open=root.default_open
                data-build-tree-branch=format!("root:{}", root.group.clone())
                data-build-tree-children-count=child_count.to_string()
            >
                <summary class="build-tree-summary build-tree-summary--root">
                    <span class="build-tree-kind build-tree-kind--root" aria-hidden="true">"▦"</span>
                    <span class="build-tree-label">{root.label.clone()}</span>
                </summary>
                <ul class="build-tree-list build-tree-list--nested">{children}</ul>
            </details>
        </li>
    }
    .into_any()
}

fn tree_node(node: &ReachabilityTreeNode, app_path: &str, active_node_id: Option<&str>) -> AnyView {
    let is_active = active_node_id == Some(node.node_id.as_str());
    let href = runtime_node_href(app_path, node.node_id.as_str(), Some("overview"));
    let class = if is_active {
        "build-tree-link build-tree-link--active"
    } else {
        "build-tree-link"
    };
    let badges = node.badges.clone();
    let short_label = shorten_tree_label(&node.label);
    let title = if short_label == node.label {
        None
    } else {
        Some(node.label.clone())
    };
    view! {
        <li class="build-tree-node">
            <a class=class href=href data-runtime-node=node.node_id.clone() title=title.unwrap_or_default()>
                <span class="build-tree-spacer" aria-hidden="true"></span>
                <span class="build-tree-kind" aria-hidden="true">{runtime_kind_glyph(&node.node_id, &node.kind)}</span>
                <span class="build-tree-label">
                    {short_label}
                    {badges.into_iter().take(2).map(|value| view! {
                        <span class="build-tree-badge build-tree-badge--meta">{value}</span>
                    }).collect_view()}
                </span>
            </a>
        </li>
    }
    .into_any()
}

fn shorten_tree_label(label: &str) -> String {
    const MAX: usize = 52;
    let trimmed = label.trim();
    if trimmed.chars().count() <= MAX {
        return trimmed.to_string();
    }
    format!("{}…", trimmed.chars().take(MAX).collect::<String>())
}

fn runtime_kind_glyph(node_id: &str, kind: &str) -> &'static str {
    if node_id.starts_with("overview-") {
        return "◆";
    }
    if node_id.starts_with("l1-") {
        return "◫";
    }
    if node_id.starts_with("l2-") {
        return "⌁";
    }
    if node_id.starts_with("l3-") {
        return "⬡";
    }
    if node_id.starts_with("l4-") || node_id.starts_with("mrg-slot:") {
        return "▣";
    }
    if node_id.starts_with("build-") {
        return "⚙";
    }
    if node_id.starts_with("log") {
        return "!";
    }
    match kind {
        "mrg_slot" => "▣",
        _ => "·",
    }
}
