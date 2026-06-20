use serde::{Deserialize, Serialize};

use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp};

/// One node in the build-view reachability tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReachabilityTreeNode {
    pub id: String,
    pub node_id: String,
    pub kind: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub badges: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ReachabilityTreeNode>,
}

/// Top-level grouping root in the build-view sidebar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReachabilityTreeRoot {
    pub group: String,
    pub label: String,
    #[serde(default)]
    pub default_open: bool,
    #[serde(default)]
    pub children: Vec<ReachabilityTreeNode>,
}

pub fn build_reachability_tree(compiled: &CompiledApp) -> Vec<ReachabilityTreeRoot> {
    vec![
        routes_root(compiled),
        scenes_root(compiled),
        world_root(compiled),
        datasets_root(compiled),
        components_root(compiled),
        artifacts_root(compiled),
    ]
}

fn routes_root(compiled: &CompiledApp) -> ReachabilityTreeRoot {
    ReachabilityTreeRoot {
        group: "routes".to_string(),
        label: "Routes".to_string(),
        default_open: true,
        children: compiled
            .scene_routes
            .iter()
            .map(|route| {
                let node = BuildNodeId::route(route.scene_id.clone());
                let mut badges = Vec::new();
                if route.access_export {
                    badges.push("access".to_string());
                }
                if route.is_default {
                    badges.push("default".to_string());
                }
                ReachabilityTreeNode {
                    id: format!("route-{}", route.scene_id),
                    node_id: node.encode(),
                    kind: "route".to_string(),
                    label: route
                        .title
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| route.scene_id.clone()),
                    badges,
                    children: Vec::new(),
                }
            })
            .collect(),
    }
}

fn scenes_root(compiled: &CompiledApp) -> ReachabilityTreeRoot {
    ReachabilityTreeRoot {
        group: "scenes".to_string(),
        label: "Scenes".to_string(),
        default_open: false,
        children: compiled
            .scene_routes
            .iter()
            .map(|route| {
                let scene_node = BuildNodeId::scene(route.scene_id.clone());
                let mut children = Vec::new();
                if let Some(assembly) = compiled
                    .scene_projection_assembly_by_id
                    .get(&route.scene_id)
                {
                    if let Some(overlays) = assembly.get("overlays").and_then(|v| v.as_object()) {
                        for (projection_id, _) in overlays {
                            let node = BuildNodeId::projection(route.scene_id.clone(), projection_id);
                            children.push(ReachabilityTreeNode {
                                id: format!("projection-{}-{}", route.scene_id, projection_id),
                                node_id: node.encode(),
                                kind: "projection".to_string(),
                                label: projection_id.clone(),
                                badges: vec!["link-only".to_string()],
                                children: Vec::new(),
                            });
                        }
                    }
                    if let Some(boards) = assembly.get("boards").and_then(|v| v.as_object()) {
                        for (projection_id, _) in boards {
                            let node = BuildNodeId::projection(route.scene_id.clone(), projection_id);
                            children.push(ReachabilityTreeNode {
                                id: format!("board-{}-{}", route.scene_id, projection_id),
                                node_id: node.encode(),
                                kind: "projection".to_string(),
                                label: projection_id.clone(),
                                badges: vec!["board".to_string()],
                                children: Vec::new(),
                            });
                        }
                    }
                }
                ReachabilityTreeNode {
                    id: format!("scene-{}", route.scene_id),
                    node_id: scene_node.encode(),
                    kind: "scene".to_string(),
                    label: route
                        .title
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| route.scene_id.clone()),
                    badges: vec![route.target_file.clone()],
                    children,
                }
            })
            .collect(),
    }
}

