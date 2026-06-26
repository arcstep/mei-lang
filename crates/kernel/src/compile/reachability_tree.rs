use std::collections::HashSet;

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
    /// Compile scene anchor (`home`, board export id, …) for fast client navigation.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compile_scene: String,
    /// Compile preview target file (`scenes/home.mei`, `*.board.mei`, …).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compile_target: String,
    /// Board slot layout zone (`filter`, `chart`, `detail`, …) for build preview inspect.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub board_layout_zone: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ReachabilityTreeNode>,
}

impl Default for ReachabilityTreeNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            node_id: String::new(),
            kind: String::new(),
            label: String::new(),
            badges: Vec::new(),
            compile_scene: String::new(),
            compile_target: String::new(),
            board_layout_zone: String::new(),
            children: Vec::new(),
        }
    }
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
            BuildNodeId::parse(&node.node_id).and_then(|id| {
                (id.kind == BuildNodeKind::Scene).then_some(id.key)
            })
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
                BuildNodeId::parse(&node.node_id)
                    .is_some_and(|id| id.kind == BuildNodeKind::Route && allowed_scene_ids.contains(&id.key))
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
    if pack_selected && is_stock_facet_root_group(root.group.as_str()) && root.children.is_empty()
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BlockDecl, PanelDecl, UiNodeDecl};
    use serde_json::Value;
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
            build_experience_index: Default::default(),
            build_board_index: Default::default(),
            build_template_index: Default::default(),
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
        assert_eq!(roots.len(), 5);
        assert!(!roots[0].default_open);
        assert_eq!(roots[0].group, "scenes");
        assert_eq!(roots[1].group, "routes");
        assert_eq!(roots[1].children.len(), 1);
        assert!(!roots[2].default_open);
        assert_eq!(roots[2].label, "Backing · World");
        assert_eq!(roots[2].children.len(), 1);
    }

    #[test]
    fn reachability_tree_expands_scene_panels_from_assembly() {
        let panel = PanelDecl {
            kind: "panel".to_string(),
            id: "kpi_row".to_string(),
            title: Some("KPI 行".to_string()),
            head: None,
            area: None,
            layout: None,
            blocks: vec![UiNodeDecl::Block(BlockDecl {
                kind: "component".to_string(),
                use_key: "cockpit.metric-card".to_string(),
                id: Some("pending_card".to_string()),
                title: Some("待办数".to_string()),
                area: None,
                props: serde_json::json!({
                    "metric": { "__ref": "metric", "id": "total", "from_dataset": "agency_objects" }
                }),
                base: None,
                layout: None,
                blocks: Vec::new(),
                component: None,
                placement: None,
                interactions: Vec::new(),
                lifecycle: None,
                constraints: None,
                data: None,
            })],
            slot: None,
            props: Value::Null,
            head_props: Value::Null,
            body_props: Value::Null,
            base: None,
            import_scope: None,
        };
        let mut assembly = BTreeMap::<String, Value>::new();
        assembly.insert(
            "home".to_string(),
            serde_json::json!({
                "scene_id": "home",
                "panels": [panel],
            }),
        );
        let compiled = CompiledApp {
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
            scene_projection_assembly_by_id: assembly,
            resources: Vec::new(),
            world_metrics: BTreeMap::new(),
            world_semantic_by_file: BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: Vec::new(),
            build_experience_index: Default::default(),
            build_board_index: Default::default(),
            build_template_index: Default::default(),
        };
        let roots = build_reachability_tree(&compiled);
        let scene = &roots[0].children[0];
        let panels_group = scene
            .children
            .iter()
            .find(|node| node.label == "Panels")
            .expect("panels group");
        assert_eq!(panels_group.children.len(), 1);
        assert_eq!(panels_group.children[0].label, "KPI 行");
        assert_eq!(panels_group.children[0].children.len(), 1);
        assert_eq!(panels_group.children[0].children[0].label, "待办数");
        assert!(panels_group.children[0].children[0]
            .badges
            .iter()
            .any(|badge| badge.contains("agency_objects")));
    }

    fn sample_catalog_scene_node(scene_id: &str, target_file: &str) -> ReachabilityTreeNode {
        ReachabilityTreeNode {
            id: format!("scene-{scene_id}"),
            node_id: BuildNodeId::scene(scene_id).encode(),
            kind: "scene".to_string(),
            label: scene_id.to_string(),
            badges: vec![target_file.to_string()],
            children: Vec::new(),
            ..Default::default()
        }
    }

    #[test]
    fn stock_catalog_filter_narrows_scenes_and_flattens_facet_by_pack() {
        let roots = vec![
            ReachabilityTreeRoot {
                group: "scenes".to_string(),
                label: "Scenes".to_string(),
                default_open: false,
                children: vec![
                    sample_catalog_scene_node(
                        "chart.pie",
                        "../../stock/components/chart/echarts/previews/chart.pie.mei",
                    ),
                    sample_catalog_scene_node(
                        "chart.line",
                        "../../stock/components/chart/line/previews/chart.line.mei",
                    ),
                ],
            },
            ReachabilityTreeRoot {
                group: "templates".to_string(),
                label: "Components".to_string(),
                default_open: false,
                children: vec![ReachabilityTreeNode {
                    id: "pack-chart-echarts".to_string(),
                    node_id: String::new(),
                    kind: "component_pack".to_string(),
                    label: "chart/echarts".to_string(),
                    badges: Vec::new(),
                    children: vec![ReachabilityTreeNode {
                        id: "tpl-pie".to_string(),
                        node_id: BuildNodeId::new(BuildNodeKind::Template, "chart.pie.mei").encode(),
                        kind: "template".to_string(),
                        label: "chart.pie".to_string(),
                        badges: Vec::new(),
                        children: Vec::new(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
            },
            ReachabilityTreeRoot {
                group: "template_files".to_string(),
                label: "Templates".to_string(),
                default_open: false,
                children: vec![ReachabilityTreeNode {
                    id: "tpl-pack".to_string(),
                    node_id: String::new(),
                    kind: "template_group".to_string(),
                    label: "layout/basic".to_string(),
                    badges: Vec::new(),
                    children: Vec::new(),
                    ..Default::default()
                }],
            },
        ];
        let filtered = filter_reachability_roots_for_stock_catalog(
            roots,
            true,
            Some("components"),
            Some("chart/echarts"),
        );
        assert!(
            filtered
                .iter()
                .all(|root| root.group != "template_files"),
            "components facet should drop template_files root"
        );
        let scenes = filtered
            .iter()
            .find(|root| root.group == "scenes")
            .expect("scenes root");
        assert_eq!(scenes.children.len(), 1);
        assert_eq!(scenes.children[0].label, "chart.pie");
        let components = filtered
            .iter()
            .find(|root| root.group == "templates")
            .expect("components root");
        assert_eq!(components.label, "chart/echarts");
        assert_eq!(components.children.len(), 1);
        assert_eq!(components.children[0].label, "chart.pie");
    }
}
