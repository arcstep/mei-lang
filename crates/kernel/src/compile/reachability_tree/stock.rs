

use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp};

use super::{ReachabilityTreeNode, ReachabilityTreeRoot};

pub(crate) fn routes_root(compiled: &CompiledApp) -> ReachabilityTreeRoot {
    ReachabilityTreeRoot {
        group: "routes".to_string(),
        label: "Routes".to_string(),
        default_open: false,
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
                    ..Default::default()
                }
            })
            .collect(),
    }
}

pub(crate) fn world_root(compiled: &CompiledApp) -> ReachabilityTreeRoot {
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
                        ..Default::default()
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
                ..Default::default()
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
                                ..Default::default()
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
                        ..Default::default()
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
                ..Default::default()
            });
        }
        children.push(ReachabilityTreeNode {
            id: format!("world-file-{}", file_path),
            node_id: file_node.encode(),
            kind: "world_file".to_string(),
            label: file_path.clone(),
            badges: Vec::new(),
            children: file_children,
            ..Default::default()
        });
    }
    ReachabilityTreeRoot {
        group: "world".to_string(),
        label: "Backing · World".to_string(),
        default_open: false,
        children,
    }
}

pub(crate) fn datasets_root(compiled: &CompiledApp) -> ReachabilityTreeRoot {
    ReachabilityTreeRoot {
        group: "datasets".to_string(),
        label: "Backing · Datasets".to_string(),
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
                    ..Default::default()
                }
            })
            .collect(),
    }
}

pub(crate) fn artifacts_root(compiled: &CompiledApp) -> ReachabilityTreeRoot {
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
                    ..Default::default()
                }
            })
            .collect(),
    }
}

