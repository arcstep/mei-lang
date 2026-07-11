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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub widths: BTreeMap<String, String>,
    /// Compiler-derived section height in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_derived_height_px: Option<f64>,
    /// Section padding profile enum key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding_profile: Option<String>,
    /// Grid track list from `panel.layout.columns` (space-joined CSS value).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_template_columns: Option<String>,
    /// Grid track list from `panel.layout.rows` (space-joined CSS value).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_template_rows: Option<String>,
    /// `grid-template-areas` value (quoted row tokens, space-joined).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_template_areas: Option<String>,
    /// Named slot areas for direct-child `grid-area` assignment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_areas: Option<Vec<String>>,
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

    /// Client runtime projection manifest keyed by `preview_scope`.
    pub fn layout_budget_manifest(&self, revision: &str) -> LayoutBudgetManifest {
        let mut entries = BTreeMap::new();
        for node in self.nodes.values() {
            // Include Content hosts that carry grid budgets (e.g. status-flow
            // `issue_body`); otherwise client compose never receives the 2×N grid.
            let role_ok = matches!(
                node.role,
                UiScopeRole::Section
                    | UiScopeRole::Slot
                    | UiScopeRole::Region
                    | UiScopeRole::Content
            );
            if !role_ok {
                continue;
            }
            let Some(budget) = node.budget.as_ref() else {
                continue;
            };
            if node.role == UiScopeRole::Content
                && budget.grid_template_areas.is_none()
                && budget.slot_areas.is_none()
                && budget.grid_template_columns.is_none()
                && budget.grid_template_rows.is_none()
            {
                continue;
            }
            entries.insert(
                node.preview_scope.clone(),
                LayoutBudgetManifestEntry {
                    preview_scope: node.preview_scope.clone(),
                    slot_height_px: None,
                    padding_profile: budget.padding_profile.clone(),
                    grid_template_columns: budget.grid_template_columns.clone(),
                    grid_template_rows: budget.grid_template_rows.clone(),
                    grid_template_areas: budget.grid_template_areas.clone(),
                    slot_areas: budget.slot_areas.clone(),
                    gap: budget.gap.clone(),
                },
            );
        }
        LayoutBudgetManifest {
            revision: revision.to_string(),
            entries,
        }
    }
}

/// Bootstrap attachment for client layout budget projection (pretty-panels pilot).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LayoutBudgetManifest {
    pub revision: String,
    #[serde(default)]
    pub entries: BTreeMap<String, LayoutBudgetManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LayoutBudgetManifestEntry {
    pub preview_scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_height_px: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_template_columns: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_template_rows: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_template_areas: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_areas: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<String>,
}
