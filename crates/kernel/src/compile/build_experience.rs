use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use crate::mei_config::{resolve_templates_root, resolve_workspace_source_root_from_app_root};
use crate::model::{
    BlockDecl, BuildNodeId, BuildNodeKind, CompiledApp, ExperienceNodeManifest, PanelDecl,
    UiNodeDecl,
};

/// First path segment of scene-scoped UI node keys (`home/panel/block`).
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
/// `CompileOptions.preview_target` (e.g. `../.stock/templates/cockpit/metric-card.mei`).
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

pub(crate) fn preview_target_for_absolute_path(
    compiled: &CompiledApp,
    abs_path: &Path,
) -> Option<String> {
    relative_path_from_to(Path::new(compiled.app_root.as_str()), abs_path)
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
        BuildNodeKind::Template => compiled.and_then(|app| {
            super::build_template_index::authoring_preview_target_for_template(
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
        super::build_template_index::preview_target_for_template_file_consumer(
            compiled,
            template_key,
        )
    } else {
        super::build_template_index::preview_target_for_template_consumer(compiled, template_key)
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
    if node.kind == BuildNodeKind::Template {
        if let Some(app) = compiled {
            if super::build_template_index::authoring_preview_target_for_template(
                app,
                node.key.as_str(),
            )
            .is_some()
            {
                return None;
            }
            if is_template_file_node_key(node.key.as_str()) {
                return super::build_template_index::preview_scene_id_for_template_file_consumer(
                    app,
                    node.key.as_str(),
                )
                .or_else(|| app.active_scene.clone());
            }
            return super::build_template_index::preview_scene_id_for_template_consumer(
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

pub fn compile_coordinate_for_node(
    node: &BuildNodeId,
    compiled: &CompiledApp,
) -> Option<BuildCompileCoordinate> {
    let preview_target = preview_target_from_build_node_with_app(node, Some(compiled))?;
    let scene_id = compile_scene_from_build_node_with_app(node, Some(compiled))
        .or_else(|| compile_scene_from_build_node(node));
    let preview_kind = match node.kind {
        BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => BuildPreviewKind::BoardCapsule,
        BuildNodeKind::WorldFile => {
            if node.key.ends_with(".board.mei") {
                BuildPreviewKind::BoardCapsule
            } else {
                BuildPreviewKind::WorldCapsule
            }
        }
        BuildNodeKind::WorldDataset
        | BuildNodeKind::WorldMetric
        | BuildNodeKind::WorldExplain => BuildPreviewKind::WorldCapsule,
        BuildNodeKind::Scene
        | BuildNodeKind::Route
        | BuildNodeKind::ScenePanel
        | BuildNodeKind::SceneBlock
        | BuildNodeKind::Projection => {
            if preview_target.ends_with(".board.mei") {
                BuildPreviewKind::BoardCapsule
            } else {
                BuildPreviewKind::SceneCapsule
            }
        }
        BuildNodeKind::Template => {
            if super::build_template_index::authoring_preview_target_for_template(
                compiled,
                node.key.as_str(),
            )
            .is_some()
            {
                BuildPreviewKind::Script
            } else if is_template_file_node_key(node.key.as_str()) {
                if super::build_template_index::preview_scene_id_for_template_file_consumer(
                    compiled,
                    node.key.as_str(),
                )
                .is_some()
                {
                    BuildPreviewKind::SceneCapsule
                } else {
                    BuildPreviewKind::Other
                }
            } else if super::build_template_index::preview_scene_id_for_template_consumer(
                compiled,
                node.key.as_str(),
            )
            .is_some()
            {
                BuildPreviewKind::SceneCapsule
            } else {
                BuildPreviewKind::Other
            }
        }
        BuildNodeKind::Artifact | BuildNodeKind::GraphSemantic | BuildNodeKind::GraphEval => {
            BuildPreviewKind::Other
        }
        BuildNodeKind::Dataset | BuildNodeKind::Component => BuildPreviewKind::Script,
    };
    Some(BuildCompileCoordinate {
        scene_id,
        preview_target,
        preview_kind,
    })
}

/// Human-readable breadcrumb segments for build overview / agent export.
pub fn build_experience_path(compiled: &CompiledApp, node: &BuildNodeId) -> Vec<String> {
    if let Some(manifest) = ExperienceNodeManifest::lookup(compiled, node) {
        if !manifest.experience_path.is_empty() {
            return manifest.experience_path.clone();
        }
    }
    build_experience_path_runtime(compiled, node)
}

fn build_experience_path_runtime(compiled: &CompiledApp, node: &BuildNodeId) -> Vec<String> {
    match node.kind {
        BuildNodeKind::Route | BuildNodeKind::Scene => scene_label(compiled, &node.key),
        BuildNodeKind::Projection => {
            let (scene_id, projection_id) = split_projection_key(&node.key);
            let mut path = scene_label(compiled, &scene_id);
            path.push(projection_label(projection_id.as_str()));
            path
        }
        BuildNodeKind::ScenePanel => {
            let (scene_id, panel_path) = split_panel_key(&node.key);
            let mut path = scene_label(compiled, &scene_id);
            if let Some(panel) = find_panel_by_path(compiled, &scene_id, panel_path.as_str()) {
                path.push(panel_label(&panel));
            } else {
                path.push(panel_path);
            }
            path
        }
        BuildNodeKind::SceneBlock => {
            let (scene_id, panel_path, block_id) = split_block_key(&node.key);
            let mut path = scene_label(compiled, &scene_id);
            if let Some(panel) = find_panel_by_path(compiled, &scene_id, panel_path.as_str()) {
                path.push(panel_label(&panel));
                if let Some(block) = find_block_in_panel(&panel, block_id.as_str()) {
                    path.push(block_label(&block));
                } else {
                    path.push(block_id);
                }
            } else {
                path.push(panel_path);
                path.push(block_id);
            }
            path
        }
        BuildNodeKind::WorldFile => vec!["Backing · World".to_string(), node.key.clone()],
        BuildNodeKind::WorldDataset | BuildNodeKind::WorldMetric => {
            let (file, symbol) = split_file_symbol(&node.key);
            vec!["Backing · World".to_string(), file, symbol]
        }
        BuildNodeKind::WorldExplain => {
            let (file, metric, explain) = split_world_explain_key(&node.key);
            vec!["Backing · World".to_string(), file, metric, explain]
        }
        BuildNodeKind::Dataset => vec!["Backing · Datasets".to_string(), node.key.clone()],
        BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => {
            if let Some(entry) = compiled.build_board_index.lookup(node) {
                vec![
                    "Board".to_string(),
                    entry.label.clone(),
                    entry.scene_id.clone(),
                ]
            } else {
                vec!["Board".to_string(), node.key.clone()]
            }
        }
        BuildNodeKind::Template => {
            if let Some(entry) = compiled.build_template_index.lookup(node.key.as_str()) {
                let mut rows = vec![
                    "Template".to_string(),
                    entry.template_key.clone(),
                    entry.template_file.clone(),
                ];
                if let Some(anchor) = super::build_template_index::template_primary_consumer(
                    compiled,
                    entry.template_key.as_str(),
                ) {
                    rows.push(format!(
                        "→ {} / {} / {}",
                        anchor.scene_id, anchor.panel_path, anchor.label
                    ));
                }
                rows
            } else {
                vec!["Template".to_string(), node.key.clone()]
            }
        }
        _ => vec![node.encode()],
    }
}

pub fn backing_refs_from_block_props(props: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    collect_backing_refs(props, &mut refs);
    dedupe_preserve_order(&mut refs);
    refs
}

pub fn aggregate_use_key_badges(blocks: &[UiNodeDecl]) -> Vec<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for block in blocks {
        if let UiNodeDecl::Block(block) = block {
            *counts.entry(block.use_key.clone()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|(use_key, count)| {
            if count > 1 {
                format!("{use_key} ×{count}")
            } else {
                use_key
            }
        })
        .collect()
}

fn scene_label(compiled: &CompiledApp, scene_id: &str) -> Vec<String> {
    let label = compiled
        .scene_routes
        .iter()
        .find(|route| route.scene_id == scene_id)
        .and_then(|route| route.title.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| scene_id.to_string());
    vec![label]
}

fn projection_label(projection_id: &str) -> String {
    format!("Board · {projection_id}")
}

fn panel_label(panel: &PanelDecl) -> String {
    panel
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panel.id.clone())
}

pub fn block_instance_id(block: &BlockDecl, ordinal: usize) -> String {
    let stem = block
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| block.use_key.clone());
    format!("{stem}~{ordinal}")
}

fn block_label(block: &BlockDecl) -> String {
    block
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            block
                .id
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| block.use_key.clone())
        })
}

fn find_panel_by_path(
    compiled: &CompiledApp,
    scene_id: &str,
    panel_path: &str,
) -> Option<PanelDecl> {
    let top_level = panels_for_scene(compiled, scene_id)?;
    let mut segments = panel_path.split('/').filter(|s| !s.is_empty());
    let first = segments.next()?;
    let mut current = top_level.into_iter().find(|panel| panel.id == first)?;
    for segment in segments {
        current = current.blocks.iter().find_map(|node| match node {
            UiNodeDecl::Panel(panel) if panel.id == segment => Some(panel.clone()),
            _ => None,
        })?;
    }
    Some(current)
}

fn find_block_in_panel(panel: &PanelDecl, block_id: &str) -> Option<BlockDecl> {
    for (ordinal, block) in blocks_in_panel(panel).iter().enumerate() {
        if block_instance_id(block, ordinal) == block_id {
            return Some((*block).clone());
        }
    }
    None
}

fn blocks_in_panel(panel: &PanelDecl) -> Vec<&BlockDecl> {
    panel
        .blocks
        .iter()
        .filter_map(|node| match node {
            UiNodeDecl::Block(block) => Some(block),
            _ => None,
        })
        .collect()
}

pub fn panels_for_scene(compiled: &CompiledApp, scene_id: &str) -> Option<Vec<PanelDecl>> {
    compiled
        .scene_projection_assembly_by_id
        .get(scene_id)
        .and_then(|assembly| assembly.get("panels"))
        .and_then(|value| serde_json::from_value::<Vec<PanelDecl>>(value.clone()).ok())
}

fn split_panel_key(key: &str) -> (String, String) {
    let mut parts = key.splitn(2, '/');
    (
        parts.next().unwrap_or("").to_string(),
        parts.next().unwrap_or("").to_string(),
    )
}

fn split_block_key(key: &str) -> (String, String, String) {
    let mut parts = key.splitn(3, '/');
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

fn non_empty_path(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn collect_backing_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(ref_kind) = map.get("__ref").and_then(Value::as_str) {
                match ref_kind {
                    "data" | "dataset" | "resource" | "entity" => {
                        if let Some(id) = map
                            .get("id")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                        {
                            out.push(format!("→ {id}"));
                        }
                    }
                    "metric" => {
                        if let Some(id) = map.get("id").and_then(Value::as_str) {
                            let from = map
                                .get("from_dataset")
                                .or_else(|| map.get("from"))
                                .and_then(Value::as_str);
                            if let Some(dataset) = from.filter(|s| !s.trim().is_empty()) {
                                out.push(format!("→ {dataset}::{id}"));
                            } else {
                                out.push(format!("→ metric:{id}"));
                            }
                        }
                    }
                    _ => {}
                }
            }
            for child in map.values() {
                collect_backing_refs(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_backing_refs(item, out);
            }
        }
        _ => {}
    }
}

pub fn build_overview_backing(compiled: &CompiledApp, node: &BuildNodeId) -> Vec<String> {
    if let Some(manifest) = ExperienceNodeManifest::lookup(compiled, node) {
        if !manifest.backing_refs.is_empty() {
            return manifest.backing_refs.clone();
        }
    }
    build_overview_backing_runtime(compiled, node)
}

pub fn experience_mount_chain(
    compiled: &CompiledApp,
    node: &BuildNodeId,
) -> Vec<crate::model::MountChainEntry> {
    ExperienceNodeManifest::lookup(compiled, node)
        .map(|manifest| manifest.mount_chain.clone())
        .unwrap_or_default()
}

pub fn experience_layout_hint(compiled: &CompiledApp, node: &BuildNodeId) -> Option<String> {
    ExperienceNodeManifest::lookup(compiled, node).and_then(|manifest| manifest.layout_hint.clone())
}

fn build_overview_backing_runtime(compiled: &CompiledApp, node: &BuildNodeId) -> Vec<String> {
    use BuildNodeKind::*;
    match node.kind {
        SceneBlock => {
            let (scene_id, panel_path, block_id) = split_block_key(&node.key);
            let Some(panel) = find_panel_by_path(compiled, &scene_id, panel_path.as_str()) else {
                return Vec::new();
            };
            let Some(block) = find_block_in_panel(&panel, block_id.as_str()) else {
                return Vec::new();
            };
            backing_refs_from_block_props(&block.props)
        }
        ScenePanel => {
            let (scene_id, panel_path) = split_panel_key(&node.key);
            let Some(panel) = find_panel_by_path(compiled, &scene_id, panel_path.as_str()) else {
                return Vec::new();
            };
            let mut refs = Vec::new();
            for ui_node in &panel.blocks {
                if let UiNodeDecl::Block(block) = ui_node {
                    refs.extend(backing_refs_from_block_props(&block.props));
                }
            }
            dedupe_preserve_order(&mut refs);
            refs
        }
        WorldDataset | WorldMetric => {
            vec![format!("world: {}", node.key.replace('#', " › "))]
        }
        Dataset => vec![format!("resource: {}", node.key)],
        _ => Vec::new(),
    }
}

pub fn format_experience_path(path: &[String]) -> String {
    path.join(" › ")
}

fn dedupe_preserve_order(items: &mut Vec<String>) {
    let mut seen = BTreeMap::<String, ()>::new();
    items.retain(|item| {
        if seen.contains_key(item) {
            false
        } else {
            seen.insert(item.clone(), ());
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backing_refs_from_metric_binding() {
        let props = serde_json::json!({
            "metric": { "__ref": "metric", "id": "total", "from_dataset": "agency_objects" }
        });
        let refs = backing_refs_from_block_props(&props);
        assert!(refs.iter().any(|r| r.contains("agency_objects")));
    }

    #[test]
    fn board_build_node_resolves_preview_target_and_scene() {
        use crate::model::BuildNodeId;

        let node = BuildNodeId::board_file(
            "scenes/01-执法要素.board.mei#enforcement_units_analytics_board",
        );
        assert_eq!(
            preview_target_from_build_node_with_app(&node, None).as_deref(),
            Some("scenes/01-执法要素.board.mei")
        );
        assert_eq!(
            compile_scene_from_build_node(&node).as_deref(),
            Some("enforcement_units_analytics_board")
        );
        let slot = BuildNodeId::board_slot(
            "scenes/01-执法要素.board.mei#enforcement_units_analytics_board",
            "hero",
        );
        assert_eq!(
            preview_target_from_build_node_with_app(&slot, None).as_deref(),
            Some("scenes/01-执法要素.board.mei")
        );
        assert_eq!(
            compile_scene_from_build_node(&slot).as_deref(),
            Some("enforcement_units_analytics_board")
        );
    }

    #[test]
    fn compile_scene_from_panel_node() {
        let node = BuildNodeId::scene_panel("home", "kpi_row");
        assert_eq!(
            compile_scene_from_build_node(&node).as_deref(),
            Some("home")
        );
    }

    #[test]
    fn compile_coordinate_board_exports_share_preview_target() {
        use crate::model::{BuildNodeId, CompiledApp, CompiledSceneRoute};
        use std::collections::BTreeMap;

        let compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: "zhifa".to_string(),
            scene_routes: vec![CompiledSceneRoute {
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
        let board_a = BuildNodeId::board_file("scenes/01.board.mei#board_a");
        let board_b = BuildNodeId::board_file("scenes/01.board.mei#board_b");
        let slot = BuildNodeId::board_slot("scenes/01.board.mei#board_a", "chart");
        let coord_a = compile_coordinate_for_node(&board_a, &compiled).expect("board a");
        let coord_b = compile_coordinate_for_node(&board_b, &compiled).expect("board b");
        let coord_slot = compile_coordinate_for_node(&slot, &compiled).expect("slot");
        assert_eq!(coord_a.preview_target, "scenes/01.board.mei");
        assert_eq!(coord_b.preview_target, coord_a.preview_target);
        assert_eq!(coord_slot.preview_target, coord_a.preview_target);
        assert_ne!(coord_a.scene_id, coord_b.scene_id);
        assert_eq!(coord_slot.scene_id, coord_a.scene_id);
    }

    #[test]
    fn compile_coordinate_groups_scene_panels_with_scene_route() {
        use crate::model::{BuildNodeId, CompiledApp, CompiledSceneRoute};
        use std::collections::BTreeMap;

        let compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: "zhifa".to_string(),
            scene_routes: vec![CompiledSceneRoute {
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
        let scene = BuildNodeId::scene("home");
        let panel = BuildNodeId::scene_panel("home", "kpi_row");
        let scene_coord = compile_coordinate_for_node(&scene, &compiled).expect("scene coord");
        let panel_coord = compile_coordinate_for_node(&panel, &compiled).expect("panel coord");
        assert_eq!(scene_coord.preview_target, "scenes/home.mei");
        assert_eq!(panel_coord.preview_target, "scenes/home.mei");
        assert_eq!(scene_coord.scene_id.as_deref(), Some("home"));
        assert_eq!(panel_coord.scene_id.as_deref(), Some("home"));
    }

    #[test]
    fn compile_coordinate_for_template_file_uses_authoring_preview() {
        use crate::model::{
            BuildNodeId, BuildTemplateIndex, CompiledApp, CompiledSceneRoute, TemplateCatalogEntry,
            TemplateConsumerAnchor,
        };
        use std::collections::BTreeMap;

        let mut templates = BTreeMap::new();
        templates.insert(
            "cockpit.main".to_string(),
            TemplateCatalogEntry {
                template_key: "cockpit.main".to_string(),
                template_file: ".stock/templates/cockpit/main.mei".to_string(),
                category: "component".to_string(),
                props_schema: Vec::new(),
                variants: Vec::new(),
                consumers: vec!["home/header".to_string()],
                consumer_anchors: vec![TemplateConsumerAnchor {
                    scene_id: "home".to_string(),
                    panel_path: "header".to_string(),
                    block_id: "cockpit.main~0".to_string(),
                    label: "Header".to_string(),
                }],
                agent_hint: None,
            },
        );
        let compiled = CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: "zhifa".to_string(),
            scene_routes: vec![CompiledSceneRoute {
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
            build_template_index: BuildTemplateIndex { templates },
        };
        let node = BuildNodeId::template("cockpit/main.mei");
        let coord = compile_coordinate_for_node(&node, &compiled).expect("coord");
        assert_eq!(coord.scene_id, None);
        assert_eq!(coord.preview_target, "../.stock/templates/cockpit/main.mei");
        assert_eq!(coord.preview_kind, BuildPreviewKind::Script);
    }

    #[test]
    fn block_instance_id_always_includes_ordinal() {
        let block = BlockDecl {
            kind: "block".to_string(),
            use_key: "mei.text".to_string(),
            id: Some("mei.text".to_string()),
            title: None,
            area: None,
            props: serde_json::Value::Null,
            base: None,
            layout: None,
            blocks: Vec::new(),
            component: None,
            placement: None,
            interactions: Vec::new(),
            lifecycle: None,
            constraints: None,
            data: None,
        };
        assert_eq!(block_instance_id(&block, 0), "mei.text~0");
        assert_eq!(block_instance_id(&block, 1), "mei.text~1");
    }

    #[test]
    fn ws_hello_chart_bar_resolves_authoring_example_preview() {
        use std::path::Path;

        use crate::compile::build_experience::preview_target_from_build_node_with_app;
        use crate::compile::{compile_app_from_root_with_options, CompileOptions};
        use crate::mei_config::WORKSPACE_CONFIG_FILENAME;
        use crate::model::BuildNodeId;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root")
            .join("workspaces")
            .join("ws-hello");
        if !source_root.join(WORKSPACE_CONFIG_FILENAME).is_file() {
            return;
        }
        let app_root = source_root.join("apps").join("hello");
        if !app_root.is_dir() {
            return;
        }
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions::default(),
        )
        .expect("compile hello");
        let node = BuildNodeId::template("chart.bar");
        let preview = preview_target_from_build_node_with_app(&node, Some(&compiled))
            .expect("chart.bar preview target");
        assert!(
            preview.contains("chart-baseline.mei"),
            "chart.bar should preview stock authoring example, got {preview}"
        );
    }

    #[test]
    fn ws_hello_doc_markdown_resolves_scene_consumer_preview() {
        use std::path::Path;

        use crate::compile::build_experience::{
            compile_coordinate_for_node, preview_target_from_build_node_with_app,
        };
        use crate::compile::build_node_context::resolve_build_node_context;
        use crate::compile::{compile_app_from_root_with_options, CompileOptions};
        use crate::mei_config::WORKSPACE_CONFIG_FILENAME;
        use crate::model::BuildNodeId;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root")
            .join("workspaces")
            .join("ws-hello");
        if !source_root.join(WORKSPACE_CONFIG_FILENAME).is_file() {
            return;
        }
        let app_root = source_root.join("apps").join("hello");
        if !app_root.is_dir() {
            return;
        }
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions::default(),
        )
        .expect("compile hello");
        let node = BuildNodeId::template("doc.markdown");
        let entry = compiled.build_template_index.lookup("doc.markdown");
        assert!(
            entry.is_some(),
            "doc.markdown should be indexed from home scene compile"
        );
        assert!(
            !entry.expect("entry").consumer_anchors.is_empty(),
            "doc.markdown should have consumer anchors from home scene"
        );
        let preview = preview_target_from_build_node_with_app(&node, Some(&compiled))
            .expect("preview target");
        assert!(
            preview.contains("dataset-baseline.mei") || preview.contains("home"),
            "doc.markdown should preview authoring example or home consumer scene, got {preview}"
        );
        let ctx = resolve_build_node_context(&compiled, &node);
        assert!(
            ctx.target_file.ends_with(".mei"),
            "build context should not fall back to raw js, got {}",
            ctx.target_file
        );
        let coord = compile_coordinate_for_node(&node, &compiled).expect("coord");
        assert!(
            coord.preview_target.ends_with(".mei"),
            "coord preview should be scene mei, got {}",
            coord.preview_target
        );
    }

    #[test]
    fn v2_template_file_preview_resolves_stock_templates_path() {
        use crate::compile::{compile_app_from_root_with_options, CompileOptions};
        use crate::mei_config::WORKSPACE_CONFIG_FILENAME;
        use crate::model::BuildNodeId;

        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("workspace root")
            .join("workspaces")
            .join("ws-hello");
        if !source_root.join(WORKSPACE_CONFIG_FILENAME).is_file() {
            return;
        }
        let app_root = source_root.join("apps").join("hello");
        if !app_root.is_dir() {
            return;
        }
        let template_key = "cockpit/metric-card.mei";
        if !source_root
            .join("stock/templates")
            .join(template_key)
            .is_file()
        {
            return;
        }
        let compiled = compile_app_from_root_with_options(
            &source_root,
            &app_root,
            CompileOptions::default(),
        )
        .expect("compile hello");
        let node = BuildNodeId::template(template_key);
        let target = preview_target_from_build_node_with_app(&node, Some(&compiled))
            .expect("template preview target");
        assert!(
            target.contains("metric-card.mei"),
            "expected stock template path, got {target}"
        );
        assert!(
            preview_target_relative_to_app(&compiled, &target)
                .is_some_and(|rel| rel.contains("metric-card.mei")),
            "preview target should compile from app-relative stock path"
        );
    }
}
