use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp, ProvenanceAnchor};

/// Script path used as `CompileOptions.preview_target` before compile, when the build URL
/// selects a node via `?node=` without legacy `?file=`.
pub fn preview_target_from_build_node(node: &BuildNodeId) -> Option<String> {
    super::build_experience::preview_target_from_build_node_with_app(node, None)
}

/// Resolved preview / routing context for a build-view node selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildNodeContext {
    pub node: BuildNodeId,
    pub target_file: String,
    pub scene_id: Option<String>,
    pub world_metric: Option<String>,
    pub world_dataset: Option<String>,
    pub explain: Option<String>,
    pub projection_id: Option<String>,
    pub provenance: ProvenanceAnchor,
}

pub fn resolve_build_node_context(compiled: &CompiledApp, node: &BuildNodeId) -> BuildNodeContext {
    let provenance = provenance_for_node(compiled, node);
    match node.kind {
        BuildNodeKind::Route | BuildNodeKind::Scene => {
            let scene_id = if node.kind == BuildNodeKind::Route {
                node.key.clone()
            } else {
                node.key.clone()
            };
            let target_file = compiled
                .scene_routes
                .iter()
                .find(|route| route.scene_id == scene_id)
                .map(|route| route.target_file.clone())
                .unwrap_or_else(|| compiled.active_target_file.clone());
            BuildNodeContext {
                node: node.clone(),
                target_file,
                scene_id: Some(scene_id),
                world_metric: None,
                world_dataset: None,
                explain: None,
                projection_id: None,
                provenance,
            }
        }
        BuildNodeKind::Projection => {
            let (scene_id, projection_id) = split_projection_key(&node.key);
            let target_file = compiled
                .scene_routes
                .iter()
                .find(|route| route.scene_id == scene_id)
                .map(|route| route.target_file.clone())
                .unwrap_or_else(|| compiled.active_target_file.clone());
            BuildNodeContext {
                node: node.clone(),
                target_file,
                scene_id: Some(scene_id),
                world_metric: None,
                world_dataset: None,
                explain: None,
                projection_id: Some(projection_id),
                provenance,
            }
        }
        BuildNodeKind::WorldFile => BuildNodeContext {
            node: node.clone(),
            target_file: node.key.clone(),
            scene_id: None,
            world_metric: None,
            world_dataset: None,
            explain: None,
            projection_id: None,
            provenance,
        },
        BuildNodeKind::WorldDataset => {
            let (file, dataset_id) = split_file_symbol(&node.key);
            BuildNodeContext {
                node: node.clone(),
                target_file: file,
                scene_id: None,
                world_metric: None,
                world_dataset: Some(dataset_id),
                explain: None,
                projection_id: None,
                provenance,
            }
        }
        BuildNodeKind::WorldMetric => {
            let (file, metric_id) = split_file_symbol(&node.key);
            BuildNodeContext {
                node: node.clone(),
                target_file: file,
                scene_id: None,
                world_metric: Some(metric_id),
                world_dataset: None,
                explain: None,
                projection_id: None,
                provenance,
            }
        }
        BuildNodeKind::WorldExplain => {
            let (file, metric_id, explain_id) = split_world_explain_key(&node.key);
            BuildNodeContext {
                node: node.clone(),
                target_file: file,
                scene_id: None,
                world_metric: Some(metric_id),
                world_dataset: None,
                explain: Some(explain_id),
                projection_id: None,
                provenance,
            }
        }
        BuildNodeKind::Dataset => {
            let resource_id = node.key.clone();
            let target_file = compiled
                .resources
                .iter()
                .find(|resource| resource.id == resource_id)
                .and_then(|resource| resource.document.clone())
                .unwrap_or_else(|| compiled.active_target_file.clone());
            BuildNodeContext {
                node: node.clone(),
                target_file,
                scene_id: compiled.active_scene.clone(),
                world_metric: None,
                world_dataset: None,
                explain: None,
                projection_id: None,
                provenance,
            }
        }
        BuildNodeKind::Component | BuildNodeKind::Template => BuildNodeContext {
            node: node.clone(),
            target_file: template_target_file(compiled, node),
            scene_id: compiled.active_scene.clone(),
            world_metric: None,
            world_dataset: None,
            explain: None,
            projection_id: None,
            provenance,
        },
        BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => {
            let (board_file, scene_id) = board_context_from_node(compiled, node);
            BuildNodeContext {
                node: node.clone(),
                target_file: board_file,
                scene_id: Some(scene_id),
                world_metric: None,
                world_dataset: None,
                explain: None,
                projection_id: None,
                provenance,
            }
        }
        BuildNodeKind::Artifact | BuildNodeKind::GraphSemantic | BuildNodeKind::GraphEval => {
            BuildNodeContext {
                node: node.clone(),
                target_file: compiled.active_target_file.clone(),
                scene_id: compiled.active_scene.clone(),
                world_metric: None,
                world_dataset: None,
                explain: None,
                projection_id: None,
                provenance,
            }
        }
        BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock => {
            let (scene_id, _rest) = node
                .key
                .split_once('/')
                .map(|(scene, rest)| (scene.to_string(), rest.to_string()))
                .unwrap_or((node.key.clone(), String::new()));
            let target_file = compiled
                .scene_routes
                .iter()
                .find(|route| route.scene_id == scene_id)
                .map(|route| route.target_file.clone())
                .unwrap_or_else(|| compiled.active_target_file.clone());
            BuildNodeContext {
                node: node.clone(),
                target_file,
                scene_id: Some(scene_id),
                world_metric: None,
                world_dataset: None,
                explain: None,
                projection_id: None,
                provenance,
            }
        }
    }
}

