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

    pub fn artifact(kind: impl Into<String>, scope_key: impl Into<String>) -> Self {
        Self::new(BuildNodeKind::Artifact, format!("{}/{}", kind.into(), scope_key.into()))
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
        Some(Self {
            kind,
            key: key.to_string(),
        })
    }

    pub fn default_tab(&self) -> BuildViewTab {
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
            ScenePanel | SceneBlock => Overview,
            _ => tabs_for_node_kind(self.kind)
                .first()
                .copied()
                .unwrap_or(Overview),
        }
    }
}

/// Source anchor for external IDE agents: file path + symbol id (no line numbers).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceAnchor {
    pub file: String,
    pub symbol_id: String,
    pub symbol_kind: String,
}

impl ProvenanceAnchor {
    pub fn encode(&self) -> String {
        format!("{}#{}", self.file, self.symbol_id)
    }
}

/// Build-view main pane tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildViewTab {
    Overview,
    Preview,
    Exec,
    Semantic,
    Eval,
    Artifact,
    Provenance,
    Agent,
}

impl BuildViewTab {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Preview => "preview",
            Self::Exec => "exec",
            Self::Semantic => "semantic",
            Self::Eval => "eval",
            Self::Artifact => "artifact",
            Self::Provenance => "provenance",
            Self::Agent => "agent",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "概览",
            Self::Preview => "预览",
            Self::Exec => "执行",
            Self::Semantic => "语义图",
            Self::Eval => "求值图",
            Self::Artifact => "产物",
            Self::Provenance => "溯源",
            Self::Agent => "Agent",
        }
    }

    pub fn parse_slug(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "overview" => Some(Self::Overview),
            "preview" => Some(Self::Preview),
            "exec" => Some(Self::Exec),
            "semantic" => Some(Self::Semantic),
            "eval" => Some(Self::Eval),
            "artifact" => Some(Self::Artifact),
            "provenance" => Some(Self::Provenance),
            "agent" => Some(Self::Agent),
            // legacy manage tabs
            "source" | "diff" | "diagnostics" => Some(Self::Overview),
            _ => None,
        }
    }
}

pub fn tabs_for_node_kind(kind: BuildNodeKind) -> &'static [BuildViewTab] {
    use BuildViewTab::{
        Agent, Artifact as ArtifactTab, Eval, Exec, Overview, Preview, Provenance, Semantic,
    };
    match kind {
        BuildNodeKind::Route => &[Overview, Preview, Provenance, Agent],
        BuildNodeKind::Scene => &[Overview, Preview, Provenance, Agent],
        BuildNodeKind::ScenePanel | BuildNodeKind::SceneBlock => {
            &[Overview, Preview, Provenance, Agent]
        }
        BuildNodeKind::Projection => &[Overview, Preview, Semantic, Provenance, Agent],
        BuildNodeKind::WorldFile => &[Overview, Preview, Provenance, Agent],
        BuildNodeKind::WorldDataset | BuildNodeKind::WorldMetric | BuildNodeKind::WorldExplain => {
            &[Overview, Preview, Exec, Provenance, Agent]
        }
        BuildNodeKind::Dataset => &[Overview, Preview, Exec, Provenance, Agent],
        BuildNodeKind::Component | BuildNodeKind::Template => {
            &[Overview, Preview, Provenance, Agent]
        }
        BuildNodeKind::BoardFile | BuildNodeKind::BoardSlot => {
            &[Overview, Preview, Semantic, Provenance, Agent]
        }
        BuildNodeKind::Artifact => &[Overview, ArtifactTab, Agent],
        BuildNodeKind::GraphSemantic => &[Semantic, Agent],
        BuildNodeKind::GraphEval => &[Eval, Agent],
    }
}

pub fn tab_visible_for_node(node: &BuildNodeId, tab: BuildViewTab) -> bool {
    tabs_for_node_kind(node.kind).contains(&tab)
}

/// Legacy build-view query fields (`file`, `world_*`, old tabs) mapped to canonical node + tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyBuildQuery {
    pub file: Option<String>,
    pub scene: Option<String>,
    pub world_metric: Option<String>,
    pub world_dataset: Option<String>,
    pub explain: Option<String>,
    pub tab: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBuildViewQuery {
    pub node: BuildNodeId,
    pub tab: BuildViewTab,
    pub scope: BuildExecScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildExecScope {
    #[default]
    Warmup,
    Empty,
    LastRequest,
    Custom,
}

impl BuildExecScope {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Warmup => "warmup",
            Self::Empty => "empty",
            Self::LastRequest => "last_request",
            Self::Custom => "custom",
        }
    }

    pub fn parse_slug(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "empty" => Self::Empty,
            "last_request" => Self::LastRequest,
            "custom" => Self::Custom,
            _ => Self::Warmup,
        }
    }
}

