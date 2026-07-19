//! App-level Narration Track ABI (0409 / 0119).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum NarrationTiming {
    Milliseconds(u64),
    Manual,
}

/// One narration cue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NarrationCue {
    pub id: String,
    /// Fully-qualified public target. It is intentionally not split into stage/slide fields.
    pub target_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_notes: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<NarrationTiming>,
    pub source_anchor: String,
}

/// Ordered cue track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NarrationTrack {
    pub id: String,
    pub title: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_for: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub cues: Vec<NarrationCue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_timing_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    pub source_anchor: String,
    pub digest: String,
}

/// App-level catalog. A single catalog may target multiple Stage/Admin entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NarrationCatalog {
    #[serde(default)]
    pub catalog_id: String,
    #[serde(default)]
    pub app_id: String,
    #[serde(default)]
    pub tracks: Vec<NarrationTrack>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub default_track_by_entry: BTreeMap<String, String>,
    #[serde(default)]
    pub source_digest: String,
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
