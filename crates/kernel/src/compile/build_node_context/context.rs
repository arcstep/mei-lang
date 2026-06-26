use super::{world_file_symbol_id};


use crate::compile::build_template_index::{
    authoring_preview_target_for_template, preview_scene_id_for_template_consumer,
    preview_scene_id_for_template_file_consumer, preview_target_for_template_consumer,
    preview_target_for_template_file_consumer,
};
use crate::model::{
    BuildNodeId, BuildNodeKind, CompiledApp, ProvenanceAnchor,
};

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
        BuildNodeKind::Component => {
            let component_key = node.key.as_str();
            let authoring =
                authoring_preview_target_for_template(compiled, component_key);
            let target_file = authoring.clone().unwrap_or_else(|| {
                template_target_file(compiled, node)
            });
            BuildNodeContext {
                node: node.clone(),
                target_file,
                scene_id: if authoring.is_some() {
                    None
                } else {
                    compiled.active_scene.clone()
                },
                world_metric: None,
                world_dataset: None,
                explain: None,
                projection_id: None,
                provenance: ProvenanceAnchor {
                    file: authoring.unwrap_or_else(|| template_target_file(compiled, node)),
                    symbol_id: node.key.clone(),
                    symbol_kind: "component".to_string(),
                },
            }
        }
        BuildNodeKind::Template => {
            let template_key = node.key.as_str();
            let authoring = authoring_preview_target_for_template(compiled, template_key);
            let target_file = authoring.clone().unwrap_or_else(|| {
                if crate::compile::build_experience::is_template_file_node_key(template_key) {
                    preview_target_for_template_file_consumer(compiled, template_key)
                        .unwrap_or_else(|| template_target_file(compiled, node))
                } else {
                    preview_target_for_template_consumer(compiled, template_key)
                        .unwrap_or_else(|| template_target_file(compiled, node))
                }
            });
            let scene_id = if authoring.is_some() {
                None
            } else if crate::compile::build_experience::is_template_file_node_key(template_key) {
                preview_scene_id_for_template_file_consumer(compiled, template_key)
                    .or_else(|| compiled.active_scene.clone())
            } else {
                preview_scene_id_for_template_consumer(compiled, template_key)
                    .or_else(|| compiled.active_scene.clone())
            };
            BuildNodeContext {
                node: node.clone(),
                target_file,
                scene_id,
                world_metric: None,
                world_dataset: None,
                explain: None,
                projection_id: None,
                provenance,
            }
        }
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
        BuildNodeKind::Artifact | BuildNodeKind::GraphSemantic | BuildNodeKind::GraphEval
        | BuildNodeKind::McgNode => {
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
    crate::compile::build_template_index::template_entry_for_preview(compiled, node.key.as_str())
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
        BuildNodeKind::Component => ProvenanceAnchor {
            file: template_target_file(compiled, node),
            symbol_id: node.key.clone(),
            symbol_kind: "component".to_string(),
        },
        BuildNodeKind::Template => ProvenanceAnchor {
            file: authoring_preview_target_for_template(compiled, node.key.as_str())
                .or_else(|| {
                    if crate::compile::build_experience::is_template_file_node_key(node.key.as_str()) {
                        crate::compile::build_experience::template_file_preview_target(
                            compiled,
                            node.key.as_str(),
                        )
                    } else {
                        None
                    }
                })
                .or_else(|| preview_target_for_template_consumer(compiled, node.key.as_str()))
                .unwrap_or_else(|| template_target_file(compiled, node)),
            symbol_id: node.key.clone(),
            symbol_kind: "template".to_string(),
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
        BuildNodeKind::GraphSemantic | BuildNodeKind::GraphEval | BuildNodeKind::McgNode => {
            ProvenanceAnchor {
                file: String::new(),
                symbol_id: node.key.clone(),
                symbol_kind: node.kind.slug().to_string(),
            }
        }
    }
}