pub fn resolve_build_view_query(
    node: Option<&str>,
    scope: Option<&str>,
    tab: Option<&str>,
    legacy: &LegacyBuildQuery,
) -> Option<ResolvedBuildViewQuery> {
    let exec_scope = scope
        .map(BuildExecScope::parse_slug)
        .unwrap_or_default();

    if let Some(raw_node) = node.map(str::trim).filter(|value| !value.is_empty()) {
        let parsed = BuildNodeId::parse(raw_node)?;
        let tab = tab
            .and_then(BuildViewTab::parse_slug)
            .filter(|candidate| tab_visible_for_node(&parsed, *candidate))
            .unwrap_or_else(|| parsed.default_tab());
        return Some(ResolvedBuildViewQuery {
            node: parsed,
            tab,
            scope: exec_scope,
        });
    }

    let file = legacy
        .file
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let tab = legacy
        .tab
        .as_deref()
        .and_then(BuildViewTab::parse_slug)
        .unwrap_or(BuildViewTab::Preview);

    if let Some(metric) = legacy
        .world_metric
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let node = if let Some(explain) = legacy
            .explain
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            BuildNodeId::world_explain(file, metric, explain)
        } else {
            BuildNodeId::world_metric(file, metric)
        };
        let tab = if tab_visible_for_node(&node, tab) {
            tab
        } else {
            node.default_tab()
        };
        return Some(ResolvedBuildViewQuery {
            node,
            tab,
            scope: exec_scope,
        });
    }

    if let Some(dataset) = legacy
        .world_dataset
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let node = BuildNodeId::world_dataset(file, dataset);
        let tab = if tab_visible_for_node(&node, tab) {
            tab
        } else {
            node.default_tab()
        };
        return Some(ResolvedBuildViewQuery {
            node,
            tab,
            scope: exec_scope,
        });
    }

    if file.ends_with(".world.mei") {
        let node = BuildNodeId::new(BuildNodeKind::WorldFile, file);
        let tab = if tab_visible_for_node(&node, tab) {
            tab
        } else {
            node.default_tab()
        };
        return Some(ResolvedBuildViewQuery {
            node,
            tab,
            scope: exec_scope,
        });
    }

    if file.ends_with(".mei") && !file.ends_with(".board.mei") {
        if let Some(scene_id) = file
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".mei"))
            .filter(|value| !value.is_empty())
        {
            let node = BuildNodeId::scene(scene_id.to_string());
            let tab = if tab_visible_for_node(&node, tab) {
                tab
            } else {
                node.default_tab()
            };
            return Some(ResolvedBuildViewQuery {
                node,
                tab,
                scope: exec_scope,
            });
        }
    }

    if let Some(scene) = legacy
        .scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let node = BuildNodeId::scene(scene);
        let tab = if tab_visible_for_node(&node, tab) {
            tab
        } else {
            node.default_tab()
        };
        return Some(ResolvedBuildViewQuery {
            node,
            tab,
            scope: exec_scope,
        });
    }

    let node = BuildNodeId::new(BuildNodeKind::WorldFile, file);
    let tab = if tab_visible_for_node(&node, tab) {
        tab
    } else {
        BuildViewTab::Preview
    };
    Some(ResolvedBuildViewQuery {
        node,
        tab,
        scope: exec_scope,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_node_id_roundtrip() {
        let id = BuildNodeId::world_metric("metrics.world.mei", "total_amount");
        assert_eq!(
            id.encode(),
            "world-metric:metrics.world.mei#total_amount"
        );
        assert_eq!(BuildNodeId::parse(&id.encode()), Some(id));
    }

    #[test]
    fn legacy_static_html_file_maps_to_world_file_preview() {
        let resolved = resolve_build_view_query(
            None,
            None,
            Some("preview"),
            &LegacyBuildQuery {
                file: Some("demo/index.html".to_string()),
                scene: None,
                world_metric: None,
                world_dataset: None,
                explain: None,
                tab: Some("preview".to_string()),
            },
        )
        .expect("resolved");
        assert_eq!(resolved.node.kind, BuildNodeKind::WorldFile);
        assert_eq!(resolved.tab, BuildViewTab::Preview);
    }

    #[test]
    fn legacy_scene_capsule_file_overrides_scene_query() {
        let resolved = resolve_build_view_query(
            None,
            None,
            Some("preview"),
            &LegacyBuildQuery {
                file: Some("details.mei".to_string()),
                scene: Some("home".to_string()),
                world_metric: None,
                world_dataset: None,
                explain: None,
                tab: Some("preview".to_string()),
            },
        )
        .expect("resolved");
        assert_eq!(resolved.node.encode(), "scene:details");
        assert_eq!(resolved.tab, BuildViewTab::Preview);
    }

    #[test]
    fn legacy_world_metric_maps_to_node() {
        let resolved = resolve_build_view_query(
            None,
            None,
            Some("preview"),
            &LegacyBuildQuery {
                file: Some("metrics.world.mei".to_string()),
                scene: None,
                world_metric: Some("total_amount".to_string()),
                world_dataset: None,
                explain: None,
                tab: None,
            },
        )
        .expect("resolved");
        assert_eq!(
            resolved.node.encode(),
            "world-metric:metrics.world.mei#total_amount"
        );
        assert_eq!(resolved.tab, BuildViewTab::Preview);
    }

    #[test]
    fn legacy_source_tab_maps_to_overview() {
        assert_eq!(
            BuildViewTab::parse_slug("source"),
            Some(BuildViewTab::Overview)
        );
    }

    #[test]
    fn projection_node_default_tab_is_preview() {
        let node = BuildNodeId::projection("home", "warning_board");
        assert_eq!(node.default_tab(), BuildViewTab::Preview);
        assert!(tab_visible_for_node(&node, BuildViewTab::Preview));
    }
}
