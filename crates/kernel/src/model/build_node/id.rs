use super::{tab_visible_for_node, tabs_for_node_kind, BuildViewTab};

use serde::{Deserialize, Serialize};

/// Stable identifier for a node in the build-view reachability tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BuildNodeId {
    pub kind: BuildNodeKind,
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildNodeKind {
    Route,
    Scene,
    ScenePanel,
    SceneBlock,
    Projection,
    WorldFile,
    WorldDataset,
    WorldMetric,
    WorldExplain,
    Dataset,
    Component,
    BoardFile,
    BoardSlot,
    Template,
    Artifact,
    GraphSemantic,
    GraphEval,
    McgNode,
    UiScope,
}

impl BuildNodeKind {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Route => "route",
            Self::Scene => "scene",
            Self::ScenePanel => "scene-panel",
            Self::SceneBlock => "scene-block",
            Self::Projection => "projection",
            Self::WorldFile => "world-file",
            Self::WorldDataset => "world-dataset",
            Self::WorldMetric => "world-metric",
            Self::WorldExplain => "world-explain",
            Self::Dataset => "dataset",
            Self::Component => "component",
            Self::BoardFile => "board-file",
            Self::BoardSlot => "board-slot",
            Self::Template => "template",
            Self::Artifact => "artifact",
            Self::GraphSemantic => "graph-semantic",
            Self::GraphEval => "graph-eval",
            Self::McgNode => "mcg-node",
            Self::UiScope => "ui-scope",
        }
    }

    pub fn parse_slug(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "route" => Some(Self::Route),
            "scene" => Some(Self::Scene),
            "scene-panel" => Some(Self::ScenePanel),
            "scene-block" => Some(Self::SceneBlock),
            "projection" => Some(Self::Projection),
            "world-file" => Some(Self::WorldFile),
            "world-dataset" => Some(Self::WorldDataset),
            "world-metric" => Some(Self::WorldMetric),
            "world-explain" => Some(Self::WorldExplain),
            "dataset" => Some(Self::Dataset),
            "component" => Some(Self::Component),
            "board-file" => Some(Self::BoardFile),
            "board-slot" => Some(Self::BoardSlot),
            "template" => Some(Self::Template),
            "artifact" => Some(Self::Artifact),
            "graph-semantic" => Some(Self::GraphSemantic),
            "graph-eval" => Some(Self::GraphEval),
            "mcg-node" => Some(Self::McgNode),
            "ui-scope" => Some(Self::UiScope),
            _ => None,
        }
    }
}

impl BuildNodeId {
    pub fn new(kind: BuildNodeKind, key: impl Into<String>) -> Self {
        Self {
            kind,
            key: key.into(),
        }
    }

    pub fn route(scene_id: impl Into<String>) -> Self {
        Self::new(BuildNodeKind::Route, scene_id)
    }

    pub fn scene(scene_id: impl Into<String>) -> Self {
        Self::new(BuildNodeKind::Scene, scene_id)
    }

    pub fn projection(scene_id: impl Into<String>, projection_id: impl Into<String>) -> Self {
        Self::new(
            BuildNodeKind::Projection,
            format!("{}/{}", scene_id.into(), projection_id.into()),
        )
    }

    /// Key format: `{scene_id}/{panel_id}`.
    pub fn scene_panel(scene_id: impl Into<String>, panel_id: impl Into<String>) -> Self {
        Self::new(
            BuildNodeKind::ScenePanel,
            format!("{}/{}", scene_id.into(), panel_id.into()),
        )
    }

    /// Key format: `{scene_id}/{panel_id}/{block_id}`.
    pub fn scene_block(
        scene_id: impl Into<String>,
        panel_id: impl Into<String>,
        block_id: impl Into<String>,
    ) -> Self {
        Self::new(
            BuildNodeKind::SceneBlock,
            format!(
                "{}/{}/{}",
                scene_id.into(),
                panel_id.into(),
                block_id.into()
            ),
        )
    }

