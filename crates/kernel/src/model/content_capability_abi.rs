//! Phase 3 Content Capability ABI (0104 / 0105 §8).
//!
//! Projected from content_panel / ComponentAsset; internal nested cards stay private.
//! Phase 6: World Content is a capability kind, never a Stage identity.

use serde::{Deserialize, Serialize};

/// Content capability id (usually content_panel id).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentCapabilityId(pub String);

impl ContentCapabilityId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// High-level capability family (Phase 6 additive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContentCapabilityKind {
    #[default]
    Panel,
    World,
}

impl ContentCapabilityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::World => "world",
        }
    }
}

/// Public Content Capability contract (Phase 3 minimum + Phase 6 World).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentCapability {
    pub id: ContentCapabilityId,
    #[serde(default = "default_cap_version")]
    pub version: String,
    #[serde(default)]
    pub kind: ContentCapabilityKind,
    /// High-level props/input summary (not full JSON schema yet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub props_summary: Option<String>,
    /// Placeholder events/actions list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events_actions: Vec<String>,
    /// Data/Eval requirement summary (metric_ref ids etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_eval_requirements: Vec<String>,
    #[serde(default)]
    pub supports_fill: bool,
    #[serde(default)]
    pub supports_intrinsic_size: bool,
    #[serde(default)]
    pub requires_hydration: bool,
    /// Nested private panel/card ids (not part of public Stage MDX contract).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub private_child_ids: Vec<String>,
    pub source_anchor: String,
    /// World Content: render family (`three`, `map`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_family: Option<String>,
    /// World Content: authored world ref id (e.g. `park_world`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_ref: Option<String>,
}

fn default_cap_version() -> String {
    "1".to_string()
}

impl ContentCapability {
    pub fn from_content_panel(
        id: &str,
        source_anchor: &str,
        private_children: Vec<String>,
    ) -> Self {
        let mut cap = Self {
            id: ContentCapabilityId::new(id),
            version: default_cap_version(),
            kind: ContentCapabilityKind::Panel,
            props_summary: None,
            events_actions: Vec::new(),
            data_eval_requirements: Vec::new(),
            supports_fill: true,
            supports_intrinsic_size: false,
            requires_hydration: false,
            private_child_ids: private_children,
            source_anchor: source_anchor.replace('\\', "/"),
            render_family: None,
            world_ref: None,
        };
        cap.maybe_mark_world_from_id();
        cap
    }

    /// Promote to World Content capability when id/component looks like world-stage.
    pub fn maybe_mark_world_from_id(&mut self) {
        let id = self.id.as_str().to_ascii_lowercase();
        let anchor = self.source_anchor.to_ascii_lowercase();
        let is_world = id.contains("world-stage")
            || id.contains("world_stage")
            || id.ends_with("-world")
            || id == "park_world"
            || id == "plaza_native"
            || anchor.contains("world-stage")
            || anchor.contains("/world/");
        if !is_world {
            return;
        }
        self.kind = ContentCapabilityKind::World;
        if self.render_family.is_none() {
            self.render_family = Some("three".to_string());
        }
        if self.world_ref.is_none() {
            self.world_ref = Some(self.id.as_str().to_string());
        }
        if self.events_actions.is_empty() {
            self.events_actions = vec![
                "viewpoint".to_string(),
                "camera".to_string(),
                "entity_pick".to_string(),
                "layer_visibility".to_string(),
            ];
        }
        self.requires_hydration = true;
    }

    pub fn is_world(&self) -> bool {
        self.kind == ContentCapabilityKind::World
    }
}
