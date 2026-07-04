use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::build_node::BuildNodeId;

/// Semantic role in the 0300 layout structure chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiScopeRole {
    Scene,
    Plane,
    Region,
    Section,
    Slot,
    Content,
    Budget,
}

impl UiScopeRole {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Plane => "plane",
            Self::Region => "region",
            Self::Section => "section",
            Self::Slot => "slot",
            Self::Content => "content",
            Self::Budget => "budget",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Scene => "S",
            Self::Plane => "P",
            Self::Region => "R",
            Self::Section => "§",
            Self::Slot => "L",
            Self::Content => "C",
            Self::Budget => "B",
        }
    }

    pub fn agent_key(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Plane => "plane",
            Self::Region => "region",
            Self::Section => "section",
            Self::Slot => "slot",
            Self::Content => "content",
            Self::Budget => "budget",
        }
    }
}

/// Source file anchor for Agent export.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UiSourceAnchor {
    pub file: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub symbol_id: String,
}

/// Typography / spacing budget attached to a slot or section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct UiBudgetSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_height: Option<i64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub widths: BTreeMap<String, String>,
    /// Row px budgets from `__mei_content_budget.rows`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_rows: Option<Vec<i64>>,
    /// Gap from `__mei_content_budget.gap`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_gap: Option<String>,
    /// Compiler-derived section height in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_derived_height_px: Option<f64>,
    /// Section padding profile enum key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding_profile: Option<String>,
}

/// One node in the UI structure tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiScopeNode {
    pub node_id: String,
    pub role: UiScopeRole,
    pub label: String,
    #[serde(default)]
    pub scope_path: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    pub preview_scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<UiBudgetSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_anchors: Vec<UiSourceAnchor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_id: Option<String>,
}

/// Compile-time UI layout structure index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct UiLayoutIndex {
    #[serde(default)]
    pub nodes: BTreeMap<String, UiScopeNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scene_roots: Vec<String>,
}

impl UiLayoutIndex {
    pub fn lookup<'a>(&'a self, node: &BuildNodeId) -> Option<&'a UiScopeNode> {
        self.nodes.get(&node.encode())
    }

    pub fn lookup_by_encoded<'a>(&'a self, node_id: &str) -> Option<&'a UiScopeNode> {
        self.nodes.get(node_id)
    }

    /// Ancestor chain from root to node (inclusive), for Agent export.
    pub fn ancestor_chain(&self, node_id: &str) -> Vec<&UiScopeNode> {
        let mut chain = Vec::new();
        let mut current = node_id.to_string();
        while let Some(node) = self.nodes.get(&current) {
            chain.push(node);
            current = node.parent_id.clone().unwrap_or_default();
            if current.is_empty() {
                break;
            }
        }
        chain.reverse();
        chain
    }
}
