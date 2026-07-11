use std::collections::HashSet;

use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp};

use super::types::ReachabilityTreeNode;
use super::ReachabilityTreeRoot;

pub fn build_reachability_tree(compiled: &CompiledApp) -> Vec<ReachabilityTreeRoot> {
    crate::compile::build_experience_index::reachability_roots_from_compiled(compiled)
}

/// When browsing `_stock-catalog`, keep only the stock facet root matching `catalog=`,
/// optionally narrowed to a single component pack or template folder (`pack=`).
/// Business apps never mount stock component/template trees (platform topbar entries only).
pub fn is_stock_catalog_facet_root(group: &str) -> bool {
    is_stock_facet_root_group(group)
}

pub fn filter_reachability_roots_for_stock_catalog(
    roots: Vec<ReachabilityTreeRoot>,
    is_catalog_app: bool,
    catalog: Option<&str>,
    pack: Option<&str>,
) -> Vec<ReachabilityTreeRoot> {
    if !is_catalog_app {
        return roots
            .into_iter()
            .filter(|root| !is_stock_facet_root_group(root.group.as_str()))
            .collect();
    }
    let facet = catalog
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("components");
    let pack = pack.map(str::trim).filter(|value| !value.is_empty());
    let path_prefix = stock_catalog_path_prefix(facet, pack);
    let mut filtered: Vec<ReachabilityTreeRoot> = roots
        .into_iter()
        .filter(|root| match facet {
            "templates" => root.group != "templates",
            _ => root.group != "template_files",
        })
        .map(|mut root| {
            if is_stock_facet_root_group(root.group.as_str()) {
                if let Some(pack) = pack {
                    narrow_stock_facet_root(&mut root, pack);
                }
            }
            root
        })
        .collect();
    filter_catalog_scene_roots(&mut filtered, &path_prefix);
    filtered.retain(|root| !should_hide_catalog_root(root, pack.is_some()));
    filtered
}

fn stock_catalog_path_prefix(facet: &str, pack: Option<&str>) -> String {
    let base = match facet {
        "templates" => "stock/templates/",
        _ => "stock/components/",
    };
    match pack {
        Some(pack) => format!("{base}{pack}/"),
        None => base.to_string(),
    }
}

fn normalize_stock_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn node_matches_stock_prefix(node: &ReachabilityTreeNode, prefix: &str) -> bool {
    if node
        .badges
        .iter()
        .any(|badge| normalize_stock_path(badge).contains(prefix))
    {
        return true;
    }
    if !node.compile_target.is_empty()
        && normalize_stock_path(&node.compile_target).contains(prefix)
    {
        return true;
    }
    false
}

fn scene_ids_from_nodes(nodes: &[ReachabilityTreeNode]) -> HashSet<String> {
    nodes
        .iter()
        .filter_map(|node| {
            BuildNodeId::parse(&node.node_id)
                .and_then(|id| (id.kind == BuildNodeKind::Scene).then_some(id.key))
        })
        .collect()
}

fn filter_catalog_scene_roots(roots: &mut [ReachabilityTreeRoot], path_prefix: &str) {
    let mut allowed_scene_ids = HashSet::new();
    for root in roots.iter_mut() {
        if root.group == "scenes" {
            root.children
                .retain(|node| node_matches_stock_prefix(node, path_prefix));
            allowed_scene_ids.extend(scene_ids_from_nodes(&root.children));
            continue;
        }
        if root.group == "routes" {
            root.children.retain(|node| {
                BuildNodeId::parse(&node.node_id).is_some_and(|id| {
                    id.kind == BuildNodeKind::Route && allowed_scene_ids.contains(&id.key)
                })
            });
            continue;
        }
        if root.group == "artifacts" {
            root.children.retain(|node| {
                BuildNodeId::parse(&node.node_id).is_some_and(|id| {
                    id.kind == BuildNodeKind::Artifact
                        && id
                            .key
                            .split('/')
                            .nth(1)
                            .is_some_and(|scene_id| allowed_scene_ids.contains(scene_id))
                })
            });
        }
    }
}

fn should_hide_catalog_root(root: &ReachabilityTreeRoot, pack_selected: bool) -> bool {
    if root.children.is_empty() {
        return matches!(
            root.group.as_str(),
            "scenes" | "routes" | "artifacts" | "world" | "datasets" | "boards"
        );
    }
    if pack_selected && is_stock_facet_root_group(root.group.as_str()) && root.children.is_empty() {
        return true;
    }
    false
}

fn narrow_stock_facet_root(root: &mut ReachabilityTreeRoot, pack: &str) {
    if let Some(pack_node) = root
        .children
        .iter()
        .find(|node| node.label == pack)
        .cloned()
    {
        root.label = pack_node.label.clone();
        root.children = pack_node.children;
        return;
    }
    root.children.retain(|node| node.label == pack);
}

fn is_stock_facet_root_group(group: &str) -> bool {
    group == "templates" || group == "template_files"
}
