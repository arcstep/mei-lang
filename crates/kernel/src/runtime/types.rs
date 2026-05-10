use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeIntent {
    pub kind: String,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeTraceItem {
    pub kind: String,
    pub message: String,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    pub seed: u64,
    pub phase: String,
    pub result: String,
    #[serde(default)]
    pub reason: Option<String>,
    pub countdown: i64,
    #[serde(default)]
    pub placements: BTreeMap<String, String>,
    #[serde(default)]
    pub inventory: Vec<String>,
    #[serde(default)]
    pub statuses: BTreeMap<String, String>,
    #[serde(default)]
    pub flags: BTreeMap<String, bool>,
    #[serde(default)]
    pub timeline: Vec<String>,
    #[serde(default)]
    pub trace_events: Vec<RuntimeTraceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeEntityView {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub slot: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub flags: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCellView {
    pub id: String,
    #[serde(default)]
    pub entities: Vec<RuntimeEntityView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSceneView {
    pub scene_id: String,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    pub phase: String,
    pub result: String,
    #[serde(default)]
    pub reason: Option<String>,
    pub countdown: i64,
    #[serde(default)]
    pub inventory: Vec<String>,
    #[serde(default)]
    pub entities: Vec<RuntimeEntityView>,
    #[serde(default)]
    pub cells: Vec<RuntimeCellView>,
    #[serde(default)]
    pub available_actions: Vec<String>,
    #[serde(default)]
    pub start_label: Option<String>,
}
