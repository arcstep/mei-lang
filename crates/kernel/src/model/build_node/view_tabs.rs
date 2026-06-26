use super::BuildNodeId;

use super::BuildNodeKind;

use serde::{Deserialize, Serialize};

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
        BuildNodeKind::McgNode => &[Overview, Provenance, Agent],
    }
}

pub fn tab_visible_for_node(node: &BuildNodeId, tab: BuildViewTab) -> bool {
    tabs_for_node_kind(node.kind).contains(&tab)
}
