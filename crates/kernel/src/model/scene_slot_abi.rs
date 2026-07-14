//! Phase 3 Scene Slot ABI (0104 §9.1 / 0105 §8).
//!
//! Public semantic slots exposed by a Scene module. Projected from legacy
//! panel_ref / content_panel / ui_layout_index — not author-facing yet.

use serde::{Deserialize, Serialize};

/// Scene slot module id (stable string, e.g. `scene:home`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SceneSlotModuleId(pub String);

impl SceneSlotModuleId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn for_stage(stage_id: &str) -> Self {
        Self::new(format!("scene:{stage_id}"))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Cardinality for a semantic slot fill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlotCardinality {
    #[serde(default = "default_min")]
    pub min: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
}

fn default_min() -> u32 {
    0
}

impl Default for SlotCardinality {
    fn default() -> Self {
        Self {
            min: 0,
            max: Some(1),
        }
    }
}

impl SlotCardinality {
    pub fn required_one() -> Self {
        Self {
            min: 1,
            max: Some(1),
        }
    }

    pub fn allows_count(&self, count: u32) -> bool {
        if count < self.min {
            return false;
        }
        match self.max {
            Some(max) => count <= max,
            None => true,
        }
    }
}

/// One public semantic slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticSlotDecl {
    pub slot_id: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub cardinality: SlotCardinality,
    #[serde(default)]
    pub accepted_capability_ids: Vec<String>,
    /// Optional viewpoint / anchor ids bound to this slot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<String>,
    /// Call-site anchor (panel_ref / section).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_site_anchor: Option<String>,
    /// Definition-site anchor (content_panel / module).
    pub source_anchor: String,
    /// When set, slot is local to a Slides unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slide_unit_id: Option<String>,
}

/// Scene module Slot ABI surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SceneSlotModule {
    pub module_id: SceneSlotModuleId,
    #[serde(default = "default_abi_version")]
    pub version: String,
    #[serde(default)]
    pub slots: Vec<SemanticSlotDecl>,
    #[serde(default)]
    pub compatible_surfaces: Vec<String>,
    pub source_anchor: String,
}

fn default_abi_version() -> String {
    "1".to_string()
}

impl SceneSlotModule {
    pub fn get_slot(&self, slot_id: &str) -> Option<&SemanticSlotDecl> {
        self.slots.iter().find(|s| s.slot_id == slot_id)
    }

    pub fn slot_ids(&self) -> Vec<&str> {
        self.slots.iter().map(|s| s.slot_id.as_str()).collect()
    }
}
