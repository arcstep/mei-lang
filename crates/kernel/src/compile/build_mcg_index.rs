use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::compile::reachability_tree::{ReachabilityTreeNode, ReachabilityTreeRoot};
use crate::mei_config::resolve_workspace_graph_root;
use crate::model::{BuildNodeId, BuildNodeKind};

#[derive(Debug, Deserialize)]
struct McgRegistryFile {
    #[serde(default)]
    nodes: Vec<McgNodeFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct McgNodeFile {
    id: McgNodeIdFile,
    revision: String,
    state: String,
    #[serde(default, rename = "ownerResourceId")]
    owner_resource_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct McgNodeIdFile {
    kind: McgKindSlug,
    key: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum McgKindSlug {
    ScenePayload,
    MetricDefBundle,
    AssemblyView,
}

impl McgKindSlug {
    fn slug(self) -> &'static str {
        match self {
            Self::ScenePayload => "scene_payload",
            Self::MetricDefBundle => "metric_def_bundle",
            Self::AssemblyView => "assembly_view",
        }
    }
}

pub fn build_mcg_tree_root(source_root: &Path, app_id: &str) -> ReachabilityTreeRoot {
    let path = resolve_workspace_graph_root(source_root, app_id).join("mcg-registry.json");
    let nodes = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<McgRegistryFile>(&raw).ok())
        .map(|registry| registry.nodes)
        .unwrap_or_default();
    let mut groups: [Vec<McgNodeFile>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for node in nodes {
        match node.id.kind.slug() {
            "scene_payload" => groups[0].push(node),
            "metric_def_bundle" => groups[1].push(node),
            "assembly_view" => groups[2].push(node),
            _ => {}
        }
    }
    let children = [
        ("scene_payload", "ScenePayload", &groups[0]),
        ("metric_def_bundle", "MetricDefBundle", &groups[1]),
        ("assembly_view", "AssemblyView", &groups[2]),
    ]
    .into_iter()
    .filter_map(|(kind_slug, kind_label, entries)| {
        if entries.is_empty() {
            return None;
        }
        let mut sorted = entries.to_vec();
        sorted.sort_by(|left, right| left.id.key.cmp(&right.id.key));
        Some(ReachabilityTreeNode {
            id: format!("mcg-group-{kind_slug}"),
            node_id: String::new(),
            kind: "mcg_group".to_string(),
            label: kind_label.to_string(),
            badges: Vec::new(),
            compile_scene: String::new(),
            compile_target: String::new(),
            board_layout_zone: String::new(),
            children: sorted
                .into_iter()
                .map(|node| mcg_leaf_node(&node))
                .collect(),
        })
    })
    .collect();
    ReachabilityTreeRoot {
        group: "mcg".to_string(),
        label: "Compile · MCG".to_string(),
        default_open: false,
        children,
    }
}

fn mcg_leaf_node(node: &McgNodeFile) -> ReachabilityTreeNode {
    let node_id = BuildNodeId::new(
        BuildNodeKind::McgNode,
        format!("{}:{}", node.id.kind.slug(), node.id.key),
    );
    let mut label = node.id.key.clone();
    if let Some(owner) = node
        .owner_resource_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        label = format!("{label} · {owner}");
    }
    let badges = vec![node.state.clone(), format!("rev:{}", node.revision)];
    ReachabilityTreeNode {
        id: format!("mcg-{}-{}", node.id.kind.slug(), node.id.key),
        node_id: node_id.encode(),
        kind: "mcg_node".to_string(),
        label,
        badges,
        compile_scene: String::new(),
        compile_target: String::new(),
        board_layout_zone: String::new(),
        children: Vec::new(),
    }
}