pub fn default_build_node_for_compiled(compiled: &CompiledApp) -> BuildNodeId {
    if let Some(scene) = compiled.active_scene.as_deref() {
        return BuildNodeId::scene(scene);
    }
    if let Some(route) = compiled.scene_routes.first() {
        return BuildNodeId::route(route.scene_id.clone());
    }
    BuildNodeId::new(
        BuildNodeKind::WorldFile,
        compiled.active_target_file.clone(),
    )
}

fn split_file_symbol(key: &str) -> (String, String) {
    key.split_once('#')
        .map(|(file, symbol)| (file.to_string(), symbol.to_string()))
        .unwrap_or((key.to_string(), String::new()))
}

fn split_world_explain_key(key: &str) -> (String, String, String) {
    let mut parts = key.splitn(3, '#');
    (
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
    )
}

fn split_projection_key(key: &str) -> (String, String) {
    key.split_once('/')
        .map(|(scene, projection)| (scene.to_string(), projection.to_string()))
        .unwrap_or((key.to_string(), String::new()))
}

fn board_entry_for_node<'a>(
    compiled: &'a CompiledApp,
    node: &'a BuildNodeId,
) -> Option<&'a crate::model::BoardFileEntry> {
    compiled.build_board_index.lookup(node)
}

fn board_context_from_node(compiled: &CompiledApp, node: &BuildNodeId) -> (String, String) {
    if let Some(entry) = board_entry_for_node(compiled, node) {
        return (entry.board_file.clone(), entry.scene_id.clone());
    }
    let board_key = match node.kind {
        BuildNodeKind::BoardSlot => node
            .key
            .rsplit_once('/')
            .map(|(board, _)| board.to_string())
            .unwrap_or_else(|| node.key.clone()),
        _ => node.key.clone(),
    };
    let scene_id = board_key
        .split_once('#')
        .map(|(_, scene)| scene.to_string())
        .unwrap_or_else(|| board_key.clone());
    let board_file = board_key
        .split_once('#')
        .map(|(file, _)| file.to_string())
        .unwrap_or(board_key);
    (board_file, scene_id)
}

fn template_target_file(compiled: &CompiledApp, node: &BuildNodeId) -> String {
    compiled
        .build_template_index
        .lookup(node.key.as_str())
        .map(|entry| entry.template_file.clone())
        .unwrap_or_else(|| compiled.active_target_file.clone())
}

