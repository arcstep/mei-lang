use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::compile::reachability_tree::{ReachabilityTreeNode, ReachabilityTreeRoot};
use crate::mei_config::{resolve_app_registry_root, resolve_app_root};
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
    #[serde(default)]
    deps: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct McgNodeIdFile {
    kind: McgNodeKindField,
    key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum McgNodeKindField {
    Slug(String),
    Object { kind: String },
}

impl McgNodeKindField {
    fn slug(&self) -> String {
        match self {
            Self::Slug(value) => value.clone(),
            Self::Object { kind } => kind.clone(),
        }
    }
}

const MCG_KIND_ORDER: &[&str] = &[
    "semantic_graph",
    "content_panel",
    "navigation",
    "metric_def_bundle",
    "app_skeleton",
    "warmup_policy",
    "world_model",
    "page_instance",
    "scene_payload",
    "catalog_resource",
    "data_source",
    "eval_plan",
    "workset",
    "material_slot",
];

fn mcg_kind_label(kind: &str) -> String {
    match kind {
        "semantic_graph" => "SemanticGraph",
        "content_panel" => "ContentPanel",
        "navigation" => "Navigation",
        "metric_def_bundle" => "MetricDefBundle",
        "app_skeleton" => "AppSkeleton",
        "warmup_policy" => "WarmupPolicy",
        "world_model" => "WorldModel",
        "page_instance" => "PageInstance (legacy)",
        "scene_payload" => "ScenePayload (legacy)",
        other => other,
    }
    .to_string()
}

pub fn build_mcg_tree_root(source_root: &Path, app_id: &str) -> ReachabilityTreeRoot {
    let path = resolve_app_registry_root(&resolve_app_root(source_root, app_id))
        .join("mcg-registry.json");
    let nodes = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<McgRegistryFile>(&raw).ok())
        .map(|registry| registry.nodes)
        .unwrap_or_default();

    let mut grouped: BTreeMap<String, Vec<McgNodeFile>> = BTreeMap::new();
    for node in nodes {
        grouped
            .entry(node.id.kind.slug())
            .or_default()
            .push(node);
    }

    let mut kind_order: Vec<String> = MCG_KIND_ORDER
        .iter()
        .map(|kind| kind.to_string())
        .collect();
    for kind in grouped.keys() {
        if !kind_order.iter().any(|entry| entry == kind) {
            kind_order.push(kind.clone());
        }
    }

    let children = kind_order
        .into_iter()
        .filter_map(|kind_slug| {
            let entries = grouped.get(&kind_slug)?;
            if entries.is_empty() {
                return None;
            }
            let mut sorted = entries.clone();
            sorted.sort_by(|left, right| left.id.key.cmp(&right.id.key));
            let kind_label = mcg_kind_label(kind_slug.as_str());
            Some(ReachabilityTreeNode {
                id: format!("mcg-group-{kind_slug}"),
                node_id: String::new(),
                kind: "mcg_group".to_string(),
                label: kind_label,
                badges: vec![format!("count:{}", sorted.len())],
                compile_scene: String::new(),
                compile_target: String::new(),
                board_layout_zone: String::new(),
                children: sorted
                    .into_iter()
                    .map(|node| mcg_leaf_node(&kind_slug, &node))
                    .collect(),
                ..Default::default()
            })
        })
        .collect();

    ReachabilityTreeRoot {
        group: "mcg".to_string(),
        label: "MCG".to_string(),
        default_open: true,
        children,
    }
}

fn mcg_leaf_node(kind_slug: &str, node: &McgNodeFile) -> ReachabilityTreeNode {
    let stable_key = format!("{kind_slug}:{}", node.id.key);
    let node_id = BuildNodeId::new(BuildNodeKind::McgNode, stable_key.clone());
    let mut badges = vec![
        format!("state:{}", node.state),
        format!("rev:{}", node.revision),
    ];
    if !node.deps.is_empty() {
        badges.push(format!("deps:{}", node.deps.len()));
    }
    if let Some(owner) = node
        .owner_resource_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        badges.push(format!("owner:{owner}"));
    }
    ReachabilityTreeNode {
        id: format!("mcg-{kind_slug}-{}", node.id.key),
        node_id: node_id.encode(),
        kind: "mcg_node".to_string(),
        label: node.id.key.clone(),
        badges,
        compile_scene: String::new(),
        compile_target: String::new(),
        board_layout_zone: String::new(),
        children: Vec::new(),
        ..Default::default()
    }
}
