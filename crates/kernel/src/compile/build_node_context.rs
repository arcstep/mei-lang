use std::collections::BTreeMap;
use std::path::Path;

use super::build_template_index::{
    authoring_preview_target_for_template, preview_scene_id_for_template_consumer,
    preview_scene_id_for_template_file_consumer, preview_target_for_template_consumer,
    preview_target_for_template_file_consumer,
};
use crate::model::{
    BlockDecl, BuildNodeId, BuildNodeKind, CompiledApp, PanelDecl, ProvenanceAnchor, UiNodeDecl,
};

/// Script path used as `CompileOptions.preview_target` before compile, when the build URL
/// selects a node via `?node=` without legacy `?file=`.
pub fn preview_target_from_build_node(node: &BuildNodeId) -> Option<String> {
    super::build_experience::preview_target_from_build_node_with_app(node, None)
}

fn panel_path_for_use_key(
    panel: &PanelDecl,
    parent_path: Option<&str>,
    use_key: &str,
) -> Option<String> {
    let panel_path = match parent_path {
        Some(parent) => format!("{parent}/{}", panel.id),
        None => panel.id.clone(),
    };
    for node in &panel.blocks {
        match node {
            UiNodeDecl::Block(BlockDecl { use_key: key, .. }) if key.as_str() == use_key => {
                return Some(panel_path);
            }
            UiNodeDecl::Panel(nested) => {
                if let Some(found) =
                    panel_path_for_use_key(nested, Some(panel_path.as_str()), use_key)
                {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

/// When build preview compiles an authoring example, SSR may scope to the panel that
/// hosts the selected component so one tree node maps to one preview surface.
pub fn build_preview_panel_scope(compiled: &CompiledApp, node: &BuildNodeId) -> Option<String> {
    match node.kind {
        BuildNodeKind::Component => {
            let contract = compiled.scene_contract.as_ref()?;
            let scene_id = contract.scene.id.as_str();
            for panel in &contract.panels {
                if let Some(panel_path) =
                    panel_path_for_use_key(panel, None, node.key.as_str())
                {
                    return Some(format!("{scene_id}/{panel_path}"));
                }
            }
            None
        }
        BuildNodeKind::ScenePanel => {
            let key = node.key.trim();
            if key.is_empty() {
                None
            } else {
                Some(key.to_string())
            }
        }
        BuildNodeKind::SceneBlock => node
            .key
            .rsplit_once('/')
            .map(|(panel_path, _)| panel_path.to_string()),
        _ => None,
    }
}

pub fn catalog_preview_target_for_build_node(
    app_root: &Path,
    node: &BuildNodeId,
) -> Option<String> {
    let scene_routes = crate::catalog_app::catalog_scene_routes_from_app_root(app_root);
    if scene_routes.is_empty() {
        return None;
    }
    let active_target_file = scene_routes
        .first()
        .map(|route| route.target_file.clone())
        .unwrap_or_default();
    let active_scene = scene_routes.first().map(|route| route.scene_id.clone());
    let stub = CompiledApp {
        app_id: String::new(),
        title: String::new(),
        app_root: app_root.display().to_string(),
        scene_routes,
        active_scene,
        active_target_file,
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
    super::build_experience::preview_target_from_build_node_with_app(node, Some(&stub))
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
                if super::build_experience::is_template_file_node_key(template_key) {
                    preview_target_for_template_file_consumer(compiled, template_key)
                        .unwrap_or_else(|| template_target_file(compiled, node))
                } else {
                    preview_target_for_template_consumer(compiled, template_key)
                        .unwrap_or_else(|| template_target_file(compiled, node))
                }
            });
            let scene_id = if authoring.is_some() {
                None
            } else if super::build_experience::is_template_file_node_key(template_key) {
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
    super::build_template_index::template_entry_for_preview(compiled, node.key.as_str())
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
                    if super::build_experience::is_template_file_node_key(node.key.as_str()) {
                        super::build_experience::template_file_preview_target(
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
        let node = BuildNodeId::world_dataset("scenes/01-执法要素.world.mei", "agency_objects");
        assert_eq!(
            preview_target_from_build_node(&node).as_deref(),
            Some("scenes/01-执法要素.world.mei")
        );
    }

    #[test]
    fn component_authoring_preview_panel_scope_targets_host_panel() {
        use crate::compile::{
            compile_app_from_root_with_options, CompileOptions,
        };
        use crate::BuildPreviewKind;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root")
            .join("workspaces")
            .join("ws-hello");
        let app_root = source_root.join("apps/hello");
        let example = source_root.join("stock/authoring/examples/chart-baseline.mei");
        if !app_root.is_dir() || !example.is_file() {
            return;
        }
        let home = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions::default(),
        )
        .expect("compile hello home");
        let coord = super::super::build_experience::compile_coordinate_for_node(
            &BuildNodeId::component("chart.area"),
            &home,
        )
        .expect("coord");
        assert_eq!(coord.preview_kind, BuildPreviewKind::Script);
        let preview = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions {
                preview_target: Some(coord.preview_target.clone()),
                ..CompileOptions::default()
            },
        )
        .expect("compile chart.area preview");
        assert!(
            preview.scene_contract.is_some(),
            "expected scene contract for chart.area preview"
        );
        let scope = build_preview_panel_scope(&preview, &BuildNodeId::component("chart.area"));
        assert_eq!(scope.as_deref(), Some("home/area_panel"));
    }
}