fn provenance_for_node(compiled: &CompiledApp, node: &BuildNodeId) -> ProvenanceAnchor {
    match node.kind {
        BuildNodeKind::Route | BuildNodeKind::Scene => ProvenanceAnchor {
            file: compiled
                .scene_routes
                .iter()
                .find(|route| route.scene_id == node.key)
                .map(|route| route.target_file.clone())
                .unwrap_or_else(|| compiled.active_target_file.clone()),
            symbol_id: node.key.clone(),
            symbol_kind: if node.kind == BuildNodeKind::Route {
                "route".to_string()
            } else {
                "scene".to_string()
            },
        },
        BuildNodeKind::Projection => {
            let (scene_id, projection_id) = split_projection_key(&node.key);
            let file = compiled
                .scene_routes
                .iter()
                .find(|route| route.scene_id == scene_id)
                .map(|route| route.target_file.clone())
                .unwrap_or_else(|| compiled.active_target_file.clone());
            ProvenanceAnchor {
                file,
                symbol_id: projection_id,
                symbol_kind: "projection".to_string(),
            }
        }
        BuildNodeKind::WorldFile => ProvenanceAnchor {
            file: node.key.clone(),
            symbol_id: world_file_symbol_id(compiled, &node.key),
            symbol_kind: "world_file".to_string(),
        },
        BuildNodeKind::WorldDataset => {
            let (file, dataset_id) = split_file_symbol(&node.key);
            ProvenanceAnchor {
                file,
                symbol_id: dataset_id,
                symbol_kind: "dataset".to_string(),
            }
        }
        BuildNodeKind::WorldMetric => {
            let (file, metric_id) = split_file_symbol(&node.key);
            ProvenanceAnchor {
                file,
                symbol_id: metric_id,
                symbol_kind: "metric".to_string(),
            }
        }
        BuildNodeKind::WorldExplain => {
            let (file, metric_id, explain_id) = split_world_explain_key(&node.key);
            ProvenanceAnchor {
                file,
                symbol_id: format!("{metric_id}/{explain_id}"),
                symbol_kind: "explain".to_string(),
            }
        }
        BuildNodeKind::Dataset => ProvenanceAnchor {
            file: compiled
                .resources
                .iter()
                .find(|resource| resource.id == node.key)
                .and_then(|resource| resource.document.clone())
                .unwrap_or_else(|| compiled.active_target_file.clone()),
            symbol_id: node.key.clone(),
            symbol_kind: "resource".to_string(),
        },
        BuildNodeKind::Component | BuildNodeKind::Template => ProvenanceAnchor {
            file: template_target_file(compiled, node),
            symbol_id: node.key.clone(),
            symbol_kind: if node.kind == BuildNodeKind::Template {
                "template".to_string()
            } else {
                "component".to_string()
            },
        },
        BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => {
            let entry = board_entry_for_node(compiled, node);
            ProvenanceAnchor {
                file: entry
                    .as_ref()
                    .map(|value| value.board_file.clone())
                    .unwrap_or_default(),
                symbol_id: entry
                    .as_ref()
                    .map(|value| value.scene_id.clone())
                    .unwrap_or_else(|| node.key.clone()),
                symbol_kind: if node.kind == BuildNodeKind::BoardSlot {
                    "board_slot".to_string()
                } else {
                    "board".to_string()
                },
            }
        }
        BuildNodeKind::Artifact => ProvenanceAnchor {
            file: String::new(),
            symbol_id: node.key.clone(),
            symbol_kind: "artifact".to_string(),
        },
        BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock => {
            let (scene_id, symbol) = node
                .key
                .split_once('/')
                .map(|(scene, rest)| (scene.to_string(), rest.to_string()))
                .unwrap_or((node.key.clone(), String::new()));
            let file = compiled
                .scene_routes
                .iter()
                .find(|route| route.scene_id == scene_id)
                .map(|route| route.target_file.clone())
                .unwrap_or_else(|| compiled.active_target_file.clone());
            ProvenanceAnchor {
                file,
                symbol_id: symbol,
                symbol_kind: if node.kind == BuildNodeKind::ScenePanel {
                    "panel".to_string()
                } else {
                    "block".to_string()
                },
            }
        }
        BuildNodeKind::GraphSemantic | BuildNodeKind::GraphEval => ProvenanceAnchor {
            file: String::new(),
            symbol_id: node.key.clone(),
            symbol_kind: node.kind.slug().to_string(),
        },
    }
}

fn world_file_symbol_id(compiled: &CompiledApp, file: &str) -> String {
    compiled
        .world_semantic_by_file
        .get(file)
        .and_then(|index| index.world_id.as_deref())
        .map(str::to_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| file.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::model::CompiledSceneRoute;

    fn sample_compiled() -> CompiledApp {
        CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: ".".to_string(),
            scene_routes: vec![CompiledSceneRoute {
                scene_id: "home".to_string(),
                frame_id: None,
                target_file: "scenes/home.mei".to_string(),
                kind: "file_ref".to_string(),
                title: None,
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
        }
    }

    #[test]
    fn scene_node_resolves_target_file() {
        let compiled = sample_compiled();
        let ctx = resolve_build_node_context(&compiled, &BuildNodeId::scene("home"));
        assert_eq!(ctx.target_file, "scenes/home.mei");
        assert_eq!(ctx.provenance.symbol_id, "home");
    }

    #[test]
    fn preview_target_from_world_dataset_node() {
        let node = BuildNodeId::world_dataset(
            "scenes/01-执法要素.world.mei",
            "agency_objects",
        );
        assert_eq!(
            preview_target_from_build_node(&node).as_deref(),
            Some("scenes/01-执法要素.world.mei")
        );
    }
}
