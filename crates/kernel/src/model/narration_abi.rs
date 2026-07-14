//! Phase 3 Narration ABI minimum freeze (0409 / 0105 §8).
//!
//! Projected from presentation_map.defaultScript / deck steps.
//! No authored script ⇒ empty catalog (do not synthesize default cues).

use serde::{Deserialize, Serialize};

/// Public cue target kinds (no DOM / mesh paths).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum NarrationCueTarget {
    Slide(String),
    Slot(String),
    Viewpoint(String),
    T2Page(String),
    WorldEntity(String),
}

impl NarrationCueTarget {
    pub fn kind_slug(&self) -> &'static str {
        match self {
            Self::Slide(_) => "slide",
            Self::Slot(_) => "slot",
            Self::Viewpoint(_) => "viewpoint",
            Self::T2Page(_) => "t2_page",
            Self::WorldEntity(_) => "world_entity",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Slide(id)
            | Self::Slot(id)
            | Self::Viewpoint(id)
            | Self::T2Page(id)
            | Self::WorldEntity(id) => id.as_str(),
        }
    }
}

/// One narration cue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NarrationCue {
    pub id: String,
    pub target: NarrationCueTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_notes: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing_ms: Option<u64>,
    pub source_anchor: String,
}

/// Ordered cue track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NarrationTrack {
    pub id: String,
    #[serde(default)]
    pub cues: Vec<NarrationCue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Stage-level narration catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NarrationCatalog {
    #[serde(default)]
    pub catalog_id: String,
    #[serde(default)]
    pub tracks: Vec<NarrationTrack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_anchor: Option<String>,
}

impl NarrationCatalog {
    pub fn is_empty(&self) -> bool {
        self.tracks.iter().all(|t| t.cues.is_empty())
    }

    pub fn cue_count(&self) -> usize {
        self.tracks.iter().map(|t| t.cues.len()).sum()
    }
}
