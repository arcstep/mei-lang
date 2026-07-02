use super::{BuildCompileCoordinate, BuildPreviewKind};

use super::{compile_scene_from_build_node, compile_scene_from_build_node_with_app, is_template_file_node_key, preview_target_from_build_node_with_app};



use crate::catalog_app::catalog_scene_route_for_build_node;
use crate::model::{
    BuildNodeId, BuildNodeKind, CompiledApp,
};

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
            if node.key.ends_with(".board.mei") || node.key.ends_with(".page.mei") {
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
        | BuildNodeKind::UiScope
        | BuildNodeKind::Projection => {
            if preview_target.ends_with(".board.mei") || preview_target.ends_with(".page.mei") {
                BuildPreviewKind::BoardCapsule
            } else {
                BuildPreviewKind::SceneCapsule
            }
        }
        BuildNodeKind::Component => {
            if catalog_scene_route_for_build_node(compiled, node).is_some() {
                BuildPreviewKind::SceneCapsule
            } else if crate::compile::build_template_index::authoring_preview_target_for_template(
                compiled,
                node.key.as_str(),
            )
            .is_some()
            {
                BuildPreviewKind::Script
            } else {
                BuildPreviewKind::Other
            }
        }
        BuildNodeKind::Template => {
            if catalog_scene_route_for_build_node(compiled, node).is_some() {
                BuildPreviewKind::SceneCapsule
            } else if crate::compile::build_template_index::authoring_preview_target_for_template(
                compiled,
                node.key.as_str(),
            )
            .is_some()
            {
                BuildPreviewKind::Script
            } else if is_template_file_node_key(node.key.as_str()) {
                if crate::compile::build_template_index::preview_scene_id_for_template_file_consumer(
                    compiled,
                    node.key.as_str(),
                )
                .is_some()
                {
                    BuildPreviewKind::SceneCapsule
                } else {
                    BuildPreviewKind::Other
                }
            } else if crate::compile::build_template_index::preview_scene_id_for_template_consumer(
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
        BuildNodeKind::Artifact | BuildNodeKind::GraphSemantic | BuildNodeKind::GraphEval
        | BuildNodeKind::McgNode => BuildPreviewKind::Other,
        BuildNodeKind::Dataset => BuildPreviewKind::Script,
    };
    Some(BuildCompileCoordinate {
        scene_id,
        preview_target,
        preview_kind,
    })
}

