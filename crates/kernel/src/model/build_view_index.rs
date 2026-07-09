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
pub struct T2PageSlotEntry {
    pub slot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_zone: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backing_refs: Vec<String>,
}

/// Compile-time catalog entry for one board scene export.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct T2PageFileEntry {
    pub page_file: String,
    pub scene_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<T2PageSlotEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub popup_consumers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BuildT2PageIndex {
    #[serde(default)]
    pub pages: BTreeMap<String, T2PageFileEntry>,
}

/// One in-app usage site for a component / template use_key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TemplateConsumerAnchor {
    pub scene_id: String,
    pub panel_path: String,
    pub block_id: String,
    pub label: String,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub consumer_anchors: Vec<TemplateConsumerAnchor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_mei: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BuildTemplateIndex {
    #[serde(default)]
    pub templates: BTreeMap<String, TemplateCatalogEntry>,
}

impl BuildT2PageIndex {
    pub fn lookup<'a>(&'a self, node: &'a BuildNodeId) -> Option<&'a T2PageFileEntry> {
        match node.kind {
            super::build_node::BuildNodeKind::BoardFile => self.pages.get(&node.key),
            super::build_node::BuildNodeKind::BoardSlot => {
                let (board_key, _) = node.key.rsplit_once('/')?;
                self.pages.get(board_key)
            }
            _ => None,
        }
    }

    /// All board capsule exports declared in one `.board.mei` file.
    pub fn exports_for_board_file<'a>(&'a self, board_file: &str) -> Vec<&'a T2PageFileEntry> {
        self.pages
            .values()
            .filter(|entry| entry.page_file == board_file)
            .collect()
    }

    /// Default scene export for backing-tree `world-file` preview on a board resource.
    /// Uses the lexicographically first export when multiple exist.
    pub fn default_export_scene_for_board_file(&self, board_file: &str) -> Option<String> {
        let mut entries = self.exports_for_board_file(board_file);
        if entries.is_empty() {
            return None;
        }
        entries.sort_by(|left, right| left.scene_id.cmp(&right.scene_id));
        Some(entries[0].scene_id.clone())
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compile_scene: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compile_target: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub board_layout_zone: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ui_role: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preview_scope: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub plane_tier: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_symbol: String,
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
