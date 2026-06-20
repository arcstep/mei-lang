use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::build_node::BuildNodeId;

/// One step in a panel_ref / embed mount chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct MountChainEntry {
    pub file: String,
    pub panel_id: String,
    pub role: String,
}

/// Compile-time manifest for one build-view experience node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ExperienceNodeManifest {
    pub node_id: String,
    pub kind: String,
    pub label: String,
    #[serde(default)]
    pub experience_path: Vec<String>,
    #[serde(default)]
    pub mount_chain: Vec<MountChainEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backing_refs: Vec<String>,
    #[serde(default)]
    pub tree_tier: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
}

/// Pre-built build-view index written at compile finish.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BuildExperienceIndex {
    #[serde(default)]
    pub node_manifest: BTreeMap<String, ExperienceNodeManifest>,
    #[serde(default)]
    pub reachability_snapshot: Vec<ReachabilityTreeRootSnapshot>,
}

/// One slot inside a `*.board.mei` scene export.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BoardSlotEntry {
    pub slot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backing_refs: Vec<String>,
}

/// Compile-time catalog entry for one board scene export.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BoardFileEntry {
    pub board_file: String,
    pub scene_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<BoardSlotEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub popup_consumers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BuildBoardIndex {
    #[serde(default)]
    pub boards: BTreeMap<String, BoardFileEntry>,
}

/// Template catalog entry for build-view / Agent export.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TemplateCatalogEntry {
    pub template_key: String,
    pub template_file: String,
    pub category: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub props_schema: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub consumers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BuildTemplateIndex {
    #[serde(default)]
    pub templates: BTreeMap<String, TemplateCatalogEntry>,
}

impl BuildBoardIndex {
    pub fn lookup<'a>(&'a self, node: &'a BuildNodeId) -> Option<&'a BoardFileEntry> {
        match node.kind {
            super::build_node::BuildNodeKind::BoardFile => self.boards.get(&node.key),
            super::build_node::BuildNodeKind::BoardSlot => {
                let (board_key, _) = node.key.rsplit_once('/')?;
                self.boards.get(board_key)
            }
            _ => None,
        }
    }
}

impl BuildTemplateIndex {
    pub fn lookup<'a>(&'a self, key: &str) -> Option<&'a TemplateCatalogEntry> {
        self.templates.get(key)
    }
}

/// Serializable reachability tree (mirrors compile reachability_tree types).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ReachabilityTreeNodeSnapshot {
    pub id: String,
    pub node_id: String,
    pub kind: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub badges: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ReachabilityTreeNodeSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ReachabilityTreeRootSnapshot {
    pub group: String,
    pub label: String,
    #[serde(default)]
    pub default_open: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ReachabilityTreeNodeSnapshot>,
}

impl ExperienceNodeManifest {
    pub fn lookup<'a>(compiled: &'a super::CompiledApp, node: &BuildNodeId) -> Option<&'a Self> {
        compiled
            .build_experience_index
            .node_manifest
            .get(&node.encode())
    }
}
