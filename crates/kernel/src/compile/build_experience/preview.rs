use super::{non_empty_path, split_file_symbol, split_world_explain_key};

use std::path::Path;


use crate::mei_config::{resolve_templates_root, resolve_workspace_source_root_from_app_root};
use crate::catalog_app::catalog_scene_route_for_build_node;
use crate::model::{
    BuildNodeId, BuildNodeKind, CompiledApp,
};

pub fn scene_id_from_ui_node_key(key: &str) -> Option<String> {
    key.split('/')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn is_template_file_node_key(key: &str) -> bool {
    key.contains('/') || key.ends_with(".mei")
}

pub fn template_file_preview_target(compiled: &CompiledApp, key: &str) -> Option<String> {
    if key.starts_with("templates/") {
        return Some(key.to_string());
    }
    let app_root = Path::new(compiled.app_root.as_str());
    let source_root = resolve_workspace_source_root_from_app_root(app_root);
    let templates_root = resolve_templates_root(source_root.as_path());
    let templates_prefix = templates_root
        .strip_prefix(source_root.as_path())
        .ok()
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .filter(|rel| !rel.is_empty())
        .unwrap_or_else(|| "stock/templates".to_string());
    Some(format!("{templates_prefix}/{key}"))
}

/// Convert a workspace-relative or catalog template path into an app-root-relative
/// `CompileOptions.preview_target` (e.g. `../stock/templates/cockpit/metric-card.mei`).
pub fn preview_target_relative_to_app(compiled: &CompiledApp, path: &str) -> Option<String> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty() {
        return None;
    }
    if normalized.starts_with("scenes/")
        || normalized.starts_with("../")
        || normalized.starts_with("data/")
    {
        return Some(normalized);
    }
    let app_root = Path::new(compiled.app_root.as_str());
    if app_root.join(&normalized).is_file() {
        return Some(normalized);
    }
    let source_root = resolve_workspace_source_root_from_app_root(app_root);
    let abs = if let Some(suffix) = normalized.strip_prefix("templates/") {
        resolve_templates_root(source_root.as_path()).join(suffix)
    } else if normalized.starts_with("stock/") || normalized.starts_with(".stock/") {
        source_root.join(&normalized)
    } else {
        resolve_templates_root(source_root.as_path()).join(&normalized)
    };
    relative_path_from_to(app_root, abs.as_path())
}

fn relative_path_from_to(from: &Path, to: &Path) -> Option<String> {
    let mut ups = 0usize;
    let mut base = from.to_path_buf();
    loop {
        if to.starts_with(&base) {
            let rel = to.strip_prefix(&base).ok()?;
            let mut parts: Vec<String> = (0..ups).map(|_| "..".to_string()).collect();
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if !rel_str.is_empty() {
                parts.push(rel_str);
            }
            return Some(parts.join("/"));
        }
        if !base.pop() {
            break;
        }
        ups += 1;
    }
    None
}

pub fn preview_target_for_scene_id(compiled: &CompiledApp, scene_id: &str) -> Option<String> {
    compiled
        .scene_routes
        .iter()
        .find(|route| route.scene_id == scene_id)
        .map(|route| route.target_file.clone())
}