fn world_root(compiled: &CompiledApp) -> ReachabilityTreeRoot {
    let mut children = Vec::new();
    for (file_path, index) in &compiled.world_semantic_by_file {
        let file_node = BuildNodeId::new(BuildNodeKind::WorldFile, file_path.clone());
        let mut file_children = Vec::new();
        if !index.datasets.is_empty() {
            let dataset_nodes = index
                .datasets
                .iter()
                .map(|dataset| {
                    let node = BuildNodeId::world_dataset(file_path, dataset.id.clone());
                    ReachabilityTreeNode {
                        id: format!("world-dataset-{}-{}", file_path, dataset.id),
                        node_id: node.encode(),
                        kind: "world_dataset".to_string(),
                        label: dataset
                            .title
                            .clone()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or_else(|| dataset.id.clone()),
                        badges: Vec::new(),
                        children: Vec::new(),
                    }
                })
                .collect::<Vec<_>>();
            file_children.push(ReachabilityTreeNode {
                id: format!("world-group-datasets-{}", file_path),
                node_id: String::new(),
                kind: "world_group".to_string(),
                label: "数据集".to_string(),
                badges: Vec::new(),
                children: dataset_nodes,
            });
        }
        if !index.metrics.is_empty() {
            let metric_nodes = index
                .metrics
                .iter()
                .map(|metric| {
                    let node = BuildNodeId::world_metric(file_path, metric.id.clone());
                    let explain_children = metric
                        .explain
                        .iter()
                        .map(|explain| {
                            let explain_node = BuildNodeId::world_explain(
                                file_path,
                                metric.id.clone(),
                                explain.id.clone(),
                            );
                            ReachabilityTreeNode {
                                id: format!(
                                    "world-explain-{}-{}-{}",
                                    file_path, metric.id, explain.id
                                ),
                                node_id: explain_node.encode(),
                                kind: "explain_block".to_string(),
                                label: explain
                                    .label
                                    .clone()
                                    .filter(|value| !value.trim().is_empty())
                                    .unwrap_or_else(|| explain.id.clone()),
                                badges: Vec::new(),
                                children: Vec::new(),
                            }
                        })
                        .collect();
                    ReachabilityTreeNode {
                        id: format!("world-metric-{}-{}", file_path, metric.id),
                        node_id: node.encode(),
                        kind: "world_metric".to_string(),
                        label: metric
                            .label
                            .clone()
                            .filter(|value| !value.trim().is_empty())
                            .unwrap_or_else(|| metric.id.clone()),
                        badges: Vec::new(),
                        children: explain_children,
                    }
                })
                .collect::<Vec<_>>();
            file_children.push(ReachabilityTreeNode {
                id: format!("world-group-metrics-{}", file_path),
                node_id: String::new(),
                kind: "world_group".to_string(),
                label: "指标".to_string(),
                badges: Vec::new(),
                children: metric_nodes,
            });
        }
        children.push(ReachabilityTreeNode {
            id: format!("world-file-{}", file_path),
            node_id: file_node.encode(),
            kind: "world_file".to_string(),
            label: file_path.clone(),
            badges: Vec::new(),
            children: file_children,
        });
    }
    ReachabilityTreeRoot {
        group: "world".to_string(),
        label: "World".to_string(),
        default_open: true,
        children,
    }
}

fn datasets_root(compiled: &CompiledApp) -> ReachabilityTreeRoot {
    ReachabilityTreeRoot {
        group: "datasets".to_string(),
        label: "Datasets".to_string(),
        default_open: false,
        children: compiled
            .resources
            .iter()
            .filter(|resource| resource.dataset.is_some())
            .map(|resource| {
                let node = BuildNodeId::dataset(resource.id.clone());
                ReachabilityTreeNode {
                    id: format!("dataset-{}", resource.id),
                    node_id: node.encode(),
                    kind: "dataset".to_string(),
                    label: resource
                        .title
                        .clone()
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| resource.id.clone()),
                    badges: resource
                        .document
                        .clone()
                        .map(|path| vec![path])
                        .unwrap_or_default(),
                    children: Vec::new(),
                }
            })
            .collect(),
    }
}

fn components_root(compiled: &CompiledApp) -> ReachabilityTreeRoot {
    ReachabilityTreeRoot {
        group: "components".to_string(),
        label: "Components".to_string(),
        default_open: false,
        children: compiled
            .component_assets
            .iter()
            .map(|asset| {
                let node = BuildNodeId::component(asset.key.clone());
                ReachabilityTreeNode {
                    id: format!("component-{}", asset.key),
                    node_id: node.encode(),
                    kind: "component".to_string(),
                    label: asset.key.clone(),
                    badges: vec![asset.tag.clone()],
                    children: Vec::new(),
                }
            })
            .collect(),
    }
}

fn artifacts_root(compiled: &CompiledApp) -> ReachabilityTreeRoot {
    ReachabilityTreeRoot {
        group: "artifacts".to_string(),
        label: "Artifacts".to_string(),
        default_open: false,
        children: compiled
            .scene_routes
            .iter()
            .map(|route| {
                let node = BuildNodeId::artifact("compiled_app", route.scene_id.clone());
                ReachabilityTreeNode {
                    id: format!("artifact-compiled-{}", route.scene_id),
                    node_id: node.encode(),
                    kind: "artifact".to_string(),
                    label: format!("compiled_app / {}", route.scene_id),
                    badges: vec!["prebuild".to_string()],
                    children: Vec::new(),
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn reachability_tree_includes_routes_and_world() {
        let mut compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: ".".to_string(),
            scene_routes: vec![crate::model::CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "scenes/home.mei".to_string(),
                kind: "file_ref".to_string(),
                title: Some("Home".to_string()),
                is_default: true,
                access_export: true,
            }],
            active_scene: Some("home".to_string()),
            active_target_file: "scenes/home.mei".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: BTreeMap::new(),
            scene_bindings_by_id: BTreeMap::new(),
            scene_examples_by_id: BTreeMap::new(),
            scene_projection_assembly_by_id: BTreeMap::new(),
            resources: Vec::new(),
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
        };
        compiled.world_semantic_by_file.insert(
            "metrics.world.mei".to_string(),
            crate::model::WorldSemanticFileIndex {
                world_id: Some("metrics".to_string()),
                datasets: vec![],
                metrics: vec![crate::model::WorldSemanticMetric {
                    id: "total".to_string(),
                    label: Some("Total".to_string()),
                    unit: None,
                    note: None,
                    explain: vec![],
                }],
                resource_id: "__world_metrics__".to_string(),
            },
        );
        let roots = build_reachability_tree(&compiled);
        assert_eq!(roots.len(), 6);
        assert_eq!(roots[0].children.len(), 1);
        assert_eq!(roots[2].children.len(), 1);
    }
}