    pub fn world_dataset(file_path: impl Into<String>, dataset_id: impl Into<String>) -> Self {
        Self::new(
            BuildNodeKind::WorldDataset,
            format!("{}#{}", file_path.into(), dataset_id.into()),
        )
    }

    pub fn world_metric(file_path: impl Into<String>, metric_id: impl Into<String>) -> Self {
        Self::new(
            BuildNodeKind::WorldMetric,
            format!("{}#{}", file_path.into(), metric_id.into()),
        )
    }

    pub fn world_explain(
        file_path: impl Into<String>,
        metric_id: impl Into<String>,
        explain_id: impl Into<String>,
    ) -> Self {
        Self::new(
            BuildNodeKind::WorldExplain,
            format!(
                "{}#{}#{}",
                file_path.into(),
                metric_id.into(),
                explain_id.into()
            ),
        )
    }

    pub fn dataset(resource_id: impl Into<String>) -> Self {
        Self::new(BuildNodeKind::Dataset, resource_id)
    }

    pub fn component(use_key: impl Into<String>) -> Self {
        Self::new(BuildNodeKind::Component, use_key)
    }

    pub fn board_file(board_path: impl Into<String>) -> Self {
        Self::new(BuildNodeKind::BoardFile, board_path)
    }

    pub fn board_slot(board_path: impl Into<String>, slot_id: impl Into<String>) -> Self {
        Self::new(
            BuildNodeKind::BoardSlot,
            format!("{}/{}", board_path.into(), slot_id.into()),
        )
    }

    pub fn template(template_key: impl Into<String>) -> Self {
        Self::new(BuildNodeKind::Template, template_key)
    }

    /// Key format: `{scene_id}/{scope_path}`.
    pub fn ui_scope(scene_id: impl Into<String>, scope_path: impl Into<String>) -> Self {
        Self::new(
            BuildNodeKind::UiScope,
            format!("{}/{}", scene_id.into(), scope_path.into()),
        )
    }

    pub fn artifact(kind: impl Into<String>, scope_key: impl Into<String>) -> Self {
        Self::new(
            BuildNodeKind::Artifact,
            format!("{}/{}", kind.into(), scope_key.into()),
        )
    }

    pub fn encode(&self) -> String {
        format!("{}:{}", self.kind.slug(), self.key)
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let (kind_slug, key) = trimmed.split_once(':')?;
        let kind = BuildNodeKind::parse_slug(kind_slug)?;
        let key = key.trim();
        if key.is_empty() {
            return None;
        }
        let kind = normalize_legacy_catalog_kind(kind, key);
        Some(Self {
            kind,
            key: key.to_string(),
        })
    }

    pub fn default_tab(&self) -> BuildViewTab {
        use crate::{BuildNodeKind, BuildViewTab};
        use BuildNodeKind::{
            Dataset, Projection, Route, Scene, SceneBlock, ScenePanel, WorldDataset, WorldExplain,
            WorldMetric,
        };
        use BuildViewTab::{Overview, Preview};
        match self.kind {
            Route | Scene | Projection | WorldMetric | WorldDataset | WorldExplain | Dataset
                if tab_visible_for_node(self, Preview) =>
            {
                Preview
            }
            ScenePanel | SceneBlock | BuildNodeKind::UiScope => Overview,
            BuildNodeKind::Component | BuildNodeKind::Template => Preview,
            BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => Preview,
            _ => tabs_for_node_kind(self.kind)
                .first()
                .copied()
                .unwrap_or(Overview),
        }
    }
}

fn is_template_file_catalog_key(key: &str) -> bool {
    key.contains('/') || key.ends_with(".mei")
}

fn normalize_legacy_catalog_kind(kind: BuildNodeKind, key: &str) -> BuildNodeKind {
    if kind == BuildNodeKind::Template && !is_template_file_catalog_key(key) {
        BuildNodeKind::Component
    } else {
        kind
    }
}