pub fn preview_target_from_build_node_with_app(
    node: &BuildNodeId,
    compiled: Option<&CompiledApp>,
) -> Option<String> {
    match node.kind {
        BuildNodeKind::WorldFile => Some(node.key.clone()),
        BuildNodeKind::WorldDataset | BuildNodeKind::WorldMetric => {
            let (file, _) = split_file_symbol(&node.key);
            non_empty_path(file)
        }
        BuildNodeKind::WorldExplain => {
            let (file, _, _) = split_world_explain_key(&node.key);
            non_empty_path(file)
        }
        BuildNodeKind::Scene | BuildNodeKind::Route => {
            compiled.and_then(|app| preview_target_for_scene_id(app, node.key.as_str()))
        }
        BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock => {
            let scene_id = scene_id_from_ui_node_key(&node.key)?;
            compiled.and_then(|app| preview_target_for_scene_id(app, scene_id.as_str()))
        }
        BuildNodeKind::Projection => {
            let scene_id = node.key.split('/').next()?;
            compiled.and_then(|app| preview_target_for_scene_id(app, scene_id))
        }
        BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => {
            if let Some(entry) = compiled.and_then(|app| app.build_board_index.lookup(node)) {
                return Some(entry.board_file.clone());
            }
            let (file, _) = board_capsule_from_node_key(&node.key);
            non_empty_path(file)
        }
        BuildNodeKind::Component => compiled.and_then(|app| {
            if let Some(route) = catalog_scene_route_for_build_node(app, node) {
                return Some(route.target_file.clone());
            }
            crate::compile::build_template_index::authoring_preview_target_for_template(
                app,
                node.key.as_str(),
            )
        }),
        BuildNodeKind::Template => compiled.and_then(|app| {
            if let Some(route) = catalog_scene_route_for_build_node(app, node) {
                return Some(route.target_file.clone());
            }
            crate::compile::build_template_index::authoring_preview_target_for_template(
                app,
                node.key.as_str(),
            )
            .or_else(|| template_consumer_preview_target(app, node.key.as_str()))
        }),
        _ => None,
    }
}

fn template_consumer_preview_target(compiled: &CompiledApp, template_key: &str) -> Option<String> {
    if is_template_file_node_key(template_key) {
        crate::compile::build_template_index::preview_target_for_template_file_consumer(
            compiled,
            template_key,
        )
    } else {
        crate::compile::build_template_index::preview_target_for_template_consumer(compiled, template_key)
    }
}

/// `(board_file, scene_export_id)` parsed from board-file / board-slot node keys.
pub fn board_capsule_from_node_key(key: &str) -> (String, String) {
    let board_key = key
        .rsplit_once('/')
        .filter(|(base, _)| base.contains('#'))
        .map(|(base, _)| base)
        .unwrap_or(key);
    board_key
        .split_once('#')
        .map(|(file, scene)| (file.to_string(), scene.to_string()))
        .unwrap_or_else(|| (board_key.to_string(), String::new()))
}

pub fn compile_scene_from_build_node(node: &BuildNodeId) -> Option<String> {
    match node.kind {
        BuildNodeKind::Scene | BuildNodeKind::Route => non_empty_path(node.key.clone()),
        BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock | BuildNodeKind::Projection => {
            scene_id_from_ui_node_key(&node.key)
        }
        BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => {
            non_empty_path(board_capsule_from_node_key(&node.key).1)
        }
        _ => None,
    }
}

pub fn compile_scene_from_build_node_with_app(
    node: &BuildNodeId,
    compiled: Option<&CompiledApp>,
) -> Option<String> {
    if matches!(node.kind, BuildNodeKind::Component | BuildNodeKind::Template) {
        if let Some(app) = compiled {
            if let Some(route) = catalog_scene_route_for_build_node(app, node) {
                return Some(route.scene_id.clone());
            }
            if crate::compile::build_template_index::authoring_preview_target_for_template(
                app,
                node.key.as_str(),
            )
            .is_some()
            {
                return None;
            }
            if node.kind == BuildNodeKind::Component {
                return None;
            }
            if is_template_file_node_key(node.key.as_str()) {
                return crate::compile::build_template_index::preview_scene_id_for_template_file_consumer(
                    app,
                    node.key.as_str(),
                )
                .or_else(|| app.active_scene.clone());
            }
            return crate::compile::build_template_index::preview_scene_id_for_template_consumer(
                app,
                node.key.as_str(),
            )
            .or_else(|| app.active_scene.clone());
        }
        return None;
    }
    if node.kind == BuildNodeKind::WorldFile {
        let board_file = node.key.as_str();
        if board_file.ends_with(".board.mei") {
            if let Some(app) = compiled {
                return app
                    .build_board_index
                    .default_export_scene_for_board_file(board_file);
            }
        }
    }
    compile_scene_from_build_node(node)
}

/// Compile coordinate for fast build navigation (scene + preview target; inspect node is excluded).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildPreviewKind {
    SceneCapsule,
    BoardCapsule,
    WorldCapsule,
    Script,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BuildCompileCoordinate {
    pub scene_id: Option<String>,
    pub preview_target: String,
    pub preview_kind: BuildPreviewKind,
}

